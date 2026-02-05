use diesel::prelude::*;
use diesel::{AsExpression, deserialize::FromSqlRow, sql_types::SmallInt};
use diesel_enum::DbEnum;

use crate::db::EnumNotFoundError;
use crate::parsed_request::{KeyValues, PersistedUploadFile};
use crate::utils::diesel_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromSqlRow, DbEnum, AsExpression)]
#[diesel(sql_type = SmallInt)]
#[diesel_enum(error_fn = EnumNotFoundError::not_found)]
#[diesel_enum(error_type = EnumNotFoundError)]
pub enum BodyKind {
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
    pub body_type: BodyKind,
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
    pub body_type: BodyKind,
    pub body: String,
    pub file: diesel_json::Json<PersistedUploadFile>,
    pub extra_info: serde_json::Value,
    pub error_log: Option<String>,
}
