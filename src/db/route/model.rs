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
pub enum PatternKind {
    PLAIN,
    REGEX,
}

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
pub enum HandlerKind {
    STATIC,
    SCRIPT,
    NONE,
}

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, utoipa::ToSchema)]
#[diesel(table_name = crate::db::schema::route)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Route {
    pub id: i32,
    pub pattern_kind: PatternKind,
    pub pattern: String,
    pub priority: i32,
    pub timeout: i32,
    pub catalog: String,
    pub handler_kind: HandlerKind,
    pub handler: String,
    pub write_log: bool,
    pub comment: String,
    pub create_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Insertable, Clone)]
#[diesel(table_name = crate::db::schema::route)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewRoute {
    pub pattern_kind: PatternKind,
    pub pattern: String,
    pub priority: i32,
    pub timeout: i32,
    pub catalog: String,
    pub handler_kind: HandlerKind,
    pub handler: String,
    pub write_log: bool,
    pub comment: String,
}
