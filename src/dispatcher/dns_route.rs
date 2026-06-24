use std::{error::Error, fmt::Display, net::SocketAddr, time::Duration};

use async_trait::async_trait;
use boa_engine::{Context, JsError, Script, Source};
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::RecordType;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::db;
use crate::storage::{Storage, UserStorage};

use super::DispatchRoute;
use super::{ScriptCache, script_engine::register_dns_vars_to_context};

#[derive(Clone, Debug)]
pub struct DnsRequest {
    pub client_addr: SocketAddr,
    pub name: String,
    pub query_type: RecordType,
    pub query_class: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DnsAnswerKind {
    A,
    AAAA,
    CNAME,
    TXT,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DnsAnswer {
    #[serde(rename = "type")]
    pub kind: DnsAnswerKind,
    pub value: String,
    pub ttl: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DnsResponse {
    #[serde(default = "default_rcode")]
    pub rcode: String,
    #[serde(default = "default_ttl")]
    pub ttl: u32,
    #[serde(default)]
    pub answers: Vec<DnsAnswer>,
}

impl Default for DnsResponse {
    fn default() -> Self {
        Self {
            rcode: default_rcode(),
            ttl: default_ttl(),
            answers: Vec::new(),
        }
    }
}

fn default_rcode() -> String {
    "NOERROR".to_string()
}

fn default_ttl() -> u32 {
    60
}

impl DnsAnswerKind {
    pub fn as_record_type(&self) -> RecordType {
        match self {
            DnsAnswerKind::A => RecordType::A,
            DnsAnswerKind::AAAA => RecordType::AAAA,
            DnsAnswerKind::CNAME => RecordType::CNAME,
            DnsAnswerKind::TXT => RecordType::TXT,
        }
    }
}

impl DnsResponse {
    pub fn response_code(&self) -> ResponseCode {
        parse_response_code(&self.rcode)
    }

    fn filter_for_query(&self, query_type: RecordType) -> Self {
        if query_type == RecordType::ANY {
            return self.clone();
        }

        let mut response = self.clone();
        response
            .answers
            .retain(|answer| answer.kind.as_record_type() == query_type);
        response
    }
}

pub fn parse_response_code(code: &str) -> ResponseCode {
    match code.to_ascii_uppercase().as_str() {
        "NOERROR" => ResponseCode::NoError,
        "NXDOMAIN" => ResponseCode::NXDomain,
        "SERVFAIL" => ResponseCode::ServFail,
        "REFUSED" => ResponseCode::Refused,
        "FORMERR" => ResponseCode::FormErr,
        "NOTIMP" => ResponseCode::NotImp,
        _ => ResponseCode::ServFail,
    }
}

#[async_trait]
pub trait DnsRouteHandler: Sync + Send {
    async fn handle(
        &self,
        request: DnsRequest,
    ) -> anyhow::Result<(serde_json::Value, Option<DnsResponse>)>;
}

pub struct DnsRoute {
    pub(crate) pattern: String,
    pub(crate) handler: Box<dyn DnsRouteHandler>,
    pub(crate) priority: i32,
    pub(crate) write_log: bool,
}

impl DnsRoute {
    pub fn transform(
        value: db::dns_route::model::DnsRoute,
        storage: &Storage,
        cache: ScriptCache,
    ) -> anyhow::Result<Self> {
        let pattern = match value.pattern_kind {
            db::dns_route::model::PatternKind::PLAIN => {
                format!("^{}$", regex::escape(&normalize_dns_name(&value.pattern)))
            }
            db::dns_route::model::PatternKind::REGEX => value.pattern.clone(),
        };

        let handler: Box<dyn DnsRouteHandler> = match value.handler_kind {
            db::dns_route::model::HandlerKind::STATIC => Box::new(StaticDnsHandler::new(
                storage.user().absolute_path(&value.handler)?,
            )),
            db::dns_route::model::HandlerKind::SCRIPT => Box::new(ScriptDnsHandler::new(
                storage.user().absolute_path(&value.handler)?,
                value.timeout,
                storage.user().clone(),
                cache,
            )),
            db::dns_route::model::HandlerKind::NONE => Box::new(NoneDnsHandler::new()),
        };

        Ok(DnsRoute {
            pattern,
            handler,
            priority: value.priority,
            write_log: value.write_log,
        })
    }
}

impl DispatchRoute for DnsRoute {
    fn pattern(&self) -> &str {
        &self.pattern
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

pub fn normalize_dns_name(name: &str) -> String {
    name.trim_end_matches('.').to_ascii_lowercase()
}

pub struct StaticDnsHandler {
    filename: String,
}

impl StaticDnsHandler {
    pub fn new<T: Into<String>>(filename: T) -> Self {
        Self {
            filename: filename.into(),
        }
    }
}

#[async_trait]
impl DnsRouteHandler for StaticDnsHandler {
    async fn handle(
        &self,
        request: DnsRequest,
    ) -> anyhow::Result<(serde_json::Value, Option<DnsResponse>)> {
        let response: DnsResponse =
            serde_json::from_str(&tokio::fs::read_to_string(&self.filename).await?)?;
        Ok((
            serde_json::Value::Null,
            Some(response.filter_for_query(request.query_type)),
        ))
    }
}

pub struct ScriptDnsHandler {
    filename: String,
    timeout: i32,
    user_storage: UserStorage,
    cache: ScriptCache,
}

impl ScriptDnsHandler {
    pub fn new<T: Into<String>>(
        filename: T,
        timeout: i32,
        user_storage: UserStorage,
        cache: ScriptCache,
    ) -> Self {
        Self {
            filename: filename.into(),
            timeout,
            user_storage,
            cache,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScriptError(String);

impl Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "js running failed: {}", self.0)
    }
}

impl Error for ScriptError {}

impl From<JsError> for ScriptError {
    fn from(value: JsError) -> Self {
        ScriptError(value.to_string())
    }
}

#[async_trait]
impl DnsRouteHandler for ScriptDnsHandler {
    async fn handle(
        &self,
        request: DnsRequest,
    ) -> anyhow::Result<(serde_json::Value, Option<DnsResponse>)> {
        let script = tokio::fs::read_to_string(&self.filename).await?;
        let timeout = self.timeout;
        let user_storage = self.user_storage.clone();
        let cache = self.cache.clone();
        let query_type = request.query_type;
        let script_request = request.clone();

        let (result, response) = task::spawn_blocking(move || {
            let mut context = Context::default();
            let response = register_dns_vars_to_context(&mut context, &script_request, user_storage, cache);
            let source: Source<'static, boa_engine::parser::source::UTF8Input<&[u8]>> =
                Source::from_bytes(script.as_bytes());
            let script = Script::parse(source, None, &mut context)?;

            tokio::runtime::Runtime::new()
                .expect("create new async js runtime failed")
                .block_on(async {
                    tokio::select! {
                        v = script.evaluate_async(&mut context) => {
                            let v = v.map_err(|err| ScriptError(err.to_string()))?;
                            Ok((v.to_json(&mut context).map_err(|err| ScriptError(err.to_string()))?, response.cell.borrow().clone()))
                        },
                        _ = tokio::time::sleep(Duration::from_millis(timeout as u64)) => Err(ScriptError("script running timeout".to_string())),
                    }
                })
        }).await??;

        Ok((
            result.unwrap_or_else(|| serde_json::Value::Null),
            response
                .into_response()
                .map(|response| response.filter_for_query(query_type)),
        ))
    }
}

pub struct NoneDnsHandler {}

impl NoneDnsHandler {
    pub fn new() -> Self {
        NoneDnsHandler {}
    }
}

#[async_trait]
impl DnsRouteHandler for NoneDnsHandler {
    async fn handle(
        &self,
        _: DnsRequest,
    ) -> anyhow::Result<(serde_json::Value, Option<DnsResponse>)> {
        Ok((serde_json::Value::Null, None))
    }
}
