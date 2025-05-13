use std::{collections::HashMap, net::SocketAddr};

use axum::{body::Body, extract::Query, http::Request};
use futures::TryStreamExt;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

pub enum ParsedRequestBody {
    None,
    Raw(Vec<u8>),
    Form(HashMap<String, Vec<String>>),
    Json(Value),
    // File(HashMap<String, Multipart>),
}

pub struct ParsedRequest {
    pub client_addr: SocketAddr,
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, Vec<String>>,
    pub params: HashMap<String, Vec<String>>,

    pub parsed_body: ParsedRequestBody,
}

impl ParsedRequest {
    pub async fn new(client_addr: SocketAddr, request: Request<Body>) -> anyhow::Result<Self> {
        let mut parsed_request = ParsedRequest {
            client_addr,
            method: request.method().to_string(),
            path: request.uri().to_string(),
            headers: request
                .headers()
                .iter()
                .fold(HashMap::new(), |mut headers, kv| {
                    let key = Self::unify_header_key(kv.0.to_string());
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
            parsed_body: ParsedRequestBody::None,
        };

        let mut body = Vec::new();
        StreamReader::new(
            TryStreamExt::map_err(request.into_body().into_data_stream(), |err| {
                std::io::Error::other(err)
            })
            .into_stream(),
        )
        .read_to_end(&mut body)
        .await?;

        parsed_request.parse_body(body);

        Ok(parsed_request)
    }

    fn unify_header_key(key: String) -> String {
        let mut upper_case = true;
        let mut key_bytes = key.into_bytes();
        key_bytes.iter_mut().for_each(|x| {
            if upper_case {
                upper_case = false;

                *x = x.to_ascii_uppercase()
            }

            if *x == b'-' {
                upper_case = true;
            }
        });

        unsafe { String::from_utf8_unchecked(key_bytes) }
    }

    fn parse_content_type(content_type: &str) -> (String, HashMap<String, String>) {
        let contnet_type = content_type.to_ascii_lowercase();

        let parsed_content_type;
        let mut attrs = HashMap::new();

        let parts: Vec<&str> = contnet_type.split(";").collect();
        parsed_content_type = parts[0].to_owned();

        for part in parts.iter().skip(1) {
            if let Some((left, right)) = part.split_once("=") {
                attrs.insert((*left).to_owned(), (*right).to_owned());
            } else {
                attrs.insert((*part).to_owned(), String::new());
            }
        }

        (parsed_content_type, attrs)
    }

    fn parse_body(&mut self, body: Vec<u8>) {
        let content_type = match self.headers.get("Content-Type") {
            Some(content_type) => content_type,
            None => {
                // 没有 content-type 直接作为 raw
                self.parsed_body = ParsedRequestBody::Raw(body);
                return;
            }
        };

        let (content_type, attrs) = Self::parse_content_type(&content_type[0]);
        if content_type.ends_with("json") {}
    }
}
