use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::{
    Context,
    controllers::AppError,
    db::user::{helper::find_user_by_username, model::User},
    utils::{jwt::Claims, response::Response},
};

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LoggedUser {
    pub id: i32,
    pub username: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[utoipa::path(post, path = "/login", responses((status = OK, body = Response<String>)))]
pub async fn login(
    State(ctx): State<Context>,
    Json(request): Json<LoginRequest>,
) -> Result<Response<String>, AppError> {
    let mut conn = ctx.db_conn().await?;
    let user: Option<User> = find_user_by_username(&mut conn, &request.username).await?;

    if let Some(user) = user {
        if password_auth::verify_password(&request.password, &user.password).is_ok() {
            return Ok(Response::<String>::ok().msg("login success").payload(
                ctx.jwt_manager.encode_token(LoggedUser {
                    id: user.id,
                    username: user.username,
                })?,
            ));
        }
    }
    return Err(anyhow::anyhow!("username or password error").into());
}

#[utoipa::path(get, path = "/info", responses((status = OK, body = Response<LoggedUser>)))]
pub async fn info(Claims(user): Claims<LoggedUser>) -> Response<LoggedUser> {
    Response::ok().payload(user)
}
