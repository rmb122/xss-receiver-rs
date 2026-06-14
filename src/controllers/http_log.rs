use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::controllers::user::LoggedUser;
use crate::{
    Context,
    controllers::AppError,
    db::http_log::{
        helper::{get_http_log_raw_body, get_http_logs_paginated},
        model::HttpLog,
    },
    utils::{jwt::Claims, response::Response},
};

// 分页响应
#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct PaginatedHttpLogResponse {
    pub data: Vec<HttpLog>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

// 分页查询请求
#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct PaginatedRequest {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    20
}

// 分页查询 HTTP 日志
#[utoipa::path(get, path = "/", params(PaginatedRequest), responses((status = OK, body = Response<PaginatedHttpLogResponse>)))]
pub async fn get_http_logs(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Query(request): Query<PaginatedRequest>,
) -> Result<Response<PaginatedHttpLogResponse>, AppError> {
    // 验证分页参数
    if request.page < 1 {
        return Err(anyhow::anyhow!("page must be greater than 0").into());
    }
    if request.page_size < 1 || request.page_size > 500 {
        return Err(anyhow::anyhow!("page_size must be between 1 and 500").into());
    }

    let mut conn = ctx.db_conn().await?;
    let (logs, total) = get_http_logs_paginated(&mut conn, request.page, request.page_size).await?;

    Ok(
        Response::<PaginatedHttpLogResponse>::ok().payload(PaginatedHttpLogResponse {
            data: logs,
            total,
            page: request.page,
            page_size: request.page_size,
        }),
    )
}

#[utoipa::path(
    get,
    path = "/{id}/raw_body",
    params(("id" = i32, Path, description = "HTTP log id")),
    responses((status = OK, description = "原始请求体", body = Vec<u8>))
)]
pub async fn get_http_log_raw_body_response(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, AppError> {
    let mut conn = ctx.db_conn().await?;
    let body = get_http_log_raw_body(&mut conn, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("http log not found"))?;

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/octet-stream")],
        Body::from(body),
    ))
}
