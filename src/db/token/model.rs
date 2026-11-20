use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;

use crate::db::schema::{api_tokens, users};

#[derive(Queryable, Selectable, Serialize, utoipa::ToSchema)]
#[diesel(table_name = api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TokenInfo {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    #[diesel(select_expression = users::username)]
    pub username: String,
    pub create_time: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub allowed_ip: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = api_tokens)]
pub struct NewToken<'a> {
    pub user_id: i32,
    pub name: &'a str,
    pub token_hash: &'a str,
    pub expires_at: Option<DateTime<Utc>>,
    pub allowed_ip: Option<&'a str>,
}
