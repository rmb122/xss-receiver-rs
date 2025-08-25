use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use axum::{Router, routing::get};
use diesel::ConnectionResult;
use diesel_async::{
    AsyncConnection, AsyncPgConnection, RunQueryDsl,
    pooled_connection::{AsyncDieselConnectionManager, bb8},
};
use dispatcher::{Dispatcher, FileHandler, Route, ScriptHandler};
use startup_config::StartupConfig;

mod controllers;
mod db;
mod dispatcher;
mod parsed_request;
mod startup_config;

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
    fn new(config: &StartupConfig, pool: bb8::Pool<AsyncPgConnection>) -> Self {
        Context {
            startup_config: config.to_owned(),
            pool,
            dispatcher: Arc::new(RwLock::new(init(Dispatcher::new()))),
        }
    }
}

async fn establish_db_connection(db_url: &str) -> anyhow::Result<bb8::Pool<AsyncPgConnection>> {
    let config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(db_url);
    Ok(bb8::Pool::builder().build(config).await?)
}

#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() != 2 {
        println!("usage: {} [config_file]", &args[0]);
        std::process::exit(-1);
    }

    let config_content = match std::fs::read_to_string(&args[1]) {
        Ok(content) => content,
        Err(err) => {
            println!("config read error: {:?}", err);
            std::process::exit(-1);
        }
    };

    let config = match startup_config::parse(&config_content) {
        Ok(config) => config,
        Err(err) => {
            println!("config parse error: {:?}", err);
            std::process::exit(-1);
        }
    };

    let db_pool = establish_db_connection(&config.db_url).await;
    let db_pool = match db_pool {
        Ok(db_pool) => db_pool,
        Err(err) => {
            println!("establish db connection failed: {:?}", err);
            std::process::exit(-1);
        }
    };

    db_pool.get().await.unwrap();

    let admin_router = Router::new();

    let prefix = &config.http_server.admin_prefix;
    let router = if prefix.is_empty() || prefix == "/" {
        Router::new().merge(admin_router)
    } else {
        Router::new().nest(&prefix, admin_router)
    };
    let router = router
        .fallback(controllers::index)
        .with_state(Context::new(&config, db_pool));

    println!(
        "xss-receiver started, http: {}, dns: {}",
        if config.http_server.listen.len() > 0 {
            &config.http_server.listen
        } else {
            "disabled"
        },
        if config.dns_server.listen.len() > 0 {
            &config.dns_server.listen
        } else {
            "disabled"
        }
    );
    let listener = tokio::net::TcpListener::bind(config.http_server.listen)
        .await
        .unwrap();
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
