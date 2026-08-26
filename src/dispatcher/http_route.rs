use std::{error::Error, fmt::Display, time::Duration};

use async_trait::async_trait;
use axum::{body::Body, http::Response};
use boa_engine::JsError;
use tokio::task;
use tokio_util::io::ReaderStream;

use crate::db;
use crate::storage::{Storage, UserStorage};
use crate::utils::parsed_request::ParsedRequest;

use super::DispatchRoute;
use super::{
    ScriptCache, ScriptHttpClient,
    script_engine::{
        create_module_context, evaluate_module_from_path, register_http_vars_to_context,
    },
};

#[async_trait]
pub trait HttpRouteHandler: Sync + Send {
    async fn handle(
        &self,
        request: ParsedRequest,
    ) -> anyhow::Result<(serde_json::Value, Response<Body>)>;
}

pub struct HttpRoute {
    pub(crate) pattern: String,
    pub(crate) handler: Box<dyn HttpRouteHandler>,
    pub(crate) priority: i32,
    pub(crate) write_log: bool,
}

impl HttpRoute {
    pub fn transform(
        value: db::http_route::model::HttpRoute,
        storage: &Storage,
        cache: ScriptCache,
        http_client: ScriptHttpClient,
    ) -> anyhow::Result<Self> {
        // 在转换的时候验证是否是有效的路径, 避免路径穿越
        let filename = storage.user().absolute_path(&value.handler)?;

        let pattern = match value.pattern_kind {
            db::http_route::model::PatternKind::PLAIN => {
                format!("^{}$", regex::escape(&value.pattern))
            }
            db::http_route::model::PatternKind::REGEX => value.pattern.clone(),
        };

        let handler: Box<dyn HttpRouteHandler> = match value.handler_kind {
            db::http_route::model::HandlerKind::STATIC => {
                Box::new(StaticHttpHandler::new(filename))
            }
            db::http_route::model::HandlerKind::SCRIPT => Box::new(ScriptHttpHandler::new(
                filename,
                value.timeout,
                storage.user().clone(),
                cache,
                http_client,
            )),
            db::http_route::model::HandlerKind::NONE => Box::new(NoneHttpHandler::new()),
        };

        return Ok(HttpRoute {
            pattern: pattern,
            handler: handler,
            priority: value.priority,
            write_log: value.write_log,
        });
    }
}

impl DispatchRoute for HttpRoute {
    fn pattern(&self) -> &str {
        &self.pattern
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

pub struct StaticHttpHandler {
    filename: String,
}

impl StaticHttpHandler {
    pub fn new<T: Into<String>>(filename: T) -> Self {
        return Self {
            filename: filename.into(),
        };
    }
}

#[async_trait]
impl HttpRouteHandler for StaticHttpHandler {
    async fn handle(
        &self,
        _: ParsedRequest,
    ) -> anyhow::Result<(serde_json::Value, Response<Body>)> {
        let content_type = mime_guess::from_path(&self.filename).first_or_text_plain();

        Ok((
            serde_json::Value::Null,
            Response::builder()
                .header("Content-Type", content_type.to_string())
                .body(Body::from_stream(ReaderStream::with_capacity(
                    tokio::fs::File::open(&self.filename).await?,
                    10240, // 1M
                )))?,
        ))
    }
}

pub struct ScriptHttpHandler {
    filename: String,
    timeout: i32,
    user_storage: UserStorage,
    cache: ScriptCache,
    http_client: ScriptHttpClient,
}

impl ScriptHttpHandler {
    pub fn new<T: Into<String>>(
        filename: T,
        timeout: i32,
        user_storage: UserStorage,
        cache: ScriptCache,
        http_client: ScriptHttpClient,
    ) -> Self {
        return Self {
            filename: filename.into(),
            timeout,
            user_storage,
            cache,
            http_client,
        };
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
impl HttpRouteHandler for ScriptHttpHandler {
    async fn handle(
        &self,
        request: ParsedRequest,
    ) -> anyhow::Result<(serde_json::Value, Response<Body>)> {
        // 每次运行时重新读取 script
        let filename = tokio::fs::canonicalize(&self.filename).await?;
        let script = tokio::fs::read_to_string(&filename).await?;
        let timeout = self.timeout.clone();
        let user_storage = self.user_storage.clone();
        let module_root = user_storage.absolute_path("")?;
        let cache = self.cache.clone();
        let http_client = self.http_client.clone();

        // 在新线程中运行 js
        let (result, response) = task::spawn_blocking(move || {
            let (mut context, executor, module_loader) =
                create_module_context(module_root.as_ref())?;
            let response = register_http_vars_to_context(
                &mut context,
                &request,
                user_storage,
                cache,
                http_client,
            );
            tokio::runtime::Runtime::new()
                .expect("create new async js runtime failed")
                .block_on(async {
                    tokio::select! {
                        v = evaluate_module_from_path(&script, &filename, &mut context, executor, module_loader) => {
                            let v = v.map_err(|err| ScriptError(err.to_string()))?;
                            Ok((v, response.cell.borrow().clone()))
                        },
                        _ = tokio::time::sleep(Duration::from_millis(timeout as u64)) => Err(ScriptError("script running timeout".to_string())),
                    }
                })
        }).await??;

        let mut builder = Response::builder().status(response.status_code);

        for (k, vs) in response.headers {
            for v in vs {
                builder = builder.header(&k, v);
            }
        }

        let axum_response = match response.body_file {
            Some(body_file) => {
                builder.body(Body::from_stream(ReaderStream::with_capacity(
                    tokio::fs::File::open(&body_file).await?,
                    10240, // 1M
                )))?
            }
            None => builder.body(Body::from(response.body))?,
        };

        Ok((result, axum_response))
    }
}

pub struct NoneHttpHandler {}

impl NoneHttpHandler {
    pub fn new() -> Self {
        return NoneHttpHandler {};
    }
}

#[async_trait]
impl HttpRouteHandler for NoneHttpHandler {
    async fn handle(
        &self,
        _: ParsedRequest,
    ) -> anyhow::Result<(serde_json::Value, Response<Body>)> {
        Ok((
            serde_json::Value::Null,
            Response::builder().status(404).body(Body::empty())?,
        ))
    }
}
