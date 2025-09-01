use std::sync::{Arc, RwLock};

use axum::{Router, routing::post};
use diesel_async::{AsyncPgConnection, pooled_connection::bb8};

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
                handler: Box::new(FileHandler::new("/etc/passwd".to_string())),
                send_mail: false,
                write_log: false,
            },
            Route {
                pattern: "^/abc*$".to_string(),
                handler: Box::new(FileHandler::new("/etc/hosts".to_string())),
                send_mail: false,
                write_log: false,
            },
            Route {
                pattern: "^/qqq$".to_string(),
                handler: Box::new(FileHandler::new("/etc/locale.conf".to_string())),
                send_mail: false,
                write_log: false,
            },
            Route {
                pattern: "^/js$".to_string(),
                handler: Box::new(ScriptHandler::new(
                    "response.send(request.client_addr, '2');response.send(new Uint8Array([98, 55, 66]));response.sendStatus(201);response.sendHeader('a', 'x');response.sendHeader('a33Q', ['x', 'x2']);".to_string(),
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
