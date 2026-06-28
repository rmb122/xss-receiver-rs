use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::{
    Context,
    controllers::{AppError, index::get_real_addr_from_request},
    db::{
        system_log::helper::insert_system_log,
        user::{
            helper::{
                create_user as db_create_user, delete_user as db_delete_user,
                find_user_by_username, get_all_users, update_user as db_update_user,
            },
            model::{ADMIN_ID, User},
        },
    },
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

/// 用户登录
#[utoipa::path(post, path = "/login", responses((status = OK, body = Response<bool>)))]
pub async fn login(
    State(ctx): State<Context>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut conn = ctx.db_conn().await?;

    let client_addr: SocketAddr = if let Ok(client_addr) =
        get_real_addr_from_request(&ctx.config.http_server.real_addr_header, &headers)
    {
        client_addr
    } else {
        addr
    };

    let user: Option<User> = find_user_by_username(&mut conn, &request.username).await?;

    if let Some(user) = user {
        if password_auth::verify_password(&request.password, &user.password).is_ok() {
            let _ = insert_system_log(
                &mut conn,
                &format!(
                    "user {} logged from {} ({})",
                    &user.username,
                    client_addr.to_string(),
                    ctx.locator.locate(&client_addr.ip().to_string()),
                ),
            )
            .await;

            // 生成 JWT token
            let token = ctx.jwt_manager.encode_token(LoggedUser {
                id: user.id,
                username: user.username,
            })?;

            // 创建响应头，设置 Authorization Cookie
            let mut response_headers = HeaderMap::new();
            response_headers.insert(
                header::SET_COOKIE,
                format!(
                    "{}={}; Path={}; HttpOnly",
                    header::AUTHORIZATION,
                    token,
                    ctx.config.http_server.admin_prefix
                )
                .parse()?,
            );

            return Ok((response_headers, Response::<()>::ok().payload(())));
        }
    }

    let _ = insert_system_log(
        &mut conn,
        &format!(
            "failed login attempt for user {} from {} ({})",
            &request.username,
            client_addr.to_string(),
            ctx.locator.locate(&client_addr.ip().to_string()),
        ),
    )
    .await;

    return Err(anyhow::anyhow!("username or password error").into());
}

/// 获取当前登录用户
#[utoipa::path(get, path = "/current", responses((status = OK, body = Response<LoggedUser>)))]
pub async fn current(Claims(user): Claims<LoggedUser>) -> Response<LoggedUser> {
    Response::ok().payload(user)
}

// 用户响应（不包含密码）
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UserResponse {
    pub id: i32,
    pub username: String,
    pub create_time: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        UserResponse {
            id: user.id,
            username: user.username,
            create_time: user.create_time.to_string(),
        }
    }
}

// 创建用户请求
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateUserRequest {
    username: String,
    password: String,
}

// 删除用户请求
#[derive(Deserialize, utoipa::ToSchema)]
pub struct DeleteUserRequest {
    user_id: i32,
}

// 更新用户请求
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateUserRequest {
    user_id: i32,
    username: Option<String>,
    password: Option<String>,
}

// 管理员权限检查
pub(crate) fn check_admin(user: &LoggedUser) -> Result<(), AppError> {
    if user.id != ADMIN_ID {
        return Err(anyhow::anyhow!("permission denied: admin only").into());
    }
    Ok(())
}

/// 创建用户
#[utoipa::path(post, path = "/", responses((status = OK, body = Response<UserResponse>)))]
pub async fn create_user(
    State(ctx): State<Context>,
    Claims(user): Claims<LoggedUser>,
    Json(request): Json<CreateUserRequest>,
) -> Result<Response<UserResponse>, AppError> {
    check_admin(&user)?;

    let mut conn = ctx.db_conn().await?;
    let new_user = db_create_user(&mut conn, &request.username, &request.password).await?;

    Ok(Response::<UserResponse>::ok().payload(new_user.into()))
}

/// 删除用户
#[utoipa::path(delete, path = "/", responses((status = OK, body = Response<bool>)))]
pub async fn delete_user(
    State(ctx): State<Context>,
    Claims(user): Claims<LoggedUser>,
    Json(request): Json<DeleteUserRequest>,
) -> Result<Response<bool>, AppError> {
    check_admin(&user)?;

    let mut conn = ctx.db_conn().await?;
    let deleted = db_delete_user(&mut conn, request.user_id).await?;

    if deleted {
        Ok(Response::<bool>::ok().payload(true))
    } else {
        Err(anyhow::anyhow!("user not found").into())
    }
}

/// 获取用户列表
#[utoipa::path(get, path = "/", responses((status = OK, body = Response<Vec<UserResponse>>)))]
pub async fn get_users(
    State(ctx): State<Context>,
    Claims(user): Claims<LoggedUser>,
) -> Result<Response<Vec<UserResponse>>, AppError> {
    check_admin(&user)?;

    let mut conn = ctx.db_conn().await?;
    let users = get_all_users(&mut conn).await?;
    let user_responses: Vec<UserResponse> = users.into_iter().map(|u| u.into()).collect();

    Ok(Response::<Vec<UserResponse>>::ok().payload(user_responses))
}

/// 更新用户
#[utoipa::path(patch, path = "/", responses((status = OK, body = Response<UserResponse>)))]
pub async fn update_user(
    State(ctx): State<Context>,
    Claims(user): Claims<LoggedUser>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Response<UserResponse>, AppError> {
    if user.id != request.user_id {
        // 只能修改自己的
        check_admin(&user)?;
    }

    // 至少需要提供一个更新字段
    if request.username.is_none() && request.password.is_none() {
        return Err(anyhow::anyhow!("at least one field must be provided for update").into());
    }

    let mut conn = ctx.db_conn().await?;
    let updated_user = db_update_user(
        &mut conn,
        request.user_id,
        request.username.as_deref(),
        request.password.as_deref(),
    )
    .await?;

    Ok(Response::<UserResponse>::ok().payload(updated_user.into()))
}
