use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::utils::diesel_bytea;

#[derive(Queryable, Selectable, Serialize, Deserialize, utoipa::ToSchema)]
#[diesel(table_name = crate::db::schema::dns_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DnsLog {
    pub id: i32,
    pub client_ip: String,
    pub client_port: i32,
    pub location: String,
    pub query_name: String,
    pub query_type: String,
    pub query_class: String,
    #[schema(value_type = serde_json::Value)]
    pub extra_info: diesel_bytea::Json<serde_json::Value>,
    pub error_log: Option<String>,
    pub create_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::db::schema::dns_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDnsLog {
    pub client_ip: String,
    pub client_port: i32,
    pub location: String,
    pub query_name: String,
    pub query_type: String,
    pub query_class: String,
    pub extra_info: diesel_bytea::Json<serde_json::Value>,
    pub error_log: Option<String>,
}
