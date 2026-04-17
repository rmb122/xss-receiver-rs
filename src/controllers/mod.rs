use std::sync::{Arc, RwLock};

use axum::{Router, extract::FromRef, http::StatusCode, response::IntoResponse};
use diesel_async::{AsyncPgConnection, pooled_connection::bb8};

use jsonwebtoken::Algorithm;
use utoipa::openapi::Server;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::{Config, SwaggerUi};

use crate::{
    db::route::helper::get_all_routes,
    dispatcher::{Dispatcher, Route},
    startup_config::StartupConfig,
    storage::Storage,
    utils::{ip2region::Locator, jwt::JwtManager, random::get_random_bytes, response::Response},
};

mod file;
mod frontend;
mod http_log;
mod index;
mod route;
mod system_log;
mod user;

#[derive(Clone)]
pub struct Context {
    config: Arc<StartupConfig>,
    pool: bb8::Pool<AsyncPgConnection>,
    jwt_manager: Arc<JwtManager>,
    locator: Arc<Locator>,

    dispatcher: Arc<RwLock<Dispatcher>>,
    storage: Arc<Storage>,
}

impl Context {
    pub async fn new(
        config: &StartupConfig,
        pool: bb8::Pool<AsyncPgConnection>,
    ) -> anyhow::Result<Self> {
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

        let locator = Locator::new(
            if config.ip2region.ipv4_db.is_empty() {
                None
            } else {
                Some(config.ip2region.ipv4_db.clone())
            },
            if config.ip2region.ipv6_db.is_empty() {
                None
            } else {
                Some(config.ip2region.ipv6_db.clone())
            },
            crate::utils::ip2region::CachePolicy::VectorIndex,
        )?;

        let mut conn = pool.get().await?;

        let storage = Storage::new(&config.storage_path)?;

        Ok(Context {
            config: Arc::new(config.to_owned()),
            pool: pool.clone(),
            jwt_manager: Arc::new(jwt_manager),
            locator: Arc::new(locator),

            dispatcher: Arc::new(RwLock::new(Dispatcher::new(
                get_all_routes(&mut conn)
                    .await?
                    .into_iter()
                    .map(|x| Route::transform(x, &storage))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )?)),
            storage: Arc::new(storage),
        })
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
    let user_router = OpenApiRouter::new()
        .routes(routes!(user::login, user::current))
        .routes(routes!(user::create_user))
        .routes(routes!(user::get_users))
        .routes(routes!(user::delete_user))
        .routes(routes!(user::update_user));

    let route_router = OpenApiRouter::new()
        .routes(routes!(route::create_route))
        .routes(routes!(route::get_routes))
        .routes(routes!(route::delete_route))
        .routes(routes!(route::update_route));

    let http_log_router = OpenApiRouter::new().routes(routes!(http_log::get_http_logs));

    let system_log_router = OpenApiRouter::new().routes(routes!(system_log::get_system_logs));

    let file_router = OpenApiRouter::new()
        .routes(routes!(file::list_all_directorie))
        .routes(routes!(file::create_directory))
        .routes(routes!(file::delete_directory))
        .routes(routes!(file::rename_directory))
        .routes(routes!(file::list_directory))
        .routes(routes!(file::upload_file))
        .routes(routes!(file::download_file))
        .routes(routes!(file::delete_file))
        .routes(routes!(file::rename_file))
        .routes(routes!(file::upload_part))
        .routes(routes!(file::merge_parts))
        .routes(routes!(file::download_log_file));

    let (mut admin_api_router, mut openapi) = OpenApiRouter::new()
        .nest("/user", user_router)
        .nest("/route", route_router)
        .nest("/http_log", http_log_router)
        .nest("/system_log", system_log_router)
        .nest("/file", file_router)
        .split_for_parts();

    let prefix = &context.config.http_server.admin_prefix;
    let prefix = prefix.strip_suffix("/").unwrap_or(prefix);

    // 为 OpenAPI 文档添加服务器前缀
    if !prefix.is_empty() && prefix != "/" {
        openapi.servers = Some(vec![Server::new(prefix)]);
    }

    // add open api
    if context.config.http_server.openapi {
        admin_api_router = admin_api_router.merge(
            SwaggerUi::new("/swagger-ui")
                .url(OPEN_API_URL, openapi)
                .config(Config::from(format!("{}{}", prefix, OPEN_API_URL))),
        );
    }

    // add frontend static file routes
    let admin_router = Router::new()
        .route("/", axum::routing::get(frontend::index))
        .route("/{*path}", axum::routing::get(frontend::serve))
        .nest("/api", admin_api_router);

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
