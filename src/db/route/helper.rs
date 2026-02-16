use crate::db::{
    route::model::{NewRoute, Route},
    schema::route,
};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

pub async fn get_all_routes(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<Route>> {
    return Ok(route::table
        .select(Route::as_select())
        .order(route::id.asc())
        .get_results(conn)
        .await?);
}

pub async fn get_all_routes_except(
    conn: &mut AsyncPgConnection,
    route_id: i32,
) -> anyhow::Result<Vec<Route>> {
    return Ok(route::table
        .select(Route::as_select())
        .filter(route::id.ne(route_id))
        .order(route::id.asc())
        .get_results(conn)
        .await?);
}

// 根据ID查询路由
pub async fn find_route_by_id(
    conn: &mut AsyncPgConnection,
    route_id: i32,
) -> anyhow::Result<Option<Route>> {
    Ok(route::table
        .filter(route::id.eq(route_id))
        .select(Route::as_select())
        .first(conn)
        .await
        .optional()?)
}

// 创建路由
pub async fn create_route(
    conn: &mut AsyncPgConnection,
    new_route: &NewRoute,
) -> anyhow::Result<Route> {
    let route: Route = diesel::insert_into(route::table)
        .values(new_route)
        .returning(Route::as_returning())
        .get_result(conn)
        .await?;

    Ok(route)
}

// 删除路由
pub async fn delete_route(conn: &mut AsyncPgConnection, route_id: i32) -> anyhow::Result<bool> {
    let deleted_count = diesel::delete(route::table.filter(route::id.eq(route_id)))
        .execute(conn)
        .await?;

    Ok(deleted_count > 0)
}

// 更新路由
pub async fn update_route(
    conn: &mut AsyncPgConnection,
    route_id: i32,
    updated_route: &NewRoute,
) -> anyhow::Result<Route> {
    // 检查路由是否存在
    let existing_route = find_route_by_id(conn, route_id).await?;
    if existing_route.is_none() {
        return Err(anyhow::anyhow!("route not found"));
    }

    let route = diesel::update(route::table.filter(route::id.eq(route_id)))
        .set((
            route::pattern_kind.eq(&updated_route.pattern_kind),
            route::pattern.eq(&updated_route.pattern),
            route::priority.eq(updated_route.priority),
            route::timeout.eq(updated_route.timeout),
            route::catalog.eq(&updated_route.catalog),
            route::handler_kind.eq(&updated_route.handler_kind),
            route::handler.eq(&updated_route.handler),
            route::write_log.eq(updated_route.write_log),
            route::comment.eq(&updated_route.comment),
        ))
        .returning(Route::as_returning())
        .get_result(conn)
        .await?;

    Ok(route)
}
