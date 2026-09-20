use std::sync::{Arc, RwLock};

use axum::{Router, extract::FromRef, http::StatusCode, response::IntoResponse};
use diesel_async::{AsyncPgConnection, pooled_connection::bb8};

use jsonwebtoken::Algorithm;
use log::error;
use tokio::sync::Mutex;
use utoipa::openapi::Server;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::{Config, SwaggerUi};

use crate::{
    db::{dns_route::helper::get_all_dns_routes, http_route::helper::get_all_http_routes},
    dispatcher::{
        DnsDispatcher, DnsRoute, HttpDispatcher, HttpRoute, ScriptCache, ScriptHttpClient,
    },
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
mod log_query;
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
    pub(crate) script_http_client: ScriptHttpClient,

    // Serialize database updates while requests keep using the current dispatchers.
    http_route_update_lock: Arc<Mutex<()>>,
    dns_route_update_lock: Arc<Mutex<()>>,
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
        let script_cache = ScriptCache::new(&config.script.cache);
        let script_http_client = ScriptHttpClient::new(&config.script.http)?;

        Ok(Context {
            config: Arc::new(config.to_owned()),
            pool: pool.clone(),
            jwt_manager: Arc::new(jwt_manager),
            locator: Arc::new(locator),

            http_dispatcher: Arc::new(RwLock::new(HttpDispatcher::new(
                get_all_http_routes(&mut conn)
                    .await?
                    .into_iter()
                    .map(|x| {
                        HttpRoute::transform(
                            x,
                            &storage,
                            script_cache.clone(),
                            script_http_client.clone(),
                        )
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )?)),
            dns_dispatcher: Arc::new(RwLock::new(DnsDispatcher::new(
                get_all_dns_routes(&mut conn)
                    .await?
                    .into_iter()
                    .map(|x| {
                        DnsRoute::transform(
                            x,
                            &storage,
                            script_cache.clone(),
                            script_http_client.clone(),
                        )
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?,
            )?)),
            storage: Arc::new(storage),
            script_cache,
            script_http_client,

            http_route_update_lock: Arc::new(Mutex::new(())),
            dns_route_update_lock: Arc::new(Mutex::new(())),
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
        .routes(routes!(file::create))
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

#[cfg(test)]
mod route_update_tests {
    use axum::{Json, extract::State};
    use diesel_async::{AsyncConnection, AsyncPgConnection, SimpleAsyncConnection};
    use futures::future::join_all;
    use serde_json::{Value, json};

    use super::{Context, dns_route, http_route, user::LoggedUser};
    use crate::{db, utils::jwt::Claims};

    #[derive(Clone, Copy, Debug)]
    enum Protocol {
        Http,
        Dns,
    }

    async fn mutate(
        ctx: &Context,
        protocol: Protocol,
        method: &str,
        body: Value,
    ) -> anyhow::Result<Value> {
        macro_rules! call {
            ($handler:path) => {
                serde_json::to_value(
                    $handler(
                        State(ctx.clone()),
                        Claims(LoggedUser {
                            id: 1,
                            username: "route-test".into(),
                        }),
                        Json(serde_json::from_value(body)?),
                    )
                    .await
                    .map_err(|error| error.0)?,
                )?
            };
        }

        let response = match (protocol, method) {
            (Protocol::Http, "create") => call!(http_route::create_http_route),
            (Protocol::Http, "update") => call!(http_route::update_http_route),
            (Protocol::Http, "delete") => call!(http_route::delete_http_route),
            (Protocol::Dns, "create") => call!(dns_route::create_dns_route),
            (Protocol::Dns, "update") => call!(dns_route::update_dns_route),
            (Protocol::Dns, "delete") => call!(dns_route::delete_dns_route),
            _ => anyhow::bail!("unsupported test mutation"),
        };
        Ok(response["payload"].clone())
    }

    fn route_body(pattern: &str, priority: i32, id: i32) -> Value {
        json!({
            "pattern_kind": "PLAIN",
            "pattern": pattern,
            "priority": priority,
            "timeout": 100,
            "catalog": "",
            "handler_kind": "NONE",
            "handler": "",
            "write_log": false,
            "comment": "",
            "http_route_id": id,
            "route_id": id,
        })
    }

    async fn verify_dispatcher(
        ctx: &Context,
        protocol: Protocol,
        candidates: &[String],
    ) -> anyhow::Result<()> {
        let mut conn = ctx.db_conn().await?;
        let routes: Vec<(String, i32)> = match protocol {
            Protocol::Http => db::http_route::helper::get_all_http_routes(&mut conn)
                .await?
                .into_iter()
                .map(|route| (route.pattern, route.priority))
                .collect(),
            Protocol::Dns => db::dns_route::helper::get_all_dns_routes(&mut conn)
                .await?
                .into_iter()
                .map(|route| (route.pattern, route.priority))
                .collect(),
        };
        drop(conn);
        anyhow::ensure!(routes.len() == 4, "unexpected database route count");
        for candidate in candidates {
            let expected = routes
                .iter()
                .find(|(pattern, _)| pattern == candidate)
                .map(|(_, priority)| *priority);
            let actual = match protocol {
                Protocol::Http => ctx
                    .http_dispatcher
                    .read()
                    .expect("lock poisoned")
                    .dispatch_key(candidate)
                    .map(|route| route.priority),
                Protocol::Dns => ctx
                    .dns_dispatcher
                    .read()
                    .expect("lock poisoned")
                    .dispatch_key(candidate)
                    .map(|route| route.priority),
            };
            anyhow::ensure!(
                actual == expected,
                "{protocol:?} dispatcher differs from database for {candidate}: {actual:?} != {expected:?}"
            );
        }
        Ok(())
    }

    async fn exercise_concurrent_mutations(
        ctx: &Context,
        protocol: Protocol,
    ) -> anyhow::Result<()> {
        let mut candidates = (0..4)
            .map(|index| format!("route{index}.example"))
            .collect::<Vec<_>>();
        let created = join_all(
            candidates
                .iter()
                .map(|pattern| mutate(ctx, protocol, "create", route_body(pattern, 0, 0))),
        )
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;
        verify_dispatcher(ctx, protocol, &candidates).await?;

        let mut mutations = Vec::new();
        for (index, route) in created.iter().enumerate() {
            let id = route["id"].as_i64().unwrap() as i32;
            if index % 2 == 0 {
                let pattern = format!("updated{index}.example");
                mutations.push(("update", route_body(&pattern, 10, id)));
                candidates.push(pattern);
            } else {
                mutations.push(("delete", route_body("", 0, id)));
                let pattern = format!("new{index}.example");
                mutations.push(("create", route_body(&pattern, 20, 0)));
                candidates.push(pattern);
            }
        }
        for result in join_all(
            mutations
                .into_iter()
                .map(|(method, body)| mutate(ctx, protocol, method, body)),
        )
        .await
        {
            result?;
        }
        verify_dispatcher(ctx, protocol, &candidates).await
    }

    // Run with TEST_DATABASE_URL=postgres://... cargo test concurrent_route_updates -- --ignored
    // The database user needs CREATE SCHEMA permission. All data lives in a temporary schema.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires PostgreSQL via TEST_DATABASE_URL"]
    async fn concurrent_route_updates_keep_dispatchers_in_sync() -> anyhow::Result<()> {
        let database_url = std::env::var("TEST_DATABASE_URL")?;
        let schema = format!(
            "route_updates_{}",
            hex::encode(crate::utils::random::get_random_bytes(8))
        );
        let storage_path = std::env::temp_dir().join(&schema);
        let mut admin = AsyncPgConnection::establish(&database_url).await?;
        admin
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .await?;

        let result: anyhow::Result<()> = async {
            let mut url = url::Url::parse(&database_url)?;
            url.query_pairs_mut()
                .append_pair("options", &format!("-csearch_path={schema}"));
            let pool = db::establish_db_connection(url.as_str()).await?;
            db::run_migrations(&pool).await?;
            {
                let mut conn = pool.get().await?;
                // Delay writes so unprotected handlers read overlapping old snapshots reliably.
                conn.batch_execute(
                    "CREATE FUNCTION delay_route_write() RETURNS trigger LANGUAGE plpgsql AS $$
                     BEGIN
                        PERFORM pg_sleep(0.05);
                        IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
                        RETURN NEW;
                     END $$;
                     CREATE TRIGGER delay_http_write BEFORE INSERT OR UPDATE OR DELETE ON http_route
                        FOR EACH ROW EXECUTE FUNCTION delay_route_write();
                     CREATE TRIGGER delay_dns_write BEFORE INSERT OR UPDATE OR DELETE ON dns_route
                        FOR EACH ROW EXECUTE FUNCTION delay_route_write();",
                )
                .await?;
            }
            let mut config =
                crate::startup_config::parse(include_str!("../../config_example.toml"))?;
            config.storage_path = storage_path.to_string_lossy().into_owned();
            config.ip2region.ipv4_db.clear();
            config.ip2region.ipv6_db.clear();
            let ctx = Context::new(&config, pool).await?;
            let (http, dns) = tokio::join!(
                exercise_concurrent_mutations(&ctx, Protocol::Http),
                exercise_concurrent_mutations(&ctx, Protocol::Dns),
            );
            anyhow::ensure!(
                http.is_ok() && dns.is_ok(),
                "HTTP mutations: {http:?}; DNS mutations: {dns:?}"
            );
            Ok(())
        }
        .await;

        admin
            .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .await?;
        if storage_path.exists() {
            std::fs::remove_dir_all(storage_path)?;
        }
        result
    }
}
