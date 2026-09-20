use std::{net::SocketAddr, time::Duration};

use async_trait::async_trait;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::RecordType;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::db;
use crate::storage::{Storage, UserStorage};

use super::DispatchRoute;
use super::{
    ScriptCache, ScriptHttpClient,
    script_engine::{
        create_module_context, evaluate_module_from_path, register_dns_vars_to_context,
    },
};

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
        // CNAME redirects the queried name regardless of the requested record type.
        // Resolvers need the alias in A/AAAA answers to continue resolving its target.
        response.answers.retain(|answer| {
            let record_type = answer.kind.as_record_type();
            record_type == query_type || record_type == RecordType::CNAME
        });
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
        http_client: ScriptHttpClient,
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
                http_client,
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
    http_client: ScriptHttpClient,
}

impl ScriptDnsHandler {
    pub fn new<T: Into<String>>(
        filename: T,
        timeout: i32,
        user_storage: UserStorage,
        cache: ScriptCache,
        http_client: ScriptHttpClient,
    ) -> Self {
        Self {
            filename: filename.into(),
            timeout,
            user_storage,
            cache,
            http_client,
        }
    }
}

pub use super::script_engine::ScriptError;

#[async_trait]
impl DnsRouteHandler for ScriptDnsHandler {
    async fn handle(
        &self,
        request: DnsRequest,
    ) -> anyhow::Result<(serde_json::Value, Option<DnsResponse>)> {
        let filename = tokio::fs::canonicalize(&self.filename).await?;
        let script = tokio::fs::read_to_string(&filename).await?;
        let timeout = self.timeout;
        let user_storage = self.user_storage.clone();
        let module_root = user_storage.absolute_path("")?;
        let cache = self.cache.clone();
        let http_client = self.http_client.clone();
        let query_type = request.query_type;
        let script_request = request.clone();

        let (result, response) = task::spawn_blocking(move || {
            // QuickJS stays on this blocking thread while native futures use
            // the server runtime's I/O drivers and timers.
            tokio::runtime::Handle::current().block_on(async {
                let (_runtime, context) = create_module_context(module_root.as_ref()).await?;
                let response = context
                    .with(|ctx| {
                        register_dns_vars_to_context(
                            &ctx,
                            &script_request,
                            user_storage,
                            cache,
                            http_client,
                        )
                        .map_err(|error| ScriptError::from_js(&ctx, error))
                    })
                    .await?;
                let result = evaluate_module_from_path(
                    &script,
                    &filename,
                    &context,
                    Duration::from_millis(timeout.max(0) as u64),
                )
                .await?;
                let response = response.borrow().clone();
                Ok::<_, ScriptError>((result, response))
            })
        })
        .await??;

        Ok((
            result,
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(0);

    struct TempDnsFiles(PathBuf);

    impl TempDnsFiles {
        fn new() -> Self {
            let id = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("xss-receiver-dns-{}-{id}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, name: &str, contents: &str) -> String {
            let path = self.0.join(name);
            fs::write(&path, contents).unwrap();
            path.to_str().unwrap().to_owned()
        }
    }

    impl Drop for TempDnsFiles {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn request(query_type: RecordType) -> DnsRequest {
        DnsRequest {
            client_addr: "127.0.0.1:12345".parse().unwrap(),
            name: "alias.example".to_owned(),
            query_type,
            query_class: "IN".to_owned(),
        }
    }

    async fn assert_alias_answer(handler: &dyn DnsRouteHandler, query_type: RecordType) {
        let (_, response) = handler.handle(request(query_type)).await.unwrap();
        let response = response.unwrap();
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert_eq!(response.answers.len(), 1);
        let answer = &response.answers[0];
        assert_eq!(answer.kind.as_record_type(), RecordType::CNAME);
        assert_eq!(answer.value, "target.example.");
        assert_eq!(answer.ttl, Some(120));
    }

    #[tokio::test]
    async fn static_alias_survives_address_queries() {
        let files = TempDnsFiles::new();
        let handler = StaticDnsHandler::new(files.write(
            "alias.json",
            r#"{"answers":[{"type":"CNAME","value":"target.example.","ttl":120}]}"#,
        ));

        for query_type in [
            RecordType::A,
            RecordType::AAAA,
            RecordType::CNAME,
            RecordType::TXT,
            RecordType::ANY,
        ] {
            assert_alias_answer(&handler, query_type).await;
        }
    }

    #[tokio::test]
    async fn scripted_alias_survives_address_queries() {
        let files = TempDnsFiles::new();
        let handler = ScriptDnsHandler::new(
            files.write(
                "alias.js",
                r#"response.answer("CNAME", "target.example.", 120); export default null;"#,
            ),
            1000,
            UserStorage::new(files.0.clone()),
            ScriptCache::new(&Default::default()),
            ScriptHttpClient::new(&Default::default()).unwrap(),
        );

        for query_type in [RecordType::A, RecordType::AAAA] {
            assert_alias_answer(&handler, query_type).await;
        }
    }

    #[test]
    fn filtering_still_excludes_unrelated_records_and_preserves_metadata() {
        let response: DnsResponse = serde_json::from_str(
            r#"{
                "rcode":"NOERROR", "ttl":300,
                "answers":[
                    {"type":"A","value":"192.0.2.1","ttl":120},
                    {"type":"AAAA","value":"2001:db8::1"},
                    {"type":"TXT","value":"example"}
                ]
            }"#,
        )
        .unwrap();

        for query_type in [RecordType::A, RecordType::AAAA, RecordType::TXT] {
            let filtered = response.filter_for_query(query_type);
            assert_eq!(filtered.rcode, "NOERROR");
            assert_eq!(filtered.ttl, 300);
            assert_eq!(filtered.answers.len(), 1);
            assert_eq!(filtered.answers[0].kind.as_record_type(), query_type);
        }
        assert_eq!(response.filter_for_query(RecordType::ANY).answers.len(), 3);
        assert!(response.filter_for_query(RecordType::MX).answers.is_empty());
    }
}
