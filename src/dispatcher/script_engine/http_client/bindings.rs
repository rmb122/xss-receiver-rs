use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use reqwest::{
    Method, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use rquickjs::{
    CatchResultExt, CaughtError, Ctx, Exception, Function, IntoJs, Object, Promise,
    Result as JsResult, TypedArray, Value,
    function::Rest,
    object::{Filter, Property},
};

use super::super::helpers::{
    check_argument_count, ensure_exists, json_value_to_js_value, read_u8_array_from_js_value,
};
use super::{BufferedResponse, ClientKey, HttpClientError, OutboundRequest, ScriptHttpClient};

struct ScriptHttpClientCell {
    client: ScriptHttpClient,
    clients: RefCell<HashMap<ClientKey, reqwest::Client>>,
}

struct HttpResponseResult(std::result::Result<BufferedResponse, HttpClientError>);

impl<'js> IntoJs<'js> for HttpResponseResult {
    fn into_js(self, ctx: &Ctx<'js>) -> JsResult<Value<'js>> {
        let response = self.0.map_err(|error| match error {
            HttpClientError::ResponseTooLarge { .. } => {
                Exception::throw_range(ctx, &error.to_string())
            }
            _ => Exception::throw_message(ctx, &error.to_string()),
        })?;
        buffered_response_to_js(response, ctx)
    }
}

fn buffered_response_to_js<'js>(
    response: BufferedResponse,
    ctx: &Ctx<'js>,
) -> JsResult<Value<'js>> {
    let body = Rc::new(response.body);
    let uint8_array = TypedArray::new(ctx.clone(), body.as_ref().clone())?;
    let headers = Object::new_proto(ctx.clone(), None)?;
    for (name, values) in response.headers {
        headers.set(name, values)?;
    }
    let object = Object::new(ctx.clone())?;
    object.prop(
        "statusCode",
        Property::from(response.status_code).enumerable(),
    )?;
    object.prop("url", Property::from(response.url).enumerable())?;
    object.prop("headers", Property::from(headers).enumerable())?;
    object.prop("body", Property::from(uint8_array).enumerable())?;
    let text_body = body.clone();
    object.set(
        "text",
        Function::new(ctx.clone(), move || {
            String::from_utf8_lossy(&text_body).into_owned()
        })?,
    )?;
    object.set(
        "json",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>| {
            let value: serde_json::Value = serde_json::from_slice(&body).map_err(|error| {
                Exception::throw_syntax(&ctx, &format!("invalid JSON response body: {error}"))
            })?;
            json_value_to_js_value(&value, &ctx)
        })?,
    )?;
    Ok(object.into_value())
}

fn parse_request(
    args: &[Value<'_>],
    forced_method: Option<&'static str>,
    client: &ScriptHttpClient,
    ctx: &Ctx<'_>,
) -> JsResult<OutboundRequest> {
    check_argument_count(args, 1, ctx)?;
    let url = ensure_exists(args[0].as_string(), "URL must be a string", ctx)?.to_string()?;
    let url = Url::parse(&url)
        .map_err(|error| Exception::throw_type(ctx, &format!("invalid URL: {error}")))?;
    let options = match args.get(1) {
        None => None,
        Some(value) if value.is_undefined() => None,
        Some(value) => Some(ensure_exists(
            value.as_object(),
            "options must be an object",
            ctx,
        )?),
    };
    let method = if let Some(method) = forced_method {
        Method::from_bytes(method.as_bytes()).expect("fixed HTTP method must be valid")
    } else if let Some(options) = options {
        let value: Value = options.get("method")?;
        if value.is_undefined() {
            Method::GET
        } else {
            let value =
                ensure_exists(value.as_string(), "method must be a string", ctx)?.to_string()?;
            Method::from_bytes(value.as_bytes()).map_err(|error| {
                Exception::throw_type(ctx, &format!("invalid HTTP method: {error}"))
            })?
        }
    } else {
        Method::GET
    };
    let mut headers = HeaderMap::new();
    let mut body = None;
    let mut timeout = client.config.timeout;
    let mut max_response_size = client.config.max_response_size;
    let mut max_redirects = client.config.max_redirects;
    let mut tls_verify = true;
    if let Some(options) = options {
        let value: Value = options.get("headers")?;
        if !value.is_undefined() {
            headers = parse_headers(&value, ctx)?;
        }
        let value: Value = options.get("body")?;
        if !value.is_undefined() {
            body = Some(read_u8_array_from_js_value(&value, ctx)?);
        }
        timeout = parse_bounded_integer(
            options.get("timeout")?,
            "timeout",
            timeout,
            client.config.timeout,
            false,
            ctx,
        )?;
        max_response_size = parse_bounded_integer(
            options.get("maxResponseSize")?,
            "maxResponseSize",
            max_response_size,
            client.config.max_response_size,
            false,
            ctx,
        )?;
        max_redirects = parse_bounded_integer(
            options.get("maxRedirects")?,
            "maxRedirects",
            max_redirects as u64,
            client.config.max_redirects as u64,
            true,
            ctx,
        )? as usize;
        let value: Value = options.get("tlsVerify")?;
        if !value.is_undefined() {
            tls_verify = ensure_exists(value.as_bool(), "tlsVerify must be a boolean", ctx)?;
        }
    }
    Ok(OutboundRequest {
        method,
        url,
        headers,
        body,
        timeout: Duration::from_millis(timeout),
        max_response_size,
        max_redirects,
        tls_verify,
    })
}

fn parse_headers(value: &Value<'_>, ctx: &Ctx<'_>) -> JsResult<HeaderMap> {
    let object = ensure_exists(value.as_object(), "headers must be an object", ctx)?;
    let mut headers = HeaderMap::new();
    for entry in object.own_props::<String, Value>(Filter::new().string()) {
        let (name, value) = entry?;
        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
            Exception::throw_type(ctx, &format!("invalid HTTP header name {name:?}: {error}"))
        })?;
        if let Some(value) = value.as_string() {
            append_header(&mut headers, header_name, &value.to_string()?, ctx)?;
            continue;
        }
        let array = ensure_exists(
            value.as_array(),
            &format!("header {name:?} must be a string or string array"),
            ctx,
        )?;
        let length: u32 = array.as_object().get("length")?;
        for index in 0..length {
            let value: Value = array.get(index as usize)?;
            let value = ensure_exists(
                value.as_string(),
                &format!("header {name:?} array item {index} must be a string"),
                ctx,
            )?;
            append_header(&mut headers, header_name.clone(), &value.to_string()?, ctx)?;
        }
    }
    Ok(headers)
}

fn append_header(
    headers: &mut HeaderMap,
    name: HeaderName,
    value: &str,
    ctx: &Ctx<'_>,
) -> JsResult<()> {
    let value = HeaderValue::from_str(value).map_err(|error| {
        Exception::throw_type(ctx, &format!("invalid value for header {name}: {error}"))
    })?;
    headers.append(name, value);
    Ok(())
}

fn parse_bounded_integer(
    value: Value<'_>,
    name: &str,
    default: u64,
    maximum: u64,
    allow_zero: bool,
    ctx: &Ctx<'_>,
) -> JsResult<u64> {
    if value.is_undefined() {
        return Ok(default);
    }
    let number = ensure_exists(value.as_number(), &format!("{name} must be a number"), ctx)?;
    let minimum = if allow_zero { 0.0 } else { 1.0 };
    if !number.is_finite() || number.fract() != 0.0 || number < minimum {
        return Err(Exception::throw_range(
            ctx,
            &format!(
                "{name} must be an integer greater than or equal to {}",
                minimum as u64,
            ),
        ));
    }
    if number > maximum as f64 {
        return Err(Exception::throw_range(
            ctx,
            &format!("{name} must not exceed {maximum}"),
        ));
    }
    Ok(number as u64)
}

fn request<'js>(
    ctx: &Ctx<'js>,
    cell: &ScriptHttpClientCell,
    forced_method: Option<&'static str>,
    args: &[Value<'js>],
) -> JsResult<Promise<'js>> {
    // Snapshot all JavaScript input before starting I/O. Only Rust-owned state crosses the await.
    let prepared = (|| {
        let request = parse_request(args, forced_method, &cell.client, ctx)?;
        let client = cell
            .client
            .client_for(&cell.clients, request.tls_verify, request.max_redirects)
            .map_err(|error| Exception::throw_message(ctx, &error.to_string()))?;
        Ok((client, request))
    })()
    .catch(ctx);
    let (client, request) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let value = match error {
                CaughtError::Exception(exception) => exception.into_value(),
                CaughtError::Value(value) => value,
                CaughtError::Error(error) => {
                    Exception::from_message(ctx.clone(), &error.to_string())?.into_value()
                }
            };
            let (promise, _, reject) = ctx.promise()?;
            reject.call::<_, ()>((value,))?;
            return Ok(promise);
        }
    };
    let http = cell.client.clone();
    Promise::wrap_future(ctx, async move {
        HttpResponseResult(http.execute(client, request).await)
    })
}

pub fn register_http_client_to_context<'js>(
    ctx: &Ctx<'js>,
    client: ScriptHttpClient,
) -> JsResult<()> {
    let cell = Rc::new(ScriptHttpClientCell {
        client,
        clients: RefCell::new(HashMap::new()),
    });
    let object = Object::new(ctx.clone())?;
    for (name, method) in [
        ("request", None),
        ("get", Some("GET")),
        ("post", Some("POST")),
        ("put", Some("PUT")),
        ("patch", Some("PATCH")),
        ("delete", Some("DELETE")),
        ("head", Some("HEAD")),
    ] {
        let cell = cell.clone();
        let function = Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
            request(&ctx, &cell, method, &args.0)
        })?;
        object.set(name, function)?;
    }
    ctx.globals()
        .prop("http", Property::from(object).enumerable())
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        http::StatusCode,
        response::{AppendHeaders, Redirect},
        routing::{get, post},
    };
    use serde_json::json;

    use super::{ScriptHttpClient, register_http_client_to_context};
    use crate::{
        dispatcher::script_engine::{create_context, evaluate_module},
        startup_config,
    };

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn client_pools_are_scoped_to_one_script_context() {
        let app = Router::new().route("/", get(|| async { "ok" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = ScriptHttpClient::new(&startup_config::ScriptHttp {
            allow_private_network: true,
            ..Default::default()
        })
        .unwrap();
        tokio::task::spawn_blocking(move || {
            // Requests use separate runtimes, as they do in the dispatcher. Reusing a
            // pooled connection from the previous runtime would leave its I/O driver gone.
            for _ in 0..2 {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async {
                    let (_runtime, context) = create_context().await;
                    context
                        .with(|ctx| register_http_client_to_context(&ctx, client.clone()))
                        .await
                        .unwrap();
                    let source =
                        format!("export default (await http.get('http://{address}/')).text();");
                    assert_eq!(
                        evaluate_module(&source, &context).await.unwrap(),
                        json!("ok")
                    );
                });
            }
        })
        .await
        .unwrap();
        server.abort();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn javascript_can_await_buffered_http_response() {
        let app = Router::new()
            .route(
                "/echo",
                post(|body: String| async move {
                    (
                        StatusCode::CREATED,
                        AppendHeaders([
                            ("content-type", "application/json"),
                            ("x-reply", "ok"),
                            ("x-reply", "again"),
                        ]),
                        body,
                    )
                }),
            )
            .route(
                "/not-found",
                get(|| async { (StatusCode::NOT_FOUND, "missing") }),
            )
            .route(
                "/redirect",
                get(|| async { Redirect::temporary("/not-found") }),
            )
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    "slow"
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = ScriptHttpClient::new(&startup_config::ScriptHttp {
            allow_private_network: true,
            timeout: 5_000,
            max_response_size: 1024,
            max_redirects: 2,
        })
        .unwrap();
        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| register_http_client_to_context(&ctx, client))
            .await
            .unwrap();

        let port = address.port();
        let source = format!(
            r#"
                const response = await http.post("http://{address}/echo", {{
                    headers: {{ "content-type": "application/json" }},
                    body: JSON.stringify({{ received: true }}),
                    timeout: 1000,
                    maxResponseSize: 512,
                    maxRedirects: 0,
                }});
                const backing = new Uint8Array([88, 97, 98, 99, 89]);
                const sliced = await http.request("http://{address}/echo", {{
                    method: "POST",
                    headers: {{ "x-multi": ["one", "two"] }},
                    body: backing.subarray(1, 4),
                }});
                const missing = await http.get("http://{address}/not-found");
                const resolved = await http.get("http://localhost:{port}/not-found");
                const redirect = await http.get("http://{address}/redirect", {{ maxRedirects: 0 }});
                const followed = await http.get("http://{address}/redirect", {{ maxRedirects: 1 }});
                const firstJson = response.json();
                firstJson.received = false;
                response.body[0] = 0;
                let invalidJsonError;
                try {{
                    missing.json();
                }} catch (error) {{
                    invalidJsonError = error.name;
                }}
                let timedOut = false;
                try {{
                    await http.get("http://{address}/slow", {{ timeout: 5 }});
                }} catch (error) {{
                    timedOut = true;
                }}
                export default {{
                    statusCode: response.statusCode,
                    missingStatusCode: missing.statusCode,
                    resolvedStatusCode: resolved.statusCode,
                    redirectStatusCode: redirect.statusCode,
                    followedStatusCode: followed.statusCode,
                    header: response.headers["x-reply"][0],
                    headerCount: response.headers["x-reply"].length,
                    length: response.body.length,
                    text: response.text(),
                    json: response.json(),
                    slicedText: sliced.text(),
                    invalidJsonError,
                    timedOut,
                }};
            "#
        );
        let value = evaluate_module(&source, &context).await.unwrap();
        server.abort();

        assert_eq!(
            value,
            json!({
                "statusCode": 201,
                "missingStatusCode": 404,
                "resolvedStatusCode": 404,
                "redirectStatusCode": 307,
                "followedStatusCode": 404,
                "header": "ok",
                "headerCount": 2,
                "length": 17,
                "text": "{\"received\":true}",
                "json": { "received": true },
                "slicedText": "abc",
                "invalidJsonError": "SyntaxError",
                "timedOut": true,
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn concurrent_slow_http_responses_do_not_busy_poll_the_script() {
        use std::{cell::Cell, future::Future, time::Duration};

        let app = Router::new().route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(80)).await;
                "done"
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = ScriptHttpClient::new(&startup_config::ScriptHttp {
            allow_private_network: true,
            ..Default::default()
        })
        .unwrap();
        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| register_http_client_to_context(&ctx, client))
            .await
            .unwrap();
        let source = format!(
            r#"
            const responses = await Promise.all([
                http.get("http://{address}/slow"),
                http.get("http://{address}/slow"),
            ]);
            export default responses.map(response => response.text());
        "#
        );
        let evaluation = evaluate_module(&source, &context);
        tokio::pin!(evaluation);
        let polls = Cell::new(0);
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            std::future::poll_fn(|cx| {
                polls.set(polls.get() + 1);
                evaluation.as_mut().poll(cx)
            }),
        )
        .await
        .unwrap()
        .unwrap();
        server.abort();
        assert_eq!(result, json!(["done", "done"]));
        assert!(
            polls.get() < 64,
            "slow HTTP caused {} script polls",
            polls.get()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_responses_larger_than_the_request_limit() {
        let app = Router::new().route("/large", get(|| async { "0123456789" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = ScriptHttpClient::new(&startup_config::ScriptHttp {
            allow_private_network: true,
            timeout: 5_000,
            max_response_size: 1024,
            max_redirects: 2,
        })
        .unwrap();
        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| register_http_client_to_context(&ctx, client))
            .await
            .unwrap();

        let source = format!(
            r#"
                await http.get("http://{address}/large", {{ maxResponseSize: 5 }});
                export default null;
            "#
        );
        let error = evaluate_module(&source, &context).await.unwrap_err();
        server.abort();

        assert!(error.to_string().contains("limit of 5 bytes"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_limits_cannot_exceed_server_configuration() {
        let client = ScriptHttpClient::new(&startup_config::ScriptHttp {
            allow_private_network: true,
            timeout: 500,
            max_response_size: 1024,
            max_redirects: 2,
        })
        .unwrap();
        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| register_http_client_to_context(&ctx, client))
            .await
            .unwrap();

        let error = evaluate_module(
            r#"
                await http.get("http://127.0.0.1/", { timeout: 501 });
                export default null;
            "#,
            &context,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("timeout must not exceed 500"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn blocks_private_dns_results_by_default() {
        let client = ScriptHttpClient::new(&startup_config::ScriptHttp::default()).unwrap();
        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| register_http_client_to_context(&ctx, client))
            .await
            .unwrap();

        let result = evaluate_module(
            r#"
                await http.get("http://localhost:1/");
                export default null;
            "#,
            &context,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unawaited_http_callbacks_finish_before_export_serialization() {
        let app = Router::new().route(
            "/",
            get(|| async {
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                "done"
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = ScriptHttpClient::new(&startup_config::ScriptHttp {
            allow_private_network: true,
            ..Default::default()
        })
        .unwrap();
        let (_runtime, context) = create_context().await;
        context
            .with(|ctx| register_http_client_to_context(&ctx, client))
            .await
            .unwrap();
        let source = format!(
            r#"
                const result = {{ value: "pending" }};
                http.get("http://{address}/").then(response => {{
                    result.value = response.text();
                }});
                export default result;
            "#
        );

        let result = evaluate_module(&source, &context).await.unwrap();
        server.abort();
        assert_eq!(result, json!({ "value": "done" }));
    }
}
