use std::{error::Error, fmt::Display, time::Duration};

use async_trait::async_trait;
use axum::{body::Body, http::Response};
use boa_engine::{Context, JsError, Script, Source};
use tokio::task;
use tokio_util::io::ReaderStream;

use crate::parsed_request::ParsedRequest;

use super::script_engine::register_vars_to_context;

#[async_trait]
pub trait RouteHandler: Sync + Send {
    async fn handle(&self, request: ParsedRequest) -> anyhow::Result<Response<Body>>;
}

pub struct Route {
    pub(crate) pattern: String,
    pub(crate) handler: Box<dyn RouteHandler>,
    pub(crate) send_mail: bool,
    pub(crate) write_log: bool,
}

pub struct FileHandler {
    filename: String,
}

impl FileHandler {
    pub fn new<T: Into<String>>(filename: T) -> Self {
        return Self {
            filename: filename.into(),
        };
    }
}

#[async_trait]
impl RouteHandler for FileHandler {
    async fn handle(&self, _: ParsedRequest) -> anyhow::Result<Response<Body>> {
        Ok(
            Response::builder().body(Body::from_stream(ReaderStream::with_capacity(
                tokio::fs::File::open(&self.filename).await?,
                10240, // 1M
            )))?,
        )
    }
}

pub struct ScriptHandler {
    script: String,
    timeout: i32,
}

impl ScriptHandler {
    pub fn new<T: Into<String>>(script: T, timeout: i32) -> Self {
        return Self {
            script: script.into(),
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
    async fn handle(&self, request: ParsedRequest) -> anyhow::Result<Response<Body>> {
        let script = self.script.clone();
        let timeout = self.timeout.clone();

        // 在新线程中运行 js
        let response = task::spawn_blocking(move || {
            let mut context = Context::default();
            let response = register_vars_to_context(&mut context, &request);
            let source: Source<'static, boa_engine::parser::source::UTF8Input<&[u8]>> = Source::from_bytes(script.as_bytes());
            let script = Script::parse(source, None, &mut context)?;

            tokio::runtime::Runtime::new()
                .expect("create new async js runtime failed")
                .block_on(async {
                    tokio::select! {
                        v = script.evaluate_async(&mut context) => { v.map_err(|err| ScriptError(err.to_string()))?; Ok(response.cell.borrow().clone()) },
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

        Ok(builder.body(Body::from(response.body))?)
    }
}
