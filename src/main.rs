use futures::{
    FutureExt,
    future::{BoxFuture, try_join_all},
};
use log::{error, info};
use std::net::SocketAddr;

use crate::{
    controllers::Context, db::establish_db_connection, db::user::helper::create_init_admin_user,
};

mod controllers;
mod db;
mod dispatcher;
mod startup_config;
mod storage;
mod utils;

fn init_logger() {
    let mut builder = env_logger::Builder::from_default_env();
    builder.filter_level(log::LevelFilter::Info);
    builder.init();
}

#[tokio::main]
async fn main() {
    init_logger();

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() != 2 {
        error!("usage: {} [config_file]", &args[0]);
        std::process::exit(-1);
    }

    let config_content = match std::fs::read_to_string(&args[1]) {
        Ok(content) => content,
        Err(err) => {
            error!("config read error: {:?}", err);
            std::process::exit(-1);
        }
    };

    let config = match startup_config::parse(&config_content) {
        Ok(config) => config,
        Err(err) => {
            error!("config parse error: {:?}", err);
            std::process::exit(-1);
        }
    };

    let db_pool = establish_db_connection(&config.db_url).await;
    let db_pool = match db_pool {
        Ok(db_pool) => db_pool,
        Err(err) => {
            error!("establish db connection failed: {:?}", err);
            std::process::exit(-1);
        }
    };

    if let Err(err) = db::run_migrations(&db_pool).await {
        error!("run database migrations failed: {:?}", err);
        std::process::exit(-1);
    }

    if let Some(create_result) = create_init_admin_user(&db_pool)
        .await
        .expect("create init admin user failed")
    {
        info!(
            "admin user created, username: {}, password: {}",
            create_result.0, create_result.1
        );
    }

    info!(
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

    let context = Context::new(&config, db_pool)
        .await
        .expect("failed to initialize context");

    let mut servers: Vec<BoxFuture<'static, anyhow::Result<()>>> = Vec::new();

    if !config.http_server.listen.is_empty() {
        let listener = tokio::net::TcpListener::bind(&config.http_server.listen)
            .await
            .expect("http server listen failed");
        let http_context = context.clone();

        servers.push(
            async move {
                axum::serve(
                    listener,
                    controllers::get_app_router(http_context)
                        .into_make_service_with_connect_info::<SocketAddr>(),
                )
                .await
                .map_err(anyhow::Error::from)
            }
            .boxed(),
        );
    }

    if !config.dns_server.listen.is_empty() {
        let dns_context = context.clone();
        servers.push(async move { utils::dns_server::start(dns_context).await }.boxed());
    }

    if servers.is_empty() {
        error!("http_server.listen and dns_server.listen are both empty");
        std::process::exit(-1);
    }

    try_join_all(servers).await.unwrap();
}
