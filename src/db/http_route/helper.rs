use crate::db::{
    http_route::model::{HttpRoute, NewHttpRoute},
    schema::http_route,
};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

pub async fn get_all_http_routes(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<HttpRoute>> {
    return Ok(http_route::table
        .select(HttpRoute::as_select())
        .order(http_route::id.asc())
        .get_results(conn)
        .await?);
}

pub async fn get_all_http_routes_except(
    conn: &mut AsyncPgConnection,
    http_route_id: i32,
) -> anyhow::Result<Vec<HttpRoute>> {
    return Ok(http_route::table
        .select(HttpRoute::as_select())
        .filter(http_route::id.ne(http_route_id))
        .order(http_route::id.asc())
        .get_results(conn)
        .await?);
}

// 根据ID查询 HTTP 路由
pub async fn find_http_route_by_id(
    conn: &mut AsyncPgConnection,
    http_route_id: i32,
) -> anyhow::Result<Option<HttpRoute>> {
    Ok(http_route::table
        .filter(http_route::id.eq(http_route_id))
        .select(HttpRoute::as_select())
        .first(conn)
        .await
        .optional()?)
}

// 创建 HTTP 路由
pub async fn create_http_route(
    conn: &mut AsyncPgConnection,
    new_route: &NewHttpRoute,
) -> anyhow::Result<HttpRoute> {
    let http_route: HttpRoute = diesel::insert_into(http_route::table)
        .values(new_route)
        .returning(HttpRoute::as_returning())
        .get_result(conn)
        .await?;

    Ok(http_route)
}

// 删除 HTTP 路由
pub async fn delete_http_route(
    conn: &mut AsyncPgConnection,
    http_route_id: i32,
) -> anyhow::Result<bool> {
    let deleted_count = diesel::delete(http_route::table.filter(http_route::id.eq(http_route_id)))
        .execute(conn)
        .await?;

    Ok(deleted_count > 0)
}

// 更新 HTTP 路由
pub async fn update_http_route(
    conn: &mut AsyncPgConnection,
    http_route_id: i32,
    updated_route: &NewHttpRoute,
) -> anyhow::Result<HttpRoute> {
    // 检查 HTTP 路由是否存在
    let existing_http_route = find_http_route_by_id(conn, http_route_id).await?;
    if existing_http_route.is_none() {
        return Err(anyhow::anyhow!("http_route not found"));
    }

    let http_route = diesel::update(http_route::table.filter(http_route::id.eq(http_route_id)))
        .set((
            http_route::pattern_kind.eq(&updated_route.pattern_kind),
            http_route::pattern.eq(&updated_route.pattern),
            http_route::priority.eq(updated_route.priority),
            http_route::timeout.eq(updated_route.timeout),
            http_route::catalog.eq(&updated_route.catalog),
            http_route::handler_kind.eq(&updated_route.handler_kind),
            http_route::handler.eq(&updated_route.handler),
            http_route::write_log.eq(updated_route.write_log),
            http_route::comment.eq(&updated_route.comment),
        ))
        .returning(HttpRoute::as_returning())
        .get_result(conn)
        .await?;

    Ok(http_route)
}
