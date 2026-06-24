use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::db::{
    dns_log::model::{DnsLog, NewDnsLog},
    schema::dns_log,
};

pub async fn insert_dns_log(
    conn: &mut AsyncPgConnection,
    new_dns_log: &NewDnsLog,
) -> anyhow::Result<i32> {
    Ok(diesel::insert_into(dns_log::table)
        .values(new_dns_log)
        .returning(dns_log::id)
        .get_result(conn)
        .await?)
}

pub async fn get_dns_logs_paginated(
    conn: &mut AsyncPgConnection,
    page: i64,
    page_size: i64,
) -> anyhow::Result<(Vec<DnsLog>, i64)> {
    let total: i64 = dns_log::table.count().get_result(conn).await?;
    let offset = (page - 1) * page_size;
    let logs = dns_log::table
        .select(DnsLog::as_select())
        .order(dns_log::id.desc())
        .limit(page_size)
        .offset(offset)
        .get_results(conn)
        .await?;

    Ok((logs, total))
}
