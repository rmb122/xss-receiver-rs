use std::{collections::HashMap, net::SocketAddr};

use axum::{body::Body, extract::Query, http::Request};

pub struct ParsedRequest {
    pub client_addr: SocketAddr,
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, Vec<String>>,
    pub params: HashMap<String, Vec<String>>,
    pub body: Body,
}

impl ParsedRequest {
    pub fn from(client_addr: SocketAddr, request: Request<Body>) -> Self {
        ParsedRequest {
            client_addr,
            method: request.method().to_string(),
            path: request.uri().to_string(),
            headers: request
                .headers()
                .iter()
                .fold(HashMap::new(), |mut headers, kv| {
                    let key = kv.0.to_string();
                    let value = String::from_utf8_lossy(kv.1.as_bytes()).to_string();
                    if let Some(entry) = headers.get_mut(&key) {
                        entry.push(value);
                    } else {
                        headers.insert(key, vec![value]);
                    };

                    headers
                }),
            params: if let Ok(query) =
                Query::<HashMap<String, Vec<String>>>::try_from_uri(request.uri())
            {
                query.0
            } else {
                HashMap::new()
            },
            body: request.into_body(),
        }
    }
}
