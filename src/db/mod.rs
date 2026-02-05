use diesel_async::{
    AsyncPgConnection,
    pooled_connection::{AsyncDieselConnectionManager, bb8},
};

pub mod schema;

pub mod http_log;
pub mod route;
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
