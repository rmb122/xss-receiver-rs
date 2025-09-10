use diesel::prelude::*;
use diesel::{AsExpression, deserialize::FromSqlRow, sql_types::SmallInt};
use diesel_enum::DbEnum;

use crate::parsed_request::{KeyValues, PersistedUploadFile};
use crate::utils::diesel_json;

pub const ADMIN_ID: i32 = 1;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::db::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::db::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewUser {
    pub username: String,
    pub password: String,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromSqlRow, DbEnum, AsExpression)]
#[diesel(sql_type = SmallInt)]
#[diesel_enum(error_fn = EnumNotFoundError::not_found)]
#[diesel_enum(error_type = EnumNotFoundError)]
pub enum BodyType {
    RAW,
    FORM,
    JSON,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::db::schema::http_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct HttpLog {
    pub id: i32,
    pub client_ip: String,
    pub client_port: i32,
    pub method: String,
    pub path: String,
    pub arg: diesel_json::Json<KeyValues>,
    pub header: diesel_json::Json<KeyValues>,
    pub body_type: BodyType,
    pub body: String,
    pub file: diesel_json::Json<PersistedUploadFile>,
    pub extra_info: serde_json::Value,
    pub error_log: Option<String>,
    pub create_time: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::db::schema::http_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewHttpLog {
    pub client_ip: String,
    pub client_port: i32,
    pub method: String,
    pub path: String,
    pub arg: diesel_json::Json<KeyValues>,
    pub header: diesel_json::Json<KeyValues>,
    pub body_type: BodyType,
    pub body: String,
    pub file: diesel_json::Json<PersistedUploadFile>,
    pub extra_info: serde_json::Value,
    pub error_log: Option<String>,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::db::schema::system_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SystemLog {
    pub id: i32,
    pub log: String,
    pub create_time: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::db::schema::system_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewSystemLog {
    pub log: String,
}
