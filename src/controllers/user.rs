use axum::{Json, extract::State};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};

use crate::{
    Context,
    controllers::AppError,
    db::{model::User, schema::users},
    utils::{jwt::Claims, response::Response},
};

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LoginedUser {
    pub id: i32,
    pub username: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct LoginRequeqst {
    username: String,
    password: String,
}

#[utoipa::path(post, path = "/login", responses((status = OK, body = Response<String>)))]
pub async fn login(
    State(ctx): State<Context>,
    Json(request): Json<LoginRequeqst>,
) -> Result<Response<String>, AppError> {
    let mut conn = ctx.db_conn().await?;

    let user: Option<User> = users::table
        .filter(users::username.eq(&request.username))
        .select(User::as_select())
        .first(&mut conn)
        .await
        .optional()?;

    if let Some(user) = user {
        if password_auth::verify_password(&request.password, &user.password).is_ok() {
            return Ok(Response::<String>::ok().msg("login success").payload(
                ctx.jwt_manager.encode_token(LoginedUser {
                    id: user.id,
                    username: user.username,
                })?,
            ));
        }
    }
    return Err(anyhow::anyhow!("username or password error").into());
}

#[utoipa::path(get, path = "/info", responses((status = OK, body = Response<LoginedUser>)))]
pub async fn info(Claims(user): Claims<LoginedUser>) -> Response<LoginedUser> {
    Response::ok().payload(user)
}
