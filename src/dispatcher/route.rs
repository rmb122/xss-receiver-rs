use std::{error::Error, fmt::Display, time::Duration};

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, Response},
};
use boa_engine::{Context, Script, Source};
use tokio::task;
use tokio_util::io::ReaderStream;

#[async_trait]
pub trait RouteHandler: Sync + Send {
    async fn handle(&self, request: Request<Body>) -> anyhow::Result<Response<Body>>;
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
    pub fn new(filename: String) -> Self {
        return Self { filename };
    }
}

#[async_trait]
impl RouteHandler for FileHandler {
    async fn handle(&self, _: Request<Body>) -> anyhow::Result<Response<Body>> {
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
    pub fn new(script: String, timeout: i32) -> Self {
        return Self { script, timeout };
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

#[async_trait]
impl RouteHandler for ScriptHandler {
    async fn handle(&self, _: Request<Body>) -> anyhow::Result<Response<Body>> {
        let script = self.script.clone();
        let timeout = self.timeout.clone();
        let result = task::spawn_blocking(move || {
            let mut context = Context::default();
            let source = Source::from_bytes(script.as_bytes());
            let script = Script::parse(source, None, &mut context).map_err(|err| ScriptError(err.to_string()))?;

            let result = tokio::runtime::Runtime::new()
                .expect("create new async js runtime failed")
                .block_on(async {
                    tokio::select! {
                        v = script.evaluate_async(&mut context) => v.map(|x| x.display().to_string()).map_err(|err| ScriptError(err.to_string())),
                        _ = tokio::time::sleep(Duration::from_millis(timeout as u64)) => Err(ScriptError("script running timeout".to_string())),
                    }
                });
            result
        }).await??;

        Ok(Response::builder().body(Body::new(result))?)
    }
}
