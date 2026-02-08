use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::db::{
    http_log::model::{HttpLog, NewHttpLog},
    schema::http_log,
};

pub async fn insert_http_log(
    conn: &mut AsyncPgConnection,
    new_http_log: &NewHttpLog,
) -> anyhow::Result<i32> {
    return Ok(diesel::insert_into(http_log::table)
        .values(new_http_log)
        .returning(http_log::id)
        .get_result(conn)
        .await?);
}

// 分页查询 HTTP 日志
pub async fn get_http_logs_paginated(
    conn: &mut AsyncPgConnection,
    page: i64,
    page_size: i64,
) -> anyhow::Result<(Vec<HttpLog>, i64)> {
    // 获取总数
    let total: i64 = http_log::table.count().get_result(conn).await?;

    // 分页查询
    let offset = (page - 1) * page_size;
    let logs = http_log::table
        .select(HttpLog::as_select())
        .order(http_log::id.desc())
        .limit(page_size)
        .offset(offset)
        .get_results(conn)
        .await?;

    Ok((logs, total))
}
