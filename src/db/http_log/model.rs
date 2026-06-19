use diesel::prelude::*;
use diesel::{AsExpression, deserialize::FromSqlRow, sql_types::SmallInt};
use diesel_enum::DbEnum;
use serde::{Deserialize, Serialize};

use crate::db::EnumNotFoundError;
use crate::utils::diesel_bytea;
use crate::utils::parsed_request::{KeyValues, PersistedUploadFile};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    FromSqlRow,
    DbEnum,
    AsExpression,
    Serialize,
    Deserialize,
    utoipa::ToSchema,
)]
#[diesel(sql_type = SmallInt)]
#[diesel_enum(error_fn = EnumNotFoundError::not_found)]
#[diesel_enum(error_type = EnumNotFoundError)]
#[serde(rename_all = "UPPERCASE")]
pub enum BodyKind {
    NONE,
    FAILED,
    FORM,
    JSON,
}

#[derive(Queryable, Selectable, Serialize, Deserialize, utoipa::ToSchema)]
#[diesel(table_name = crate::db::schema::http_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct HttpLog {
    pub id: i32,
    pub client_ip: String,
    pub client_port: i32,
    pub location: String,
    pub method: String,
    pub path: String,
    #[schema(value_type = serde_json::Object)]
    pub arg: diesel_bytea::Json<KeyValues>,
    #[schema(value_type = serde_json::Object)]
    pub header: diesel_bytea::Json<KeyValues>,
    pub parsed_body_type: BodyKind,
    #[schema(value_type = String)]
    pub parsed_body: diesel_bytea::StringBytes,
    #[schema(value_type = serde_json::Object)]
    pub file: diesel_bytea::Json<PersistedUploadFile>,
    #[schema(value_type = serde_json::Value)]
    pub extra_info: diesel_bytea::Json<serde_json::Value>,
    pub error_log: Option<String>,
    pub create_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::db::schema::http_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewHttpLog {
    pub client_ip: String,
    pub client_port: i32,
    pub location: String,
    pub method: String,
    pub path: String,
    pub arg: diesel_bytea::Json<KeyValues>,
    pub header: diesel_bytea::Json<KeyValues>,
    pub parsed_body_type: BodyKind,
    pub parsed_body: diesel_bytea::StringBytes,
    pub raw_body: Vec<u8>,
    pub file: diesel_bytea::Json<PersistedUploadFile>,
    pub extra_info: diesel_bytea::Json<serde_json::Value>,
    pub error_log: Option<String>,
}
