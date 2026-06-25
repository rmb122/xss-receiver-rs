use diesel_async::{
    AsyncMigrationHarness, AsyncPgConnection,
    pooled_connection::{AsyncDieselConnectionManager, bb8},
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

pub mod schema;

pub mod dns_log;
pub mod dns_route;
pub mod http_log;
pub mod http_route;
pub mod system_log;
pub mod user;

#[derive(Debug, thiserror::Error)]
#[error("enum not found: {msg}")]
pub struct EnumNotFoundError {
    msg: String,
}

impl EnumNotFoundError {
    fn not_found(msg: String) -> Self {
        Self { msg }
    }
}

pub async fn establish_db_connection(db_url: &str) -> anyhow::Result<bb8::Pool<AsyncPgConnection>> {
    let config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(db_url);
    Ok(bb8::Pool::builder().build(config).await?)
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub async fn run_migrations(pool: &bb8::Pool<AsyncPgConnection>) -> anyhow::Result<()> {
    let conn = pool.get_owned().await?;
    let mut harness = AsyncMigrationHarness::new(conn);
    harness
        .run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("run migrations failed: {e}"))?;
    Ok(())
}
