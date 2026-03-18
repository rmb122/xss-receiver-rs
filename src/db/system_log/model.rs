use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, utoipa::ToSchema)]
#[diesel(table_name = crate::db::schema::system_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SystemLog {
    pub id: i32,
    pub log: String,
    pub create_time: chrono::DateTime<chrono::Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::db::schema::system_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewSystemLog {
    pub log: String,
}
