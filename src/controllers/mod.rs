use std::sync::{Arc, RwLock};

use axum::{Router, extract::FromRef, http::StatusCode, response::IntoResponse};
use diesel_async::{AsyncPgConnection, pooled_connection::bb8};
use jsonwebtoken::Algorithm;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::{Config, SwaggerUi};

use crate::{
    dispatcher::Dispatcher,
    startup_config::StartupConfig,
    utils::{jwt::JwtManager, random::get_random_bytes, response::Response},
};

mod index;
mod user;

#[derive(Clone)]
pub struct Context {
    startup_config: Arc<StartupConfig>,
    pool: bb8::Pool<AsyncPgConnection>,
    dispatcher: Arc<RwLock<Dispatcher>>,
    jwt_manager: Arc<JwtManager>,
}

impl Context {
    pub fn new(config: &StartupConfig, pool: bb8::Pool<AsyncPgConnection>) -> Self {
        let mut secret = config.http_server.jwt_secret.trim().as_bytes().to_vec();
        if secret.len() == 0 {
            log::info!("jwt secret not specified, using random generated value");
            secret = get_random_bytes(128);
        }

        let jwt_manager = JwtManager::new(
            Algorithm::HS512,
            secret.as_slice(),
            config.http_server.jwt_expire_time,
        );

        Context {
            startup_config: Arc::new(config.to_owned()),
            pool,
            dispatcher: Arc::new(RwLock::new(Dispatcher::empty())),
            jwt_manager: Arc::new(jwt_manager),
        }
    }

    pub async fn db_conn(&self) -> anyhow::Result<bb8::PooledConnection<'_, AsyncPgConnection>> {
        Ok(self.pool.get().await?)
    }
}

impl FromRef<Context> for Arc<JwtManager> {
    fn from_ref(context: &Context) -> Arc<JwtManager> {
        context.jwt_manager.clone()
    }
}

const OPEN_API_URL: &str = "/api-docs/openapi.json";

pub fn get_app_router(context: Context) -> Router<()> {
    let user_router = OpenApiRouter::new().routes(routes!(user::login, user::info));
    let (mut admin_router, api) = OpenApiRouter::new()
        .nest("/user", user_router)
        .split_for_parts();

    let prefix = &context.startup_config.http_server.admin_prefix;
    let prefix = prefix.strip_suffix("/").unwrap_or(prefix);

    // add open api
    admin_router = admin_router.merge(
        SwaggerUi::new("/swagger-ui")
            .url(OPEN_API_URL, api)
            .config(Config::from(format!("{}{}", prefix, OPEN_API_URL))),
    );

    let router = if prefix.is_empty() || prefix == "/" {
        Router::new().merge(admin_router)
    } else {
        Router::new().nest(&prefix, admin_router)
    };
    return router.fallback(index::index).with_state(context);
}

// https://github.com/tokio-rs/axum/blob/main/examples/anyhow-error-response/src/main.rs
// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        (StatusCode::OK, Response::<()>::error(self.0.to_string())).into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
