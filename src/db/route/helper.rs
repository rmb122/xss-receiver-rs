use crate::db::{route::model::Route, schema::route};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

pub async fn get_all_routes(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<Route>> {
    return Ok(route::table
        .select(Route::as_select())
        .get_results(conn)
        .await?);
}
