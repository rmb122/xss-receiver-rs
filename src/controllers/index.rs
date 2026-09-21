use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, Request, header},
    response::Response,
};
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
    storage::Storage,
    utils::{diesel_bytea, ip2region::Locator, parsed_request::ParsedRequestBody},
};
use crate::{
    dispatcher::HttpRoute,
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

fn get_default_response_headers(request_headers: &HeaderMap) -> HeaderMap {
    let mut response_headers = HeaderMap::from_iter([
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate"),
        ),
        (header::PRAGMA, HeaderValue::from_static("no-cache")),
        (header::EXPIRES, HeaderValue::from_static("0")),
    ]);

    if request_headers.contains_key(header::ORIGIN) {
        response_headers.insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
        for (request_name, response_name) in [
            (header::ORIGIN, header::ACCESS_CONTROL_ALLOW_ORIGIN),
            (
                header::ACCESS_CONTROL_REQUEST_HEADERS,
                header::ACCESS_CONTROL_ALLOW_HEADERS,
            ),
            (
                header::ACCESS_CONTROL_REQUEST_METHOD,
                header::ACCESS_CONTROL_ALLOW_METHODS,
            ),
        ] {
            if let Some(value) = request_headers.get(request_name) {
                response_headers.insert(
                    response_name,
                    HeaderValue::from_str(value.to_str().unwrap_or("")).unwrap(),
                );
            }
        }
    }

    response_headers
}

pub fn get_default_response(headers: HeaderMap) -> Response<Body> {
    let mut response = Response::builder().status(404).body(Body::empty()).unwrap();
    *response.headers_mut() = headers;
    response
}

pub async fn process_http_route(
    ctx: &Context,
    client_addr: &SocketAddr,
    request: Request<Body>,
    http_route: &HttpRoute,
    response_headers: &HeaderMap,
) -> anyhow::Result<Response<Body>> {
    let request = ParsedRequest::new(
        client_addr.clone(),
        request,
        ctx.config.http_server.max_body_size,
    )
    .await?;

    let mut new_http_log = None;
    if http_route.write_log {
        new_http_log = Some(get_http_log_from_request(&request, &ctx.locator, &ctx.storage).await?);
    }

    let result = http_route
        .handler
        .handle(request, response_headers.clone())
        .await;

    let response = if let Some(mut new_http_log) = new_http_log {
        let response = match result {
            Ok((extra_info, response)) => {
                new_http_log.extra_info = diesel_bytea::Json(extra_info);
                response
            }
            Err(error) => {
                new_http_log.error_log = Some(error.to_string());
                get_default_response(response_headers.clone())
            }
        };

        let mut conn = ctx.db_conn().await?;
        let _: i32 = insert_http_log(&mut conn, &new_http_log).await?;

        response
    } else {
        match result {
            Ok((_, response)) => response,
            Err(_) => get_default_response(response_headers.clone()),
        }
    };

    Ok(response)
}

pub async fn index(
    State(ctx): State<Context>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
) -> Response<Body> {
    let response_headers = get_default_response_headers(request.headers());
    let client_addr: SocketAddr = if let Ok(client_addr) =
        get_real_addr_from_request(&ctx.config.http_server.real_addr_header, request.headers())
    {
        client_addr
    } else {
        addr
    };

    // Release the dispatcher lock before executing the selected handler.
    let http_route = ctx
        .http_dispatcher
        .read()
        .expect("lock poisoned")
        .dispatch_key(request.uri().path());
    if let Some(http_route) = http_route {
        let url = request.uri().to_string();

        match process_http_route(&ctx, &client_addr, request, &http_route, &response_headers).await
        {
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

    get_default_response(response_headers)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Arc, RwLock},
    };

    use axum::http::StatusCode;
    use diesel_async::pooled_connection::AsyncDieselConnectionManager;
    use jsonwebtoken::Algorithm;
    use tokio::sync::Mutex;

    use super::*;
    use crate::{
        db::http_route::model::{HandlerKind, PatternKind},
        dispatcher::{DnsDispatcher, HttpDispatcher, ScriptCache, ScriptHttpClient},
        utils::{ip2region::CachePolicy, jwt::JwtManager},
    };

    #[tokio::test]
    async fn script_can_remove_default_allow_headers() {
        let root =
            std::env::temp_dir().join(format!("xss-response-headers-{}", rand::random::<u64>()));
        let mut config =
            crate::startup_config::parse(include_str!("../../config_example.toml")).unwrap();
        config.storage_path = root.to_str().unwrap().to_owned();
        let storage = Storage::new(&config.storage_path).unwrap();
        storage
            .user()
            .write(
                "handler.hjs",
                b"response.removeHeader('aCcEsS-CoNtRoL-AlLoW-HeAdErS');",
            )
            .unwrap();
        let cache = ScriptCache::new(&config.script.cache);
        let http_client = ScriptHttpClient::new(&config.script.http).unwrap();
        let route = HttpRoute::transform(
            crate::db::http_route::model::HttpRoute {
                id: 0,
                pattern_kind: PatternKind::PLAIN,
                pattern: "/script".to_owned(),
                priority: 0,
                timeout: 5000,
                catalog: String::new(),
                handler_kind: HandlerKind::SCRIPT,
                handler: "handler.hjs".to_owned(),
                write_log: false,
                comment: String::new(),
                create_time: chrono::Utc::now(),
            },
            &storage,
            cache.clone(),
            http_client.clone(),
        )
        .unwrap();
        // Logging is disabled, so the lazy pool never opens a connection.
        let pool = bb8::Pool::builder()
            .build_unchecked(AsyncDieselConnectionManager::new("postgres://unused"));
        let context = Context {
            config: Arc::new(config),
            pool,
            jwt_manager: Arc::new(JwtManager::new(Algorithm::HS512, b"test", 60)),
            locator: Arc::new(Locator::new(None, None, CachePolicy::VectorIndex).unwrap()),
            http_dispatcher: Arc::new(RwLock::new(HttpDispatcher::new(vec![route]).unwrap())),
            dns_dispatcher: Arc::new(RwLock::new(DnsDispatcher::new(Vec::new()).unwrap())),
            storage: Arc::new(storage),
            script_cache: cache,
            script_http_client: http_client,
            http_route_update_lock: Arc::new(Mutex::new(())),
            dns_route_update_lock: Arc::new(Mutex::new(())),
        };

        let response = index(
            State(context),
            ConnectInfo("127.0.0.1:12345".parse().unwrap()),
            Request::builder()
                .method("OPTIONS")
                .uri("/script")
                .header("Origin", "https://example.com")
                .header("Access-Control-Request-Method", "POST")
                .header("Access-Control-Request-Headers", "x-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        fs::remove_dir_all(root).unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            !response
                .headers()
                .contains_key("Access-Control-Allow-Headers")
        );
    }

    #[test]
    fn cors_actual_response_allows_the_request_origin_with_credentials() {
        let request = Request::builder()
            .method("POST")
            .header("Origin", "https://example.com")
            .body(Body::empty())
            .unwrap();
        let headers = get_default_response_headers(request.headers());

        assert_eq!(
            headers["Access-Control-Allow-Origin"],
            "https://example.com"
        );
        assert_eq!(headers["Access-Control-Allow-Credentials"], "true");
        assert!(!headers.contains_key("Origin"));
    }

    #[test]
    fn cors_preflight_allows_the_requested_method_and_headers() {
        let request = Request::builder()
            .method("OPTIONS")
            .header("Origin", "https://example.com")
            .header("Access-Control-Request-Method", "POST")
            .header(
                "Access-Control-Request-Headers",
                "content-type, x-custom-header",
            )
            .body(Body::empty())
            .unwrap();
        let headers = get_default_response_headers(request.headers());

        assert_eq!(
            headers["Access-Control-Allow-Origin"],
            "https://example.com"
        );
        assert_eq!(headers["Access-Control-Allow-Methods"], "POST");
        assert_eq!(
            headers["Access-Control-Allow-Headers"],
            "content-type, x-custom-header"
        );
    }

    #[test]
    fn response_without_origin_does_not_add_cors_authorization() {
        let headers = get_default_response_headers(&HeaderMap::new());

        assert!(!headers.contains_key("Access-Control-Allow-Origin"));
        assert!(!headers.contains_key("Access-Control-Allow-Credentials"));
        assert_eq!(
            headers["Cache-Control"],
            "no-store, no-cache, must-revalidate"
        );
    }
}
