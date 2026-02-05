use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::db::{schema::system_log, system_log::model::NewSystemLog};

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
