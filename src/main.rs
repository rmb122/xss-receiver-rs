use axum::{Router, body::Body, http::Request, routing::get};
use dispatcher::{FileHandler, Route, THE_DISPATCHER};
mod dispatcher;

async fn init() -> () {
    THE_DISPATCHER
        .write()
        .await
        .compile_routes(vec![
            Route {
                pattern: "/abc".to_string(),
                handler: Box::new(FileHandler::new("/etc/passwd".to_string())),
                send_mail: false,
                write_log: false,
                comment: "".to_string(),
            },
            Route {
                pattern: "/abc*".to_string(),
                handler: Box::new(FileHandler::new("/etc/hosts".to_string())),
                send_mail: false,
                write_log: false,
                comment: "".to_string(),
            },
            Route {
                pattern: "/qqq".to_string(),
                handler: Box::new(FileHandler::new("/etc/locale.conf".to_string())),
                send_mail: false,
                write_log: false,
                comment: "".to_string(),
            },
        ])
        .expect("init error")
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(root))
        .fallback(async |request: Request<Body>| {
            dispatcher::THE_DISPATCHER
                .read()
                .await
                .dispatch(request)
                .await
        });

    init().await;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Hello, World!"
}
