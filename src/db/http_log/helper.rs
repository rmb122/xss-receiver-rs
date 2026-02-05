use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::db::{http_log::model::NewHttpLog, schema::http_log};

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
