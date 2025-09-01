use crate::{
    db::model::{ADMIN_ID, User},
    db::schema::users,
    utils::random::get_random_string,
};
use diesel::prelude::*;
use diesel_async::{
    AsyncPgConnection, RunQueryDsl,
    pooled_connection::{AsyncDieselConnectionManager, bb8},
};

pub async fn create_init_admin_user(
    pool: &bb8::Pool<AsyncPgConnection>,
) -> anyhow::Result<Option<(String, String)>> {
    let username = "admin".to_string();
    let password = get_random_string(16);

    let mut conn = pool.get().await?;

    let count: i64 = users::table
        .filter(users::id.eq(ADMIN_ID))
        .count()
        .get_result(&mut conn)
        .await?;
    if count == 0 {
        let _: i32 = diesel::insert_into(users::table)
            .values(&User {
                id: ADMIN_ID,
                username: username.clone(),
                password: password_auth::generate_hash(&password),
            })
            .returning(users::id)
            .get_result(&mut conn)
            .await?;
        return Ok(Some((username, password)));
    } else {
        return Ok(None);
    }
}

pub async fn establish_db_connection(db_url: &str) -> anyhow::Result<bb8::Pool<AsyncPgConnection>> {
    let config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(db_url);
    Ok(bb8::Pool::builder().build(config).await?)
}
