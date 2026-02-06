use crate::{
    db::{
        schema::users,
        user::model::{ADMIN_ID, NewUser, User},
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

// 创建用户
pub async fn create_user(
    conn: &mut AsyncPgConnection,
    username: &str,
    password: &str,
) -> anyhow::Result<User> {
    // 检查用户名是否已存在
    let existing_user = find_user_by_username(conn, username).await?;
    if existing_user.is_some() {
        return Err(anyhow::anyhow!("username already exists"));
    }

    let new_user = NewUser {
        username: username.to_string(),
        password: password_auth::generate_hash(password),
    };

    let user: User = diesel::insert_into(users::table)
        .values(&new_user)
        .returning(User::as_returning())
        .get_result(conn)
        .await?;

    Ok(user)
}

// 删除用户
pub async fn delete_user(conn: &mut AsyncPgConnection, user_id: i32) -> anyhow::Result<bool> {
    // 不允许删除管理员账户
    if user_id == ADMIN_ID {
        return Err(anyhow::anyhow!("cannot delete admin user"));
    }

    let deleted_count = diesel::delete(users::table.filter(users::id.eq(user_id)))
        .execute(conn)
        .await?;

    Ok(deleted_count > 0)
}

// 查询所有用户
pub async fn get_all_users(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<User>> {
    Ok(users::table
        .select(User::as_select())
        .order(users::id.asc())
        .get_results(conn)
        .await?)
}

// 根据ID查询用户
pub async fn get_user_by_id(
    conn: &mut AsyncPgConnection,
    user_id: i32,
) -> anyhow::Result<Option<User>> {
    Ok(users::table
        .filter(users::id.eq(user_id))
        .select(User::as_select())
        .first(conn)
        .await
        .optional()?)
}

// 更新用户
pub async fn update_user(
    conn: &mut AsyncPgConnection,
    user_id: i32,
    username: Option<&str>,
    password: Option<&str>,
) -> anyhow::Result<User> {
    // 检查用户是否存在
    let existing_user = get_user_by_id(conn, user_id).await?;
    if existing_user.is_none() {
        return Err(anyhow::anyhow!("user not found"));
    }

    // 如果更新用户名，检查新用户名是否已被使用
    if let Some(new_username) = username {
        let user_with_name = find_user_by_username(conn, new_username).await?;
        if let Some(user) = user_with_name {
            if user.id != user_id {
                return Err(anyhow::anyhow!("username already exists"));
            }
        }
    }

    // 根据不同情况执行更新
    let updated_user = match (username, password) {
        (Some(new_username), Some(new_password)) => {
            // 同时更新用户名和密码
            let hashed_password = password_auth::generate_hash(new_password);
            diesel::update(users::table.filter(users::id.eq(user_id)))
                .set((
                    users::username.eq(new_username),
                    users::password.eq(hashed_password),
                ))
                .returning(User::as_returning())
                .get_result(conn)
                .await?
        }
        (Some(new_username), None) => {
            // 只更新用户名
            diesel::update(users::table.filter(users::id.eq(user_id)))
                .set(users::username.eq(new_username))
                .returning(User::as_returning())
                .get_result(conn)
                .await?
        }
        (None, Some(new_password)) => {
            // 只更新密码
            let hashed_password = password_auth::generate_hash(new_password);
            diesel::update(users::table.filter(users::id.eq(user_id)))
                .set(users::password.eq(hashed_password))
                .returning(User::as_returning())
                .get_result(conn)
                .await?
        }
        (None, None) => {
            // 没有更新内容，返回原用户
            existing_user.unwrap()
        }
    };

    Ok(updated_user)
}
