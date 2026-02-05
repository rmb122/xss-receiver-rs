use crate::{
    db::{
        schema::users,
        user::model::{ADMIN_ID, User},
    },
    utils::random::get_random_string,
};
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::bb8};

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
                create_time: Utc::now().naive_utc(),
            })
            .returning(users::id)
            .get_result(&mut conn)
            .await?;
        return Ok(Some((username, password)));
    } else {
        return Ok(None);
    }
}

pub async fn find_user_by_username(
    conn: &mut AsyncPgConnection,
    username: &str,
) -> anyhow::Result<Option<User>> {
    return Ok(users::table
        .filter(users::username.eq(username))
        .select(User::as_select())
        .first(conn)
        .await
        .optional()?);
}
