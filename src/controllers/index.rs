use axum::{body::Body, extract::State, http::Request, response::Response};

use crate::Context;

pub async fn index(State(ctx): State<Context>, request: Request<Body>) -> Response<Body> {
    if let Some(route) = {
        // https://rustcc.cn/article?id=ab4703a7-2130-4164-be40-f7a5cd325b09
        // 这里放到花括号里面是为了避免 guard 不穿越 .await
        ctx.dispatcher
            .read()
            .expect("lock poisoned")
            .dispatch(&request)
    } {
        if let Ok(response) = route.handler.handle(request).await {
            return response;
        } else {
            // TODO: 错误处理, 记录日志
        }
    }

    Response::builder().status(404).body(Body::empty()).unwrap()
}
