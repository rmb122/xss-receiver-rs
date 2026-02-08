use std::{error::Error, fmt::Display, time::Duration};

use async_trait::async_trait;
use axum::{body::Body, http::Response};
use boa_engine::{Context, JsError, JsValue, Script, Source};
use tokio::task;
use tokio_util::io::ReaderStream;

use crate::db;
use crate::utils::parsed_request::ParsedRequest;
use crate::storage::Storage;

use super::script_engine::register_vars_to_context;

#[async_trait]
pub trait RouteHandler: Sync + Send {
    async fn handle(
        &self,
        request: ParsedRequest,
    ) -> anyhow::Result<(serde_json::Value, Response<Body>)>;
}

pub struct Route {
    pub(crate) pattern: String,
    pub(crate) handler: Box<dyn RouteHandler>,
    pub(crate) write_log: bool,
}

impl Route {
    pub fn transform(value: db::route::model::Route, storage: &Storage) -> anyhow::Result<Self> {
        // 在转换的时候验证是否是有效的路径, 避免路径穿越
        let filename = storage.user().get_absolute_path(&value.handler)?;

        let handler: Box<dyn RouteHandler> = match value.kind {
            db::route::model::RouteKind::STATIC => Box::new(StaticHandler::new(filename)),
            db::route::model::RouteKind::SCRIPT => {
                Box::new(ScriptHandler::new(filename, value.timeout))
            }
        };

        return Ok(Route {
            pattern: value.pattern,
            handler: handler,
            write_log: value.write_log,
        });
    }
}

pub struct StaticHandler {
    filename: String,
}

impl StaticHandler {
    pub fn new<T: Into<String>>(filename: T) -> Self {
        return Self {
            filename: filename.into(),
        };
    }
}

#[async_trait]
impl RouteHandler for StaticHandler {
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

pub struct ScriptHandler {
    filename: String,
    timeout: i32,
}

impl ScriptHandler {
    pub fn new<T: Into<String>>(filename: T, timeout: i32) -> Self {
        return Self {
            filename: filename.into(),
            timeout,
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
impl RouteHandler for ScriptHandler {
    async fn handle(
        &self,
        request: ParsedRequest,
    ) -> anyhow::Result<(serde_json::Value, Response<Body>)> {
        // 每次运行时重新读取 script
        let script = tokio::fs::read_to_string(&self.filename).await?;
        let timeout = self.timeout.clone();

        // 在新线程中运行 js
        let (result, response) = task::spawn_blocking(move || {
            let mut context = Context::default();
            let response = register_vars_to_context(&mut context, &request);
            let source: Source<'static, boa_engine::parser::source::UTF8Input<&[u8]>> = Source::from_bytes(script.as_bytes());
            let script = Script::parse(source, None, &mut context)?;

            tokio::runtime::Runtime::new()
                .expect("create new async js runtime failed")
                .block_on(async {
                    tokio::select! {
                        v = script.evaluate_async(&mut context) => { 
                            let mut v = v.map_err(|err| ScriptError(err.to_string()))?;
                            if let JsValue::Undefined = v {
                                v = JsValue::Null;
                            }
                            Ok((v.to_json(&mut context).map_err(|err| ScriptError(err.to_string()))?, response.cell.borrow().clone()))
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

        Ok((result, builder.body(Body::from(response.body))?))
    }
}
