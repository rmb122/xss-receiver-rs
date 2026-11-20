use std::net::IpAddr;

use chrono::Utc;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::db::{
    schema::{api_tokens, users},
    token::model::{NewToken, TokenInfo},
    user::model::ADMIN_ID,
};

pub async fn create_token(
    conn: &mut AsyncPgConnection,
    token: &NewToken<'_>,
) -> anyhow::Result<TokenInfo> {
    let id: i32 = diesel::insert_into(api_tokens::table)
        .values(token)
        .returning(api_tokens::id)
        .get_result(conn)
        .await?;

    Ok(api_tokens::table
        .inner_join(users::table)
        .filter(api_tokens::id.eq(id))
        .select(TokenInfo::as_select())
        .first(conn)
        .await?)
}

pub async fn get_tokens(
    conn: &mut AsyncPgConnection,
    user_id: i32,
) -> anyhow::Result<Vec<TokenInfo>> {
    let mut query = api_tokens::table
        .inner_join(users::table)
        .select(TokenInfo::as_select())
        .order(api_tokens::id.desc())
        .into_boxed();
    if user_id != ADMIN_ID {
        query = query.filter(api_tokens::user_id.eq(user_id));
    }
    Ok(query.load(conn).await?)
}

pub async fn delete_token(
    conn: &mut AsyncPgConnection,
    user_id: i32,
    token_id: i32,
) -> anyhow::Result<bool> {
    let query = diesel::delete(api_tokens::table.filter(api_tokens::id.eq(token_id)));
    let count = if user_id == ADMIN_ID {
        query.execute(conn).await?
    } else {
        query
            .filter(api_tokens::user_id.eq(user_id))
            .execute(conn)
            .await?
    };
    Ok(count > 0)
}

pub async fn find_token_user(
    conn: &mut AsyncPgConnection,
    token_hash: &str,
    client_ip: Option<IpAddr>,
) -> anyhow::Result<Option<(i32, String)>> {
    Ok(api_tokens::table
        .inner_join(users::table)
        .filter(api_tokens::token_hash.eq(token_hash))
        .filter(
            api_tokens::expires_at
                .is_null()
                .or(api_tokens::expires_at.gt(Utc::now())),
        )
        .filter(
            api_tokens::allowed_ip
                .is_null()
                .or(api_tokens::allowed_ip.eq(client_ip.map(|ip| ip.to_canonical().to_string()))),
        )
        .select((users::id, users::username))
        .first(conn)
        .await
        .optional()?)
}
