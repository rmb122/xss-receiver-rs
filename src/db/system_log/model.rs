use diesel::prelude::*;

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
