use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::controllers::user::LoggedUser;
use crate::{
    Context,
    controllers::AppError,
    db::dns_log::{helper::get_dns_logs_paginated, model::DnsLog},
    utils::{jwt::Claims, response::Response},
};

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct PaginatedDnsLogResponse {
    pub data: Vec<DnsLog>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

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

#[utoipa::path(get, path = "/", params(PaginatedRequest), responses((status = OK, body = Response<PaginatedDnsLogResponse>)))]
pub async fn get_dns_logs(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Query(request): Query<PaginatedRequest>,
) -> Result<Response<PaginatedDnsLogResponse>, AppError> {
    if request.page < 1 {
        return Err(anyhow::anyhow!("page must be greater than 0").into());
    }
    if request.page_size < 1 || request.page_size > 500 {
        return Err(anyhow::anyhow!("page_size must be between 1 and 500").into());
    }

    let mut conn = ctx.db_conn().await?;
    let (logs, total) = get_dns_logs_paginated(&mut conn, request.page, request.page_size).await?;

    Ok(
        Response::<PaginatedDnsLogResponse>::ok().payload(PaginatedDnsLogResponse {
            data: logs,
            total,
            page: request.page,
            page_size: request.page_size,
        }),
    )
}
