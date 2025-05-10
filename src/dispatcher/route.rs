use anyhow::Ok;
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{self, Request, Response},
};
use tokio_util::io::ReaderStream;

#[async_trait]
pub trait RouteHandler: Sync + Send {
    async fn handle(&self, request: Request<Body>) -> anyhow::Result<http::Response<Body>>;
}

pub struct Route {
    pub(crate) pattern: String,
    pub(crate) handler: Box<dyn RouteHandler>,
    pub(crate) send_mail: bool,
    pub(crate) write_log: bool,
    pub(crate) comment: String,
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
    async fn handle(&self, request: Request<Body>) -> anyhow::Result<http::Response<Body>> {
        Ok(
            Response::builder().body(Body::from_stream(ReaderStream::with_capacity(
                tokio::fs::File::open(&self.filename).await?,
                10240, // 1M
            )))?,
        )
    }
}
