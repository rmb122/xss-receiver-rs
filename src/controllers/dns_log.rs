use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};

use crate::controllers::auth::AuthenticatedUser;
use crate::controllers::log_query::LogQuery;
use crate::{
    Context,
    controllers::AppError,
    db::dns_log::{helper::get_dns_logs_paginated, model::DnsLog},
    utils::response::Response,
};

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct PaginatedDnsLogResponse {
    pub data: Vec<DnsLog>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

/// List DNS logs matching the optional filter, newest first.
#[utoipa::path(get, path = "/", params(LogQuery), responses((status = OK, body = Response<PaginatedDnsLogResponse>)))]
pub async fn get_dns_logs(
    State(ctx): State<Context>,
    _user: AuthenticatedUser,
    Query(request): Query<LogQuery>,
) -> Result<Response<PaginatedDnsLogResponse>, AppError> {
    let filter = request.filter()?;
    let mut conn = ctx.db_conn().await?;
    let (logs, total) =
        get_dns_logs_paginated(&mut conn, request.page, request.page_size, &filter).await?;

    Ok(
        Response::<PaginatedDnsLogResponse>::ok().payload(PaginatedDnsLogResponse {
            data: logs,
            total,
            page: request.page,
            page_size: request.page_size,
        }),
    )
}
