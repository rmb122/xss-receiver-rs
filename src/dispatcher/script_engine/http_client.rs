use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use boa_engine::{
    Context, JsNativeError, JsObject, JsResult, JsValue, NativeFunction, js_string,
    object::{
        ObjectInitializer,
        builtins::{JsArray, JsUint8Array},
    },
    property::{Attribute, PropertyKey},
};
use boa_gc::{Finalize, Gc, Trace, empty_trace};
use futures::StreamExt;
use reqwest::{
    Method, Url,
    dns::{Addrs, Name, Resolve, Resolving},
    header::{HeaderMap, HeaderName, HeaderValue},
    redirect::Policy,
};

use crate::startup_config;

use super::helpers::{
    check_argument_count, ensure_exists, json_value_to_js_value, read_u8_array_from_js_value,
};

type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ClientKey {
    tls_verify: bool,
    max_redirects: usize,
}

#[derive(Debug)]
struct FilteringResolver {
    allow_private_network: bool,
}

impl Resolve for FilteringResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        let allow_private_network = self.allow_private_network;

        Box::pin(async move {
            let resolved = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let addresses: Vec<SocketAddr> = resolved
                .filter(|address| address_allowed(address.ip(), allow_private_network))
                .collect();

            if addresses.is_empty() {
                let error: BoxError = Box::new(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("host {host} did not resolve to an allowed address"),
                ));
                return Err(error);
            }

            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

#[derive(Clone)]
pub struct ScriptHttpClient {
    config: Arc<startup_config::ScriptHttp>,
    resolver: Arc<FilteringResolver>,
}

impl ScriptHttpClient {
    pub fn new(config: &startup_config::ScriptHttp) -> anyhow::Result<Self> {
        let client = Self {
            config: Arc::new(config.clone()),
            resolver: Arc::new(FilteringResolver {
                allow_private_network: config.allow_private_network,
            }),
        };

        // Build the default client eagerly so invalid TLS/client configuration fails at startup.
        client.build_client(true, config.max_redirects)?;
        Ok(client)
    }

    fn build_client(
        &self,
        tls_verify: bool,
        max_redirects: usize,
    ) -> anyhow::Result<reqwest::Client> {
        let redirect = if max_redirects == 0 {
            Policy::none()
        } else {
            let limited = Policy::limited(max_redirects);
            let allow_private_network = self.config.allow_private_network;
            Policy::custom(move |attempt| {
                if let Err(error) = validate_url(attempt.url(), allow_private_network) {
                    attempt.error(error)
                } else {
                    limited.redirect(attempt)
                }
            })
        };

        reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(self.resolver.clone())
            .redirect(redirect)
            .danger_accept_invalid_certs(!tls_verify)
            .build()
            .map_err(Into::into)
    }

    fn client_for(
        &self,
        clients: &RefCell<HashMap<ClientKey, reqwest::Client>>,
        tls_verify: bool,
        max_redirects: usize,
    ) -> anyhow::Result<reqwest::Client> {
        let key = ClientKey {
            tls_verify,
            max_redirects,
        };
        if let Some(client) = clients.borrow().get(&key) {
            return Ok(client.clone());
        }

        let client = self.build_client(tls_verify, max_redirects)?;
        clients.borrow_mut().insert(key, client.clone());
        Ok(client)
    }

    async fn execute(
        &self,
        client: reqwest::Client,
        request: OutboundRequest,
    ) -> Result<BufferedResponse, HttpClientError> {
        validate_url(&request.url, self.config.allow_private_network)
            .map_err(HttpClientError::Request)?;

        let mut builder = client
            .request(request.method, request.url)
            .headers(request.headers)
            .timeout(request.timeout);
        if let Some(body) = request.body {
            builder = builder.body(body);
        }

        let response = builder.send().await?;
        if response
            .content_length()
            .is_some_and(|size| size > request.max_response_size)
        {
            return Err(HttpClientError::ResponseTooLarge {
                limit: request.max_response_size,
            });
        }

        let status_code = response.status().as_u16();
        let url = response.url().to_string();
        let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, value) in response.headers() {
            headers
                .entry(name.as_str().to_owned())
                .or_default()
                .push(String::from_utf8_lossy(value.as_bytes()).into_owned());
        }

        let initial_capacity = response
            .content_length()
            .unwrap_or(0)
            .min(request.max_response_size)
            .min(64 * 1024) as usize;
        let mut body = Vec::with_capacity(initial_capacity);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let next_size = body.len() as u64 + chunk.len() as u64;
            if next_size > request.max_response_size {
                return Err(HttpClientError::ResponseTooLarge {
                    limit: request.max_response_size,
                });
            }
            body.extend_from_slice(&chunk);
        }

        Ok(BufferedResponse {
            status_code,
            url,
            headers,
            body,
        })
    }
}

#[derive(Debug, thiserror::Error)]
enum HttpClientError {
    #[error("{0}")]
    Request(String),
    #[error("HTTP response exceeds the configured limit of {limit} bytes")]
    ResponseTooLarge { limit: u64 },
    #[error(transparent)]
    Transport(#[from] reqwest::Error),
}

impl HttpClientError {
    fn into_js_error(self) -> boa_engine::JsError {
        match self {
            Self::ResponseTooLarge { .. } => {
                JsNativeError::range().with_message(self.to_string()).into()
            }
            Self::Request(_) | Self::Transport(_) => {
                JsNativeError::error().with_message(self.to_string()).into()
            }
        }
    }
}

struct OutboundRequest {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
    timeout: Duration,
    max_response_size: u64,
    max_redirects: usize,
    tls_verify: bool,
}

struct BufferedResponse {
    status_code: u16,
    url: String,
    headers: BTreeMap<String, Vec<String>>,
    body: Vec<u8>,
}

struct ScriptHttpClientCell {
    client: ScriptHttpClient,
    clients: RefCell<HashMap<ClientKey, reqwest::Client>>,
}

impl Finalize for ScriptHttpClientCell {}

// SAFETY: the cell contains only Rust-owned HTTP client state and no Boa GC-managed values.
unsafe impl Trace for ScriptHttpClientCell {
    empty_trace!();
}

struct ResponseBodyCell {
    body: Vec<u8>,
}

impl Finalize for ResponseBodyCell {}

// SAFETY: the response body contains no Boa GC-managed values.
unsafe impl Trace for ResponseBodyCell {
    empty_trace!();
}

fn response_text_fn(
    _this: &JsValue,
    _args: &[JsValue],
    body: &Gc<ResponseBodyCell>,
    _ctx: &mut Context,
) -> JsResult<JsValue> {
    Ok(JsValue::from(js_string!(
        String::from_utf8_lossy(&body.body).as_ref()
    )))
}

fn response_json_fn(
    _this: &JsValue,
    _args: &[JsValue],
    body: &Gc<ResponseBodyCell>,
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let value: serde_json::Value = serde_json::from_slice(&body.body).map_err(|error| {
        JsNativeError::syntax().with_message(format!("invalid JSON response body: {error}"))
    })?;
    json_value_to_js_value(&value, ctx)
}

fn buffered_response_to_js(response: BufferedResponse, ctx: &mut Context) -> JsResult<JsValue> {
    let body = Gc::new(ResponseBodyCell {
        body: response.body,
    });
    let uint8_array = JsUint8Array::from_iter(body.body.iter().copied(), ctx)?;

    let headers = JsObject::with_null_proto();
    for (name, values) in response.headers {
        let array = JsArray::new(ctx);
        for (index, value) in values.into_iter().enumerate() {
            array.set(index, js_string!(value), false, ctx)?;
        }
        headers.set(js_string!(name), array, false, ctx)?;
    }

    let object = ObjectInitializer::new(ctx)
        .property(
            js_string!("statusCode"),
            response.status_code,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("url"),
            js_string!(response.url),
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("headers"),
            headers,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .property(
            js_string!("body"),
            uint8_array,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .function(
            NativeFunction::from_copy_closure_with_captures(response_text_fn, body.clone()),
            js_string!("text"),
            0,
        )
        .function(
            NativeFunction::from_copy_closure_with_captures(response_json_fn, body),
            js_string!("json"),
            0,
        )
        .build();

    Ok(object.into())
}

fn parse_request(
    args: &[JsValue],
    forced_method: Option<&'static str>,
    client: &ScriptHttpClient,
    ctx: &mut Context,
) -> JsResult<OutboundRequest> {
    check_argument_count(args, 1)?;
    let url = ensure_exists(args[0].as_string(), "URL must be a string")?.to_std_string_lossy();
    let url = Url::parse(&url)
        .map_err(|error| JsNativeError::typ().with_message(format!("invalid URL: {error}")))?;

    let options = match args.get(1) {
        None => None,
        Some(value) if value.is_undefined() => None,
        Some(value) => Some(
            value
                .as_object()
                .ok_or_else(|| JsNativeError::typ().with_message("options must be an object"))?,
        ),
    };

    let method = if let Some(method) = forced_method {
        Method::from_bytes(method.as_bytes()).expect("fixed HTTP method must be valid")
    } else if let Some(ref options) = options {
        let value = options.get(js_string!("method"), ctx)?;
        if value.is_undefined() {
            Method::GET
        } else {
            let value =
                ensure_exists(value.as_string(), "method must be a string")?.to_std_string_lossy();
            Method::from_bytes(value.as_bytes()).map_err(|error| {
                JsNativeError::typ().with_message(format!("invalid HTTP method: {error}"))
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
        let value = options.get(js_string!("headers"), ctx)?;
        if !value.is_undefined() {
            headers = parse_headers(&value, ctx)?;
        }

        let value = options.get(js_string!("body"), ctx)?;
        if !value.is_undefined() {
            body = Some(read_u8_array_from_js_value(&value, ctx)?);
        }

        timeout = parse_bounded_integer(
            options.get(js_string!("timeout"), ctx)?,
            "timeout",
            timeout,
            client.config.timeout,
            false,
        )?;
        max_response_size = parse_bounded_integer(
            options.get(js_string!("maxResponseSize"), ctx)?,
            "maxResponseSize",
            max_response_size,
            client.config.max_response_size,
            false,
        )?;
        max_redirects = parse_bounded_integer(
            options.get(js_string!("maxRedirects"), ctx)?,
            "maxRedirects",
            max_redirects as u64,
            client.config.max_redirects as u64,
            true,
        )? as usize;

        let value = options.get(js_string!("tlsVerify"), ctx)?;
        if !value.is_undefined() {
            tls_verify = ensure_exists(value.as_boolean(), "tlsVerify must be a boolean")?;
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

fn parse_headers(value: &JsValue, ctx: &mut Context) -> JsResult<HeaderMap> {
    let object = value
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("headers must be an object"))?;
    let mut headers = HeaderMap::new();

    for key in object.own_property_keys(ctx)? {
        let name = match &key {
            PropertyKey::String(name) => name.to_std_string_lossy(),
            PropertyKey::Index(index) => index.get().to_string(),
            PropertyKey::Symbol(_) => continue,
        };
        let header_name = HeaderName::from_str(&name).map_err(|error| {
            JsNativeError::typ().with_message(format!("invalid HTTP header name {name:?}: {error}"))
        })?;
        let value = object.get(key, ctx)?;

        if let Some(value) = value.as_string() {
            append_header(&mut headers, header_name, &value.to_std_string_lossy())?;
            continue;
        }

        let array = JsArray::from_object(
            value
                .as_object()
                .ok_or_else(|| {
                    JsNativeError::typ()
                        .with_message(format!("header {name:?} must be a string or string array"))
                })?
                .clone(),
        )?;
        for index in 0..array.length(ctx)? {
            let value = array.get(index, ctx)?;
            let value = ensure_exists(
                value.as_string(),
                &format!("header {name:?} array item {index} must be a string"),
            )?;
            append_header(
                &mut headers,
                header_name.clone(),
                &value.to_std_string_lossy(),
            )?;
        }
    }

    Ok(headers)
}

fn append_header(headers: &mut HeaderMap, name: HeaderName, value: &str) -> JsResult<()> {
    let value = HeaderValue::from_str(value).map_err(|error| {
        JsNativeError::typ().with_message(format!("invalid value for header {name}: {error}"))
    })?;
    headers.append(name, value);
    Ok(())
}

fn parse_bounded_integer(
    value: JsValue,
    name: &str,
    default: u64,
    maximum: u64,
    allow_zero: bool,
) -> JsResult<u64> {
    if value.is_undefined() {
        return Ok(default);
    }

    let number = ensure_exists(value.as_number(), &format!("{name} must be a number"))?;
    let minimum = if allow_zero { 0.0 } else { 1.0 };
    if !number.is_finite() || number.fract() != 0.0 || number < minimum {
        return Err(JsNativeError::range()
            .with_message(format!(
                "{name} must be an integer greater than or equal to {}",
                minimum as u64
            ))
            .into());
    }
    if number > maximum as f64 {
        return Err(JsNativeError::range()
            .with_message(format!("{name} must not exceed {maximum}"))
            .into());
    }
    Ok(number as u64)
}

async fn request_impl(
    forced_method: Option<&'static str>,
    args: &[JsValue],
    context: &RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let (client, request_client, request) = {
        let mut context = context.borrow_mut();
        let cell = ensure_exists(
            context.get_data::<Gc<ScriptHttpClientCell>>().cloned(),
            "failed to get script HTTP client from context",
        )?;
        let request = parse_request(args, forced_method, &cell.client, &mut context)?;
        let request_client = cell
            .client
            .client_for(&cell.clients, request.tls_verify, request.max_redirects)
            .map_err(|error| JsNativeError::error().with_message(error.to_string()))?;
        (cell.client.clone(), request_client, request)
    };

    let response = client
        .execute(request_client, request)
        .await
        .map_err(HttpClientError::into_js_error)?;
    buffered_response_to_js(response, &mut context.borrow_mut())
}

async fn request_fn(
    _this: &JsValue,
    args: &[JsValue],
    context: &RefCell<&mut Context>,
) -> JsResult<JsValue> {
    request_impl(None, args, context).await
}

macro_rules! method_function {
    ($name:ident, $method:literal) => {
        async fn $name(
            _this: &JsValue,
            args: &[JsValue],
            context: &RefCell<&mut Context>,
        ) -> JsResult<JsValue> {
            request_impl(Some($method), args, context).await
        }
    };
}

method_function!(get_fn, "GET");
method_function!(post_fn, "POST");
method_function!(put_fn, "PUT");
method_function!(patch_fn, "PATCH");
method_function!(delete_fn, "DELETE");
method_function!(head_fn, "HEAD");

pub fn register_http_client_to_context(context: &mut Context, client: ScriptHttpClient) {
    context.insert_data(Gc::new(ScriptHttpClientCell {
        client,
        clients: RefCell::new(HashMap::new()),
    }));

    let object = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_async_fn(request_fn),
            js_string!("request"),
            2,
        )
        .function(NativeFunction::from_async_fn(get_fn), js_string!("get"), 2)
        .function(
            NativeFunction::from_async_fn(post_fn),
            js_string!("post"),
            2,
        )
        .function(NativeFunction::from_async_fn(put_fn), js_string!("put"), 2)
        .function(
            NativeFunction::from_async_fn(patch_fn),
            js_string!("patch"),
            2,
        )
        .function(
            NativeFunction::from_async_fn(delete_fn),
            js_string!("delete"),
            2,
        )
        .function(
            NativeFunction::from_async_fn(head_fn),
            js_string!("head"),
            2,
        )
        .build();

    context
        .register_global_property(
            js_string!("http"),
            object,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .expect("http property shouldn't exist");
}

fn validate_url(url: &Url, allow_private_network: bool) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err("only absolute http:// and https:// URLs are supported".to_owned());
    }

    let host = url
        .host()
        .ok_or_else(|| "URL must contain a host".to_owned())?;
    let ip = match host {
        url::Host::Ipv4(ip) => Some(IpAddr::V4(ip)),
        url::Host::Ipv6(ip) => Some(IpAddr::V6(ip)),
        url::Host::Domain(_) => None,
    };

    if ip.is_some_and(|ip| !address_allowed(ip, allow_private_network)) {
        return Err(format!("target address {} is not allowed", ip.unwrap()));
    }
    Ok(())
}

fn address_allowed(ip: IpAddr, allow_private_network: bool) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            if ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast() {
                return false;
            }
            allow_private_network || is_public_ipv4(ip)
        }
        IpAddr::V6(ip) => {
            if ip.is_unspecified() || ip.is_multicast() {
                return false;
            }
            if let Some(ipv4) = ip.to_ipv4_mapped() {
                return address_allowed(IpAddr::V4(ipv4), allow_private_network);
            }
            allow_private_network || is_public_ipv6(ip)
        }
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let value = u32::from(ip);
    ![
        ("0.0.0.0", 8),
        ("10.0.0.0", 8),
        ("100.64.0.0", 10),
        ("127.0.0.0", 8),
        ("169.254.0.0", 16),
        ("172.16.0.0", 12),
        ("192.0.0.0", 24),
        ("192.0.2.0", 24),
        ("192.88.99.0", 24),
        ("192.168.0.0", 16),
        ("198.18.0.0", 15),
        ("198.51.100.0", 24),
        ("203.0.113.0", 24),
        ("224.0.0.0", 4),
        ("240.0.0.0", 4),
    ]
    .into_iter()
    .any(|(network, prefix)| {
        let network = u32::from(Ipv4Addr::from_str(network).expect("valid network constant"));
        let mask = u32::MAX << (32 - prefix);
        value & mask == network & mask
    })
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();

    // Currently allocated global unicast space is 2000::/3.
    if segments[0] & 0xe000 != 0x2000 {
        return false;
    }
    // IETF protocol assignments (2001::/23), documentation ranges, and 3fff::/20.
    if segments[0] == 0x2001 && segments[1] <= 0x01ff {
        return false;
    }
    if segments[0] == 0x2001 && segments[1] == 0x0db8 {
        return false;
    }
    // 6to4 embeds an IPv4 tunnel endpoint and must not bypass the IPv4 policy.
    if segments[0] == 0x2002 {
        return false;
    }
    if segments[0] == 0x3fff && segments[1] & 0xf000 == 0 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use axum::{
        Router,
        http::StatusCode,
        response::{AppendHeaders, Redirect},
        routing::{get, post},
    };
    use boa_gc::Gc;
    use serde_json::json;

    use super::{
        ScriptHttpClient, ScriptHttpClientCell, address_allowed, register_http_client_to_context,
        validate_url,
    };
    use crate::{
        dispatcher::script_engine::{create_context, evaluate_module},
        startup_config,
    };

    #[test]
    fn blocks_non_public_addresses_by_default() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "169.254.169.254",
            "192.168.1.1",
            "198.18.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:7f00:1::",
            "2002:808:808::",
            "::ffff:127.0.0.1",
        ] {
            assert!(!address_allowed(address.parse::<IpAddr>().unwrap(), false));
        }
    }

    #[test]
    fn allows_public_addresses_by_default() {
        for address in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(address_allowed(address.parse::<IpAddr>().unwrap(), false));
        }
    }

    #[test]
    fn client_pools_are_scoped_to_one_script_context() {
        let client = ScriptHttpClient::new(&startup_config::ScriptHttp::default()).unwrap();
        let (mut first_context, _) = create_context();
        let (mut second_context, _) = create_context();
        register_http_client_to_context(&mut first_context, client.clone());
        register_http_client_to_context(&mut second_context, client);

        let first_cell = first_context
            .get_data::<Gc<ScriptHttpClientCell>>()
            .unwrap()
            .clone();
        let second_cell = second_context
            .get_data::<Gc<ScriptHttpClientCell>>()
            .unwrap()
            .clone();

        first_cell
            .client
            .client_for(&first_cell.clients, true, 0)
            .unwrap();

        assert_eq!(first_cell.clients.borrow().len(), 1);
        assert!(second_cell.clients.borrow().is_empty());
    }

    #[test]
    fn private_network_mode_still_rejects_non_unicast_targets() {
        assert!(address_allowed("127.0.0.1".parse().unwrap(), true));
        assert!(address_allowed("::1".parse().unwrap(), true));
        assert!(!address_allowed("0.0.0.0".parse().unwrap(), true));
        assert!(!address_allowed("ff02::1".parse().unwrap(), true));
    }

    #[test]
    fn blocks_obscured_private_ip_literals() {
        for url in [
            "http://127.1/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://0177.0.0.1/",
            "http://[::ffff:127.0.0.1]/",
        ] {
            let url = reqwest::Url::parse(url).unwrap();
            assert!(validate_url(&url, false).is_err(), "URL was allowed: {url}");
        }
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
        let (mut context, executor) = create_context();
        register_http_client_to_context(&mut context, client);

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
        let value = evaluate_module(&source, &mut context, executor)
            .await
            .unwrap();
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
        let (mut context, executor) = create_context();
        register_http_client_to_context(&mut context, client);

        let source = format!(
            r#"
                await http.get("http://{address}/large", {{ maxResponseSize: 5 }});
                export default null;
            "#
        );
        let error = evaluate_module(&source, &mut context, executor)
            .await
            .unwrap_err();
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
        let (mut context, executor) = create_context();
        register_http_client_to_context(&mut context, client);

        let error = evaluate_module(
            r#"
                await http.get("http://127.0.0.1/", { timeout: 501 });
                export default null;
            "#,
            &mut context,
            executor,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("timeout must not exceed 500"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn blocks_private_dns_results_by_default() {
        let client = ScriptHttpClient::new(&startup_config::ScriptHttp::default()).unwrap();
        let (mut context, executor) = create_context();
        register_http_client_to_context(&mut context, client);

        let result = evaluate_module(
            r#"
                await http.get("http://localhost:1/");
                export default null;
            "#,
            &mut context,
            executor,
        )
        .await;

        assert!(result.is_err());
    }
}
