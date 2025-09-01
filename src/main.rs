use log::{error, info};
use std::net::SocketAddr;

use crate::{
    controllers::Context,
    db::helper::{create_init_admin_user, establish_db_connection},
};

mod controllers;
mod db;
mod dispatcher;
mod parsed_request;
mod startup_config;
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

    let listener = tokio::net::TcpListener::bind(&config.http_server.listen)
        .await
        .expect("http server listen failed");

    axum::serve(
        listener,
        controllers::get_router(Context::new(&config, db_pool))
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
