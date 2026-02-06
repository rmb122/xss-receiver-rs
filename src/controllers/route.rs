use axum::{Json, extract::State};
use serde::Deserialize;

use crate::controllers::user::LoggedUser;
use crate::{
    Context,
    controllers::AppError,
    db::route::{
        helper::{
            create_route as db_create_route, delete_route as db_delete_route, get_all_routes,
            update_route as db_update_route,
        },
        model::{NewRoute, Route, RouteKind},
    },
    utils::{jwt::Claims, response::Response},
};

// 创建路由请求
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateRouteRequest {
    kind: RouteKind,
    pattern: String,
    timeout: i32,
    catalog: String,
    handler: String,
    write_log: bool,
    comment: String,
}

// 删除路由请求
#[derive(Deserialize, utoipa::ToSchema)]
pub struct DeleteRouteRequest {
    route_id: i32,
}

// 更新路由请求
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateRouteRequest {
    route_id: i32,
    kind: RouteKind,
    pattern: String,
    timeout: i32,
    catalog: String,
    handler: String,
    write_log: bool,
    comment: String,
}

// 创建路由
#[utoipa::path(post, path = "/", responses((status = OK, body = Response<Route>)))]
pub async fn create_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<CreateRouteRequest>,
) -> Result<Response<Route>, AppError> {
    let new_route = NewRoute {
        kind: request.kind,
        pattern: request.pattern,
        timeout: request.timeout,
        catalog: request.catalog,
        handler: request.handler,
        write_log: request.write_log,
        comment: request.comment,
    };

    let mut conn = ctx.db_conn().await?;
    let route = db_create_route(&mut conn, &new_route).await?;

    Ok(Response::<Route>::ok()
        .msg("route created successfully")
        .payload(route))
}

// 删除路由
#[utoipa::path(delete, path = "/", responses((status = OK, body = Response<bool>)))]
pub async fn delete_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<DeleteRouteRequest>,
) -> Result<Response<bool>, AppError> {
    let mut conn = ctx.db_conn().await?;
    let deleted = db_delete_route(&mut conn, request.route_id).await?;

    if deleted {
        Ok(Response::<bool>::ok()
            .msg("route deleted successfully")
            .payload(true))
    } else {
        Err(anyhow::anyhow!("route not found").into())
    }
}

// 查询所有路由
#[utoipa::path(get, path = "/", responses((status = OK, body = Response<Vec<Route>>)))]
pub async fn get_routes(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
) -> Result<Response<Vec<Route>>, AppError> {
    let mut conn = ctx.db_conn().await?;
    let routes = get_all_routes(&mut conn).await?;

    Ok(Response::<Vec<Route>>::ok()
        .msg("routes retrieved successfully")
        .payload(routes))
}

// 更新路由
#[utoipa::path(patch, path = "/", responses((status = OK, body = Response<Route>)))]
pub async fn update_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<UpdateRouteRequest>,
) -> Result<Response<Route>, AppError> {
    let updated_route = NewRoute {
        kind: request.kind,
        pattern: request.pattern,
        timeout: request.timeout,
        catalog: request.catalog,
        handler: request.handler,
        write_log: request.write_log,
        comment: request.comment,
    };

    let mut conn = ctx.db_conn().await?;
    let route = db_update_route(&mut conn, request.route_id, &updated_route).await?;

    Ok(Response::<Route>::ok()
        .msg("route updated successfully")
        .payload(route))
}
