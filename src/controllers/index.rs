use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::Request,
    response::Response,
};
use base64::prelude::*;
use diesel_async::{AsyncPgConnection, pooled_connection::bb8};
use std::net::SocketAddr;

use crate::{
    controllers::Context,
    db::{
        http_log::{
            helper::insert_http_log,
            model::{BodyKind, NewHttpLog},
        },
        system_log::helper::insert_system_log,
    },
    parsed_request::ParsedRequestBody,
    utils::{diesel_json, upload},
};
use crate::{
    dispatcher::Route,
    parsed_request::{ParsedRequest, PersistedUploadFile},
};

pub async fn handle_system_error(
    pool: bb8::Pool<AsyncPgConnection>,
    url: String,
    client_addr: SocketAddr,
    error: anyhow::Error,
) {
    // 处理系统错误, 代表 ParsedRequest::new 处理失败, 或者插入数据库失败的情况
    let msg = format!(
        "when processing request from {:?} with url {:?}, get error {:?}",
        client_addr, url, error
    );

    if let Ok(mut conn) = pool.get().await {
        let err = insert_system_log(&mut conn, &msg).await;
        if let Err(err) = err {
            log::error!("when insert system log msg {:?}, error: {:?}", msg, err);
        }
    } else {
        log::error!(
            "can't acquire connection from pool when insert system log msg {:?}",
            msg
        );
    }
}

pub fn get_http_log_from_request(
    request: &ParsedRequest,
    upload_dir: &str,
) -> anyhow::Result<NewHttpLog> {
    let (body_type, body, file) = match &request.parsed_body {
        ParsedRequestBody::None => (
            BodyKind::RAW,
            BASE64_STANDARD.encode(&request.body),
            PersistedUploadFile::new(),
        ),
        ParsedRequestBody::Form(form, file) => {
            let mut persisted_upload_file = PersistedUploadFile::new();
            for i in file.iter() {
                persisted_upload_file.insert(
                    i.0.clone(),
                    (
                        i.1.0.clone(),
                        upload::persist_upload_file(&i.1.1, upload_dir)?,
                    ),
                );
            }

            (
                BodyKind::FORM,
                serde_json::to_string(form)?,
                persisted_upload_file,
            )
        }
        ParsedRequestBody::Json(value) => (
            BodyKind::JSON,
            serde_json::to_string(value)?,
            PersistedUploadFile::new(),
        ),
    };

    Ok(NewHttpLog {
        client_ip: request.client_addr.ip().to_string(),
        client_port: request.client_addr.port() as i32,
        method: request.method.clone(),
        path: request.path.clone(),
        arg: diesel_json::Json(request.params.clone()),
        header: diesel_json::Json(request.headers.clone()),
        body_type: body_type,
        body: body,
        file: diesel_json::Json(file),
        extra_info: serde_json::Value::Null,
        error_log: None,
    })
}

fn get_real_addr_from_request(
    real_addr_header: &str,
    request: &Request<Body>,
) -> anyhow::Result<SocketAddr> {
    if let (true, Some(real_addr)) = (
        real_addr_header.len() > 0,
        request.headers().get(real_addr_header),
    ) {
        Ok(real_addr.to_str()?.parse()?)
    } else {
        Err(anyhow::anyhow!("real addr header not found"))
    }
}

pub fn get_default_response() -> Response<Body> {
    Response::builder().status(404).body(Body::empty()).unwrap()
}

pub async fn process_route(
    ctx: &Context,
    client_addr: &SocketAddr,
    request: Request<Body>,
    route: &Route,
) -> anyhow::Result<Response<Body>> {
    let request = ParsedRequest::new(client_addr.clone(), request).await?;

    let mut new_http_log = None;
    if route.write_log {
        new_http_log = Some(get_http_log_from_request(
            &request,
            &ctx.startup_config.storage_path,
        )?);
    }

    let result = route.handler.handle(request).await;

    let response = if let Some(mut new_http_log) = new_http_log {
        let response = match result {
            Ok((extra_info, response)) => {
                new_http_log.extra_info = extra_info;
                response
            }
            Err(error) => {
                new_http_log.error_log = Some(error.to_string());
                get_default_response()
            }
        };

        let mut conn = ctx.db_conn().await?;
        let _: i32 = insert_http_log(&mut conn, &new_http_log).await?;

        response
    } else {
        match result {
            Ok((_, response)) => response,
            Err(_) => get_default_response(),
        }
    };

    Ok(response)
}

pub async fn index(
    State(ctx): State<Context>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
) -> Response<Body> {
    let client_addr: SocketAddr = if let Ok(client_addr) =
        get_real_addr_from_request(&ctx.startup_config.http_server.real_addr_header, &request)
    {
        client_addr
    } else {
        addr
    };

    if let Some(route) = {
        // https://rustcc.cn/article?id=ab4703a7-2130-4164-be40-f7a5cd325b09
        // 这里放到花括号里面是为了避免 guard 不穿越 .await
        ctx.dispatcher
            .read()
            .expect("lock poisoned")
            .dispatch(&request)
    } {
        let url = request.uri().to_string();

        match process_route(&ctx, &client_addr, request, &route).await {
            Ok(response) => return response,
            Err(error) => {
                tokio::spawn(handle_system_error(
                    ctx.pool.clone(),
                    url,
                    client_addr,
                    error,
                ));
            }
        }
    };

    get_default_response()
}
