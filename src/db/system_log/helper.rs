use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::db::{
    schema::system_log,
    system_log::model::{NewSystemLog, SystemLog},
};

pub async fn insert_system_log(conn: &mut AsyncPgConnection, log: &str) -> anyhow::Result<()> {
    let _: i32 = diesel::insert_into(system_log::table)
        .values(&NewSystemLog {
            log: log.to_owned(),
        })
        .returning(system_log::id)
        .get_result(conn)
        .await?;
    return Ok(());
}

// 分页查询系统日志
pub async fn get_system_logs_paginated(
    conn: &mut AsyncPgConnection,
    page: i64,
    page_size: i64,
) -> anyhow::Result<(Vec<SystemLog>, i64)> {
    // 获取总数
    let total: i64 = system_log::table.count().get_result(conn).await?;

    // 分页查询
    let offset = (page - 1) * page_size;
    let logs = system_log::table
        .select(SystemLog::as_select())
        .order(system_log::id.desc())
        .limit(page_size)
        .offset(offset)
        .get_results(conn)
        .await?;

    Ok((logs, total))
}
