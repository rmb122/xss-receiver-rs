use diesel::prelude::*;
use diesel::{AsExpression, deserialize::FromSqlRow, sql_types::SmallInt};
use diesel_enum::DbEnum;
use serde::{Deserialize, Serialize};

use crate::db::EnumNotFoundError;

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
pub enum RouteKind {
    STATIC,
    SCRIPT,
}

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, utoipa::ToSchema)]
#[diesel(table_name = crate::db::schema::route)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Route {
    pub id: i32,
    pub kind: RouteKind,
    pub pattern: String,
    pub timeout: i32,
    pub catalog: String,
    pub handler: String,
    pub write_log: bool,
    pub comment: String,
    pub create_time: chrono::NaiveDateTime,
}

#[derive(Insertable, Clone)]
#[diesel(table_name = crate::db::schema::route)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewRoute {
    pub kind: RouteKind,
    pub pattern: String,
    pub timeout: i32,
    pub catalog: String,
    pub handler: String,
    pub write_log: bool,
    pub comment: String,
}
