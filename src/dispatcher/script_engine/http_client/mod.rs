use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use futures::StreamExt;
use reqwest::{Method, Url, header::HeaderMap, redirect::Policy};

use crate::startup_config;

mod bindings;
mod network_policy;

pub use bindings::register_http_client_to_context;
use network_policy::{FilteringResolver, validate_url};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ClientKey {
    tls_verify: bool,
    max_redirects: usize,
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
