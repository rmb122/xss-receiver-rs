use std::sync::{Arc, RwLock};

use axum::{Router, extract::FromRef, http::StatusCode, response::IntoResponse};
use diesel_async::{AsyncPgConnection, pooled_connection::bb8};

use jsonwebtoken::Algorithm;
use log::error;
use utoipa::openapi::Server;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::{Config, SwaggerUi};

use crate::{
    db::{dns_route::helper::get_all_dns_routes, http_route::helper::get_all_http_routes},
    dispatcher::{DnsDispatcher, DnsRoute, HttpDispatcher, HttpRoute, ScriptCache},
    startup_config::StartupConfig,
    storage::Storage,
    utils::{ip2region::Locator, jwt::JwtManager, random::get_random_bytes, response::Response},
};

mod dns_log;
mod dns_route;
mod file;
mod frontend;
mod http_log;
mod http_route;
mod index;
mod system_log;
mod user;

#[derive(Clone)]
pub struct Context {
    pub(crate) config: Arc<StartupConfig>,
    pub(crate) pool: bb8::Pool<AsyncPgConnection>,
    jwt_manager: Arc<JwtManager>,
    pub(crate) locator: Arc<Locator>,

    pub(crate) http_dispatcher: Arc<RwLock<HttpDispatcher>>,
    pub(crate) dns_dispatcher: Arc<RwLock<DnsDispatcher>>,
    pub(crate) storage: Arc<Storage>,
    pub(crate) script_cache: ScriptCache,
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
        let script_cache = ScriptCache::new(&config.script_cache);

        Ok(Context {
            config: Arc::new(config.to_owned()),
            pool: pool.clone(),
            jwt_manager: Arc::new(jwt_manager),
            locator: Arc::new(locator),

            http_dispatcher: Arc::new(RwLock::new(HttpDispatcher::new(
                get_all_http_routes(&mut conn)
                    .await?
                    .into_iter()
                    .map(|x| HttpRoute::transform(x, &storage, script_cache.clone()))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )?)),
            dns_dispatcher: Arc::new(RwLock::new(DnsDispatcher::new(
                get_all_dns_routes(&mut conn)
                    .await?
                    .into_iter()
                    .map(|x| DnsRoute::transform(x, &storage, script_cache.clone()))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )?)),
            storage: Arc::new(storage),
            script_cache,
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

    let http_route_router = OpenApiRouter::new()
        .routes(routes!(http_route::create_http_route))
        .routes(routes!(http_route::get_http_routes))
        .routes(routes!(http_route::delete_http_route))
        .routes(routes!(http_route::update_http_route));

    let http_log_router = OpenApiRouter::new()
        .routes(routes!(http_log::get_http_logs))
        .routes(routes!(http_log::get_http_log_raw_body_response));

    let dns_route_router = OpenApiRouter::new()
        .routes(routes!(dns_route::create_dns_route))
        .routes(routes!(dns_route::get_dns_routes))
        .routes(routes!(dns_route::delete_dns_route))
        .routes(routes!(dns_route::update_dns_route));

    let dns_log_router = OpenApiRouter::new().routes(routes!(dns_log::get_dns_logs));

    let system_log_router = OpenApiRouter::new().routes(routes!(system_log::get_system_logs));

    let file_router = OpenApiRouter::new()
        .routes(routes!(file::list))
        .routes(routes!(file::list_all))
        .routes(routes!(file::mkdir))
        .routes(routes!(file::remove))
        .routes(routes!(file::rename))
        .routes(routes!(file::upload))
        .routes(routes!(file::upload_part))
        .routes(routes!(file::merge_parts))
        .routes(routes!(file::download))
        .routes(routes!(file::stat))
        .routes(routes!(file::download_log_file));

    let (mut admin_api_router, mut openapi) = OpenApiRouter::new()
        .nest("/user", user_router)
        .nest("/http_route", http_route_router)
        .nest("/http_log", http_log_router)
        .nest("/dns_route", dns_route_router)
        .nest("/dns_log", dns_log_router)
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
                .config(Config::from(format!("{}/api{}", prefix, OPEN_API_URL))),
        );
    }

    // 访问不带尾部斜杠的前缀时, 跳转到带斜杠的版本 (如 /test -> /test/)
    let redirect_location = format!("{}/", prefix);

    // add frontend static file routes
    let admin_router = Router::new()
        .route(
            "/",
            axum::routing::get(
                move || async move { axum::response::Redirect::to(&redirect_location) },
            ),
        )
        .route("/{*path}", axum::routing::get(frontend::serve))
        .nest("/api", admin_api_router);

    if prefix.is_empty() || prefix == "/" {
        error!("prefix must not a root path");
    }

    return Router::new()
        .route(&format!("{}/", prefix), axum::routing::get(frontend::index))
        .nest(&prefix, admin_router)
        .fallback(index::index)
        .with_state(context);
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
