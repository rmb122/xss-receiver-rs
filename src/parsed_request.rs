use std::{borrow::Cow, collections::HashMap, net::SocketAddr};

use axum::{body::Body, extract::Query, http::Request};
use encoding_rs::Encoding;
use futures::TryStreamExt;
use multer::Multipart;
use multimap::MultiMap;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio_util::io::{ReaderStream, StreamReader};

#[derive(Debug)]
pub enum ParsedRequestBody {
    None,
    Form(
        MultiMap<String, String>,
        MultiMap<String, (String, Vec<u8>)>,
    ), // (普通的 post form, file)
    Json(Value),
}

#[derive(Debug)]
pub struct ParsedRequest {
    pub client_addr: SocketAddr,
    pub method: String,
    pub path: String,
    pub headers: MultiMap<String, String>,
    pub params: MultiMap<String, String>,

    pub body: Vec<u8>,
    pub parsed_body: ParsedRequestBody,
}

impl ParsedRequest {
    pub async fn new(client_addr: SocketAddr, request: Request<Body>) -> anyhow::Result<Self> {
        let mut parsed_request = ParsedRequest {
            client_addr,
            method: request.method().to_string(),
            path: request.uri().path().to_string(),
            headers: request
                .headers()
                .iter()
                .fold(MultiMap::new(), |mut headers, kv| {
                    let key = Self::unify_header_key(kv.0.to_string());
                    let value = String::from_utf8_lossy(kv.1.as_bytes()).to_string();
                    headers.insert(key, value);
                    headers
                }),
            params: {
                println!(
                    "{:?}",
                    Query::<HashMap<String, Vec<String>>>::try_from_uri(request.uri())
                );

                if let Ok(query) =
                    Query::<HashMap<String, Vec<String>>>::try_from_uri(request.uri())
                {
                    //query.0

                    println!("{:?}", query);
                    MultiMap::new()
                } else {
                    MultiMap::new()
                }
            },
            body: Vec::new(),
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

        parsed_request.parse_body(&body).await;
        parsed_request.body = body;

        println!("{:?}", parsed_request);
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
        let parsed_content_type;
        let mut attrs = HashMap::new();

        let parts: Vec<&str> = content_type.split(";").map(|x| x.trim()).collect();
        parsed_content_type = parts[0].to_owned().to_lowercase();

        for part in parts.iter().skip(1) {
            if part.len() == 0 {
                continue;
            }

            if let Some((left, mut right)) = part.split_once("=") {
                if right.len() >= 2
                    && ((right.starts_with("'") && right.ends_with("'"))
                        || (right.starts_with('"') && right.ends_with('"')))
                {
                    right = &right[1..right.len() - 1];
                }

                attrs.insert((*left).to_owned().to_ascii_lowercase(), (*right).to_owned());
            } else {
                attrs.insert((*part).to_owned().to_ascii_lowercase(), String::new());
            }
        }

        (parsed_content_type, attrs)
    }

    fn try_decode<T: AsRef<str>>(content: &[u8], charset: Option<T>) -> Cow<[u8]> {
        let charset = match charset {
            None => return Cow::Borrowed(content),
            Some(charset) => charset,
        };

        if let Some(encoding) =
            Encoding::for_label(charset.as_ref().to_ascii_lowercase().as_bytes())
        {
            return Cow::Owned(String::from(encoding.decode_with_bom_removal(content).0).into());
        } else {
            return Cow::Borrowed(content);
        }
    }

    async fn parse_body(&mut self, body: &[u8]) {
        let content_type = match self.headers.get("Content-Type") {
            Some(content_type) => content_type,
            None => {
                // 没有 content-type 不进行解析
                return;
            }
        };

        let (content_type, attrs) = Self::parse_content_type(content_type);

        // try json
        if content_type.ends_with("/json") || content_type.ends_with("+json") {
            let body = Self::try_decode(body, attrs.get("charset"));

            if let Ok(value) = serde_json::from_slice::<Value>(&body) {
                self.parsed_body = ParsedRequestBody::Json(value);
                return;
            }
        }

        // try x-www-form-urlencoded
        if content_type.ends_with("/x-www-form-urlencoded") {
            let body = Self::try_decode(body, attrs.get("charset"));

            let mut form = MultiMap::<String, String>::new();
            for (key, value) in form_urlencoded::parse(&body) {
                form.insert(key.to_string(), value.to_string());
            }
            self.parsed_body = ParsedRequestBody::Form(form, MultiMap::new());
            return;
        }

        // try form-data
        if let (true, Some(boundary)) =
            (content_type == "multipart/form-data", attrs.get("boundary"))
        {
            let mut multipart = Multipart::new(ReaderStream::new(body), boundary);
            let mut form = MultiMap::<String, String>::new();
            let mut file = MultiMap::<String, (String, Vec<u8>)>::new();

            while let Ok(Some(mut field)) = multipart.next_field().await {
                let name = if let Some(name) = field.name() {
                    name.to_owned()
                } else {
                    continue;
                };

                let filename = field.file_name().map(|x| x.to_owned());
                let charset = if let Some(content_type) = field.content_type() {
                    content_type
                        .params()
                        .find(|x| x.0.as_str().eq_ignore_ascii_case("charset"))
                        .map(|x| x.1.as_str().to_owned())
                } else {
                    None
                };
                let mut content = Vec::new();

                while let Ok(Some(chunk)) = field.chunk().await {
                    content.extend(chunk);
                }

                match filename {
                    None => {
                        let value = Self::try_decode(&content, charset);
                        form.insert(
                            name.to_string(),
                            String::from(String::from_utf8_lossy(&value)),
                        );
                    }
                    Some(filename) => {
                        file.insert(name.to_string(), (filename, content));
                    }
                }
            }

            self.parsed_body = ParsedRequestBody::Form(form, file);
            return;
        }
    }
}
