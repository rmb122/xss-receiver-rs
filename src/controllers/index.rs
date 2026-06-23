use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderName, HeaderValue, Request},
    response::Response,
};
use diesel_async::{AsyncPgConnection, pooled_connection::bb8};
use std::{collections::HashMap, net::SocketAddr, str::FromStr};

use crate::{
    controllers::Context,
    db::{
        http_log::{
            helper::insert_http_log,
            model::{BodyKind, NewHttpLog},
        },
        system_log::helper::insert_system_log,
    },
    storage::Storage,
    utils::{diesel_bytea, ip2region::Locator, parsed_request::ParsedRequestBody},
};
use crate::{
    dispatcher::Route,
    utils::parsed_request::{ParsedRequest, PersistedUploadFile},
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

pub async fn get_http_log_from_request(
    request: &ParsedRequest,
    locator: &Locator,
    storage: &Storage,
) -> anyhow::Result<NewHttpLog> {
    let (parsed_body_type, parsed_body, file) = match &request.parsed_body {
        ParsedRequestBody::None => (BodyKind::NONE, String::new(), PersistedUploadFile::new()),
        ParsedRequestBody::Failed => (BodyKind::FAILED, String::new(), PersistedUploadFile::new()),
        ParsedRequestBody::Form(form, file) => {
            let mut persisted_upload_file = PersistedUploadFile::new();
            for i in file.iter() {
                persisted_upload_file
                    .insert(i.0.clone(), (i.1.0.clone(), storage.log().save(&i.1.1)?));
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
        location: locator.locate(&request.client_addr.ip().to_string()),
        method: request.method.clone(),
        path: request.path.clone(),
        raw_query: request.raw_query.clone(),
        parsed_query: diesel_bytea::Json(request.parsed_query.clone()),
        header: diesel_bytea::Json(request.headers.clone()),
        parsed_body_type,
        parsed_body: diesel_bytea::StringBytes::new(parsed_body),
        raw_body: request.raw_body.clone(),
        file: diesel_bytea::Json(file),
        extra_info: diesel_bytea::Json(serde_json::Value::Null),
        error_log: None,
    })
}

pub fn get_real_addr_from_request(
    real_addr_header: &str,
    headers: &HeaderMap,
) -> anyhow::Result<SocketAddr> {
    if let (true, Some(real_addr)) = (real_addr_header.len() > 0, headers.get(real_addr_header)) {
        Ok(real_addr.to_str()?.parse()?)
    } else {
        Err(anyhow::anyhow!("real addr header not found"))
    }
}

fn process_response_headers(request_headers: &HeaderMap, mut response: Response) -> Response {
    let mut response_headers: HashMap<String, String> = HashMap::new();

    response_headers.insert(
        "Cache-Control".to_owned(),
        "no-store, no-cache, must-revalidate".to_owned(),
    );
    response_headers.insert("Pragma".to_owned(), "no-cache".to_owned());
    response_headers.insert("Expires".to_owned(), "0".to_owned());

    if let Some(header_value) = request_headers.get("Origin") {
        // 确定为跨域请求
        response_headers.insert(
            "Origin".to_owned(),
            header_value.to_str().unwrap_or("").to_owned(),
        );

        response_headers.insert(
            "Access-Control-Allow-Credentials".to_owned(),
            "true".to_owned(),
        );

        if let Some(header_value) = request_headers.get("Access-Control-Allow-Headers") {
            response_headers.insert(
                "Access-Control-Allow-Headers".to_owned(),
                header_value.to_str().unwrap_or("").to_owned(),
            );
        }
        if let Some(header_value) = request_headers.get("Access-Control-Allow-Method") {
            response_headers.insert(
                "Access-Control-Allow-Methods".to_owned(),
                header_value.to_str().unwrap_or("").to_owned(),
            );
        }
    }

    for (k, v) in response_headers {
        if let (Ok(k), Ok(v)) = (HeaderName::from_str(&k), HeaderValue::from_str(&v)) {
            response.headers_mut().insert(k, v);
        }
    }

    return response;
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
    let request = ParsedRequest::new(
        client_addr.clone(),
        request,
        ctx.config.http_server.max_body_size,
    )
    .await?;

    let mut new_http_log = None;
    if route.write_log {
        new_http_log = Some(get_http_log_from_request(&request, &ctx.locator, &ctx.storage).await?);
    }

    let result = route.handler.handle(request).await;

    let response = if let Some(mut new_http_log) = new_http_log {
        let response = match result {
            Ok((extra_info, response)) => {
                new_http_log.extra_info = diesel_bytea::Json(extra_info);
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
    let request_headers = request.headers().clone();
    let client_addr: SocketAddr = if let Ok(client_addr) =
        get_real_addr_from_request(&ctx.config.http_server.real_addr_header, &request_headers)
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
            Ok(response) => return process_response_headers(&request_headers, response),
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

    process_response_headers(&request_headers, get_default_response())
}
