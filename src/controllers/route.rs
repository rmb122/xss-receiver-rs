use std::sync::{Arc, RwLock};

use axum::http::request;
use axum::{Json, extract::State};
use chrono::Utc;
use diesel_async::AsyncPgConnection;
use serde::Deserialize;

use crate::controllers::user::LoggedUser;
use crate::db::route::helper::{find_route_by_id, get_all_routes_except};
use crate::db::route::model::PatternKind;
use crate::dispatcher::{self, Dispatcher};
use crate::storage::Storage;
use crate::{
    Context,
    controllers::AppError,
    db::route::{
        helper::{
            create_route as db_create_route, delete_route as db_delete_route, get_all_routes,
            update_route as db_update_route,
        },
        model::{HandlerKind, NewRoute, Route},
    },
    utils::{jwt::Claims, response::Response},
};

// 创建路由请求
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateRouteRequest {
    pattern_kind: PatternKind,
    pattern: String,
    priority: i32,
    timeout: i32,
    catalog: String,
    handler_kind: HandlerKind,
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
    pattern_kind: PatternKind,
    pattern: String,
    priority: i32,
    timeout: i32,
    catalog: String,
    handler_kind: HandlerKind,
    handler: String,
    write_log: bool,
    comment: String,
}

pub enum ModifyKind {
    NEW,
    REPLACE,
    DELETE,
}

pub struct ModifyRequest {
    kind: ModifyKind,
    route_id: i32,
    route: Option<NewRoute>,
}

pub async fn compile_routes(
    conn: &mut AsyncPgConnection,
    storage: &Storage,
    modify: ModifyRequest,
) -> anyhow::Result<Dispatcher> {
    let mut routes = match modify.kind {
        ModifyKind::NEW => get_all_routes(conn).await?,
        ModifyKind::REPLACE | ModifyKind::DELETE => {
            // route 必须存在
            find_route_by_id(conn, modify.route_id).await?;
            // 获取除了目标之外的所有 route
            get_all_routes_except(conn, modify.route_id).await?
        }
    };

    let routes = match modify.kind {
        ModifyKind::DELETE => routes,
        ModifyKind::NEW | ModifyKind::REPLACE => {
            let new_route = if let Some(new_route) = modify.route {
                new_route
            } else {
                return Err(anyhow::anyhow!(
                    "route must provide while new or replace route"
                ));
            };

            let new_route = Route {
                id: modify.route_id,
                pattern_kind: new_route.pattern_kind,
                pattern: new_route.pattern.clone(),
                priority: new_route.priority,
                timeout: new_route.timeout,
                catalog: new_route.catalog.clone(),
                handler_kind: new_route.handler_kind,
                handler: new_route.handler.clone(),
                write_log: new_route.write_log,
                comment: new_route.comment.clone(),
                create_time: Utc::now().naive_utc(),
            };

            routes.push(new_route);
            routes
        }
    };

    return Dispatcher::new(
        routes
            .into_iter()
            .map(|x| dispatcher::Route::transform(x, storage))
            .collect::<anyhow::Result<Vec<_>>>()?,
    )
    .map_err(|err| return anyhow::anyhow!("compile new dispatcher failed: {:?}", err));
}

pub fn install_dispatcher(holder: &Arc<RwLock<Dispatcher>>, new: Dispatcher) {
    let mut guard = holder.write().expect("lock poisoned");
    *guard = new;
}

// 创建路由
#[utoipa::path(post, path = "/", responses((status = OK, body = Response<Route>)))]
pub async fn create_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<CreateRouteRequest>,
) -> Result<Response<Route>, AppError> {
    let new_route = NewRoute {
        pattern_kind: request.pattern_kind,
        pattern: request.pattern.clone(),
        priority: request.priority,
        timeout: request.timeout,
        catalog: request.catalog.clone(),
        handler_kind: request.handler_kind,
        handler: request.handler,
        write_log: request.write_log,
        comment: request.comment,
    };

    let mut conn = ctx.db_conn().await?;

    // 1. 先检查路由数据是否合法
    // 2. 数据库中新建
    // 3. 替换 dispatcher
    let new_dispatcher = compile_routes(
        &mut conn,
        &ctx.storage,
        ModifyRequest {
            kind: ModifyKind::NEW,
            route_id: 0,
            route: Some(new_route.clone()),
        },
    )
    .await?;
    let route = db_create_route(&mut conn, &new_route).await?;
    install_dispatcher(&ctx.dispatcher, new_dispatcher);

    Ok(Response::<Route>::ok().payload(route))
}

// 删除路由
#[utoipa::path(delete, path = "/", responses((status = OK, body = Response<bool>)))]
pub async fn delete_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<DeleteRouteRequest>,
) -> Result<Response<bool>, AppError> {
    let mut conn = ctx.db_conn().await?;

    // 1. 先检查路由是否存在
    // 2. 数据库中删除
    // 3. 替换 dispatcher
    let new_dispatcher = compile_routes(
        &mut conn,
        &ctx.storage,
        ModifyRequest {
            kind: ModifyKind::DELETE,
            route_id: request.route_id,
            route: None,
        },
    )
    .await?;
    let deleted = db_delete_route(&mut conn, request.route_id).await?;
    install_dispatcher(&ctx.dispatcher, new_dispatcher);

    if deleted {
        Ok(Response::<bool>::ok().payload(true))
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

    Ok(Response::<Vec<Route>>::ok().payload(routes))
}

// 更新路由
#[utoipa::path(patch, path = "/", responses((status = OK, body = Response<Route>)))]
pub async fn update_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<UpdateRouteRequest>,
) -> Result<Response<Route>, AppError> {
    let updated_route = NewRoute {
        pattern_kind: request.pattern_kind,
        pattern: request.pattern,
        priority: request.priority,
        timeout: request.timeout,
        catalog: request.catalog,
        handler_kind: request.handler_kind,
        handler: request.handler,
        write_log: request.write_log,
        comment: request.comment,
    };

    let mut conn = ctx.db_conn().await?;

    // 1. 先检查路由是否正确
    // 2. 数据库更新成功
    // 3. 替换 dispatcher
    let new_dispatcher = compile_routes(
        &mut conn,
        &ctx.storage,
        ModifyRequest {
            kind: ModifyKind::REPLACE,
            route_id: request.route_id,
            route: Some(updated_route.clone()),
        },
    )
    .await?;
    let route = db_update_route(&mut conn, request.route_id, &updated_route).await?;
    install_dispatcher(&ctx.dispatcher, new_dispatcher);

    Ok(Response::<Route>::ok().payload(route))
}
