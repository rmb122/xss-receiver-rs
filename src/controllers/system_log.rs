use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::controllers::user::LoggedUser;
use crate::{
    Context,
    controllers::AppError,
    db::system_log::{helper::get_system_logs_paginated, model::SystemLog},
    utils::{jwt::Claims, response::Response},
};

// 分页响应
#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct PaginatedSystemLogResponse {
    pub data: Vec<SystemLog>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

// 分页查询请求
#[derive(Deserialize, utoipa::ToSchema)]
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

// 分页查询系统日志
#[utoipa::path(get, path = "/", responses((status = OK, body = Response<PaginatedSystemLogResponse>)))]
pub async fn get_system_logs(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<PaginatedRequest>,
) -> Result<Response<PaginatedSystemLogResponse>, AppError> {
    // 验证分页参数
    if request.page < 1 {
        return Err(anyhow::anyhow!("page must be greater than 0").into());
    }
    if request.page_size < 1 || request.page_size > 500 {
        return Err(anyhow::anyhow!("page_size must be between 1 and 500").into());
    }

    let mut conn = ctx.db_conn().await?;
    let (logs, total) =
        get_system_logs_paginated(&mut conn, request.page, request.page_size).await?;

    Ok(Response::<PaginatedSystemLogResponse>::ok()
        .msg("system logs retrieved successfully")
        .payload(PaginatedSystemLogResponse {
            data: logs,
            total,
            page: request.page,
            page_size: request.page_size,
        }))
}
