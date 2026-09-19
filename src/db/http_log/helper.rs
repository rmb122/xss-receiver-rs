use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::db::{
    http_log::model::{HttpLog, NewHttpLog},
    log_filter::LogFilter,
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

pub async fn get_http_logs_paginated(
    conn: &mut AsyncPgConnection,
    page: i64,
    page_size: i64,
    filter: &LogFilter,
) -> anyhow::Result<(Vec<HttpLog>, i64)> {
    let filtered = || -> anyhow::Result<_> {
        let mut query = http_log::table.into_boxed();
        if let Some(predicate) = filter.http()? {
            query = query.filter(predicate);
        }
        Ok(query)
    };
    let total: i64 = filtered()?.count().get_result(conn).await?;
    let offset = (page - 1) * page_size;
    let logs = filtered()?
        .select(HttpLog::as_select())
        .order(http_log::id.desc())
        .limit(page_size)
        .offset(offset)
        .get_results(conn)
        .await?;

    Ok((logs, total))
}

pub async fn get_http_log_raw_body(
    conn: &mut AsyncPgConnection,
    id: i32,
) -> anyhow::Result<Option<Vec<u8>>> {
    Ok(http_log::table
        .find(id)
        .select(http_log::raw_body)
        .first::<Vec<u8>>(conn)
        .await
        .optional()?)
}
