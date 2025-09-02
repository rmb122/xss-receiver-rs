use std::sync::{Arc, RwLock};

use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::post};
use diesel_async::{
    AsyncPgConnection,
    pooled_connection::{AsyncDieselConnectionManager, bb8},
};
use serde::Serialize;

use crate::{
    dispatcher::{Dispatcher, FileHandler, Route, ScriptHandler},
    startup_config::StartupConfig,
};

mod auth;
mod index;

fn init(mut dispatcher: Dispatcher) -> Dispatcher {
    dispatcher
        .compile_routes(vec![
            Route {
                pattern: "^/abc$".to_string(),
                handler: Box::new(FileHandler::new("/etc/passwd")),
                send_mail: false,
                write_log: false,
            },
            Route {
                pattern: "^/abc*$".to_string(),
                handler: Box::new(FileHandler::new("/etc/hosts")),
                send_mail: false,
                write_log: false,
            },
            Route {
                pattern: "^/qqq$".to_string(),
                handler: Box::new(FileHandler::new("/etc/locale.conf")),
                send_mail: false,
                write_log: false,
            },
            Route {
                pattern: "^/js$".to_string(),
                handler: Box::new(ScriptHandler::new(
                    "response.send(request.client_addr, '2');response.send(new Uint8Array([98, 55, 66]));response.sendStatus(201);response.sendHeader('a', 'x');response.sendHeader('a33Q', ['x', 'x2']);",
                    3000,
                )),
                send_mail: false,
                write_log: false,
            },
        ])
        .expect("init error");

    dispatcher
}

#[derive(Clone)]
pub struct Context {
    startup_config: StartupConfig,

    pool: bb8::Pool<AsyncPgConnection>,
    dispatcher: Arc<RwLock<Dispatcher>>,
}

impl Context {
    pub fn new(config: &StartupConfig, pool: bb8::Pool<AsyncPgConnection>) -> Self {
        Context {
            startup_config: config.to_owned(),
            pool,
            dispatcher: Arc::new(RwLock::new(init(Dispatcher::new()))),
        }
    }

    pub async fn db_conn(&self) -> anyhow::Result<bb8::PooledConnection<'_, AsyncPgConnection>> {
        Ok(self.pool.get().await?)
    }
}

pub fn get_router(context: Context) -> Router<()> {
    let admin_router = Router::new().route("/login", post(auth::login));

    let prefix = &context.startup_config.http_server.admin_prefix;
    let router = if prefix.is_empty() || prefix == "/" {
        Router::new().merge(admin_router)
    } else {
        Router::new().nest(&prefix, admin_router)
    };
    return router.fallback(index::index).with_state(context);
}

#[derive(Serialize)]
pub struct Response<T: Serialize> {
    code: i32,
    msg: Option<String>,
    payload: Option<T>,
}

impl<T: Serialize> Response<T> {
    pub fn ok() -> Response<T> {
        Response {
            code: 200,
            msg: None,
            payload: None,
        }
    }

    pub fn error<M: Into<String>>(msg: M) -> Response<T> {
        Response {
            code: 500,
            msg: Some(msg.into()),
            payload: None,
        }
    }

    pub fn msg<M: Into<String>>(mut self, msg: M) -> Response<T> {
        self.msg = Some(msg.into());
        self
    }

    pub fn payload(mut self, payload: T) -> Response<T> {
        self.payload = Some(payload);
        self
    }
}

impl<T: Serialize> IntoResponse for Response<T> {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        (StatusCode::OK, Json(self)).into_response()
    }
}

// https://github.com/tokio-rs/axum/blob/main/examples/anyhow-error-response/src/main.rs
// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        (StatusCode::OK, Response::<()>::error(self.0.to_string())).into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
