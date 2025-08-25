use std::net::SocketAddr;

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::Request,
    response::Response,
};

use crate::{Context, parsed_request::ParsedRequest};

pub async fn handle_error(error: anyhow::Error) {
    // TODO: 错误处理, 记录日志
    println!("{:?}", error);
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

pub async fn index(
    State(ctx): State<Context>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
) -> Response<Body> {
    let real_addr: SocketAddr = if let Ok(real_addr) =
        get_real_addr_from_request(&ctx.startup_config.http_server.real_addr_header, &request)
    {
        real_addr
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
        let error = match ParsedRequest::new(real_addr, request).await {
            Ok(request) => match route.handler.handle(request).await {
                Ok(response) => return response,
                Err(error) => error,
            },
            Err(error) => error,
        };

        tokio::spawn(handle_error(error));
    };

    Response::builder().status(404).body(Body::empty()).unwrap()
}
