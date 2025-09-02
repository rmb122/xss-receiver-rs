use axum::{Json, extract::State};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::Deserialize;

use crate::{
    Context,
    controllers::{AppError, Response},
    db::{model::User, schema::users},
};

#[derive(Deserialize)]
pub struct LoginRequeqst {
    username: String,
    password: String,
}

pub async fn login(
    State(ctx): State<Context>,
    Json(request): Json<LoginRequeqst>,
) -> Result<Response<()>, AppError> {
    let mut conn = ctx.db_conn().await?;

    let user: Option<User> = users::table
        .filter(users::username.eq(&request.username))
        .select(User::as_select())
        .first(&mut conn)
        .await
        .optional()?;

    if let Some(user) = user {
        if password_auth::verify_password(&request.password, &user.password).is_ok() {
            return Ok(Response::ok().msg("login success"));
        }
    }
    return Err(anyhow::anyhow!("username or password error").into());
}
