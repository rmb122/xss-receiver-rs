use std::sync::{Arc, RwLock};

use axum::{Router, routing::get};
use dispatcher::{Dispatcher, FileHandler, Route, ScriptHandler};

mod controllers;
mod dispatcher;

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
                handler: Box::new(ScriptHandler::new("while (1) {}".to_string(), 3000)),
                send_mail: false,
                write_log: false,
            },
        ])
        .expect("init error");

    dispatcher
}

#[derive(Clone)]
pub struct Context {
    dispatcher: Arc<RwLock<Dispatcher>>,
}

impl Context {
    fn new() -> Self {
        Context {
            dispatcher: Arc::new(RwLock::new(init(Dispatcher::new()))),
        }
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        // .route("/", get(root))
        .fallback(controllers::index)
        .with_state(Context::new());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
