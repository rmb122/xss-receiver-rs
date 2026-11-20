use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
    response::IntoResponse,
};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};

use super::{AppError, Context, index::get_real_addr_from_request};
use crate::{
    db::token::helper::find_token_user,
    utils::{
        api_token::{TOKEN_PREFIX, hash_token},
        response::Response,
    },
};

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LoggedUser {
    pub id: i32,
    pub username: String,
}

pub struct AuthenticatedUser {
    pub user: LoggedUser,
    pub is_api_token: bool,
}

impl AuthenticatedUser {
    pub fn require_session(&self) -> Result<(), AppError> {
        if self.is_api_token {
            return Err(anyhow::anyhow!("permission denied: password login required").into());
        }
        Ok(())
    }
}

pub enum AuthError {
    InvalidToken,
    Internal(anyhow::Error),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::InvalidToken => Response::<()>::new().code(400).msg("token verify failed"),
            Self::Internal(error) => Response::<()>::error(error.to_string()),
        }
        .into_response()
    }
}

impl FromRequestParts<Context> for AuthenticatedUser {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, ctx: &Context) -> Result<Self, Self::Rejection> {
        let token = if let Some(value) = parts.headers.get(AUTHORIZATION) {
            value
                .to_str()
                .map_err(|_| AuthError::InvalidToken)?
                .to_owned()
        } else {
            CookieJar::from_headers(&parts.headers)
                .get(AUTHORIZATION.as_str())
                .ok_or(AuthError::InvalidToken)?
                .value_trimmed()
                .to_owned()
        };
        let token = token.trim();
        let token = token.strip_prefix("Bearer ").unwrap_or(token).trim();

        let is_api_token = token.starts_with(TOKEN_PREFIX);
        let user = if is_api_token {
            let client_ip = get_real_addr_from_request(
                &ctx.config.http_server.real_addr_header,
                &parts.headers,
            )
            .ok()
            .or_else(|| {
                parts
                    .extensions
                    .get::<ConnectInfo<SocketAddr>>()
                    .map(|peer| peer.0)
            })
            .map(|addr| addr.ip());
            let mut conn = ctx.db_conn().await.map_err(AuthError::Internal)?;
            let (id, username) = find_token_user(&mut conn, &hash_token(token), client_ip)
                .await
                .map_err(AuthError::Internal)?
                .ok_or(AuthError::InvalidToken)?;
            LoggedUser { id, username }
        } else {
            ctx.jwt_manager
                .decode_token(token)
                .map_err(|_| AuthError::InvalidToken)?
        };
        Ok(Self { user, is_api_token })
    }
}
