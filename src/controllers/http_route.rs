use std::sync::{Arc, RwLock};

use axum::{Json, extract::State};
use chrono::Utc;
use diesel_async::AsyncPgConnection;
use serde::Deserialize;

use crate::controllers::user::LoggedUser;
use crate::db::http_route::helper::{find_http_route_by_id, get_all_http_routes_except};
use crate::db::http_route::model::PatternKind;
use crate::dispatcher::{HttpDispatcher, HttpRoute as DispatcherHttpRoute};
use crate::storage::Storage;
use crate::{
    Context,
    controllers::AppError,
    db::http_route::{
        helper::{
            create_http_route as db_create_http_route, delete_http_route as db_delete_http_route,
            get_all_http_routes, update_http_route as db_update_http_route,
        },
        model::{HandlerKind, HttpRoute, NewHttpRoute},
    },
    utils::{jwt::Claims, response::Response},
};

// 创建路由请求
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateHttpRouteRequest {
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
pub struct DeleteHttpRouteRequest {
    http_route_id: i32,
}

// 更新路由请求
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateHttpRouteRequest {
    http_route_id: i32,
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
    http_route_id: i32,
    http_route: Option<NewHttpRoute>,
}

pub async fn compile_http_routes(
    conn: &mut AsyncPgConnection,
    storage: &Storage,
    modify: ModifyRequest,
) -> anyhow::Result<HttpDispatcher> {
    let mut http_routes = match modify.kind {
        ModifyKind::NEW => get_all_http_routes(conn).await?,
        ModifyKind::REPLACE | ModifyKind::DELETE => {
            // HTTP 路由必须存在
            find_http_route_by_id(conn, modify.http_route_id).await?;
            // 获取除了目标之外的所有 HTTP 路由
            get_all_http_routes_except(conn, modify.http_route_id).await?
        }
    };

    let http_routes = match modify.kind {
        ModifyKind::DELETE => http_routes,
        ModifyKind::NEW | ModifyKind::REPLACE => {
            let new_http_route = if let Some(new_http_route) = modify.http_route {
                new_http_route
            } else {
                return Err(anyhow::anyhow!(
                    "http_route must provide while new or replace http_route"
                ));
            };

            let new_http_route = HttpRoute {
                id: modify.http_route_id,
                pattern_kind: new_http_route.pattern_kind,
                pattern: new_http_route.pattern.clone(),
                priority: new_http_route.priority,
                timeout: new_http_route.timeout,
                catalog: new_http_route.catalog.clone(),
                handler_kind: new_http_route.handler_kind,
                handler: new_http_route.handler.clone(),
                write_log: new_http_route.write_log,
                comment: new_http_route.comment.clone(),
                create_time: Utc::now(),
            };

            http_routes.push(new_http_route);
            http_routes
        }
    };

    return HttpDispatcher::new(
        http_routes
            .into_iter()
            .map(|x| DispatcherHttpRoute::transform(x, storage))
            .collect::<anyhow::Result<Vec<_>>>()?,
    )
    .map_err(|err| return anyhow::anyhow!("compile new http dispatcher failed: {:?}", err));
}

pub fn install_http_dispatcher(holder: &Arc<RwLock<HttpDispatcher>>, new: HttpDispatcher) {
    let mut guard = holder.write().expect("lock poisoned");
    *guard = new;
}

// 创建路由
#[utoipa::path(post, path = "/", responses((status = OK, body = Response<HttpRoute>)))]
pub async fn create_http_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<CreateHttpRouteRequest>,
) -> Result<Response<HttpRoute>, AppError> {
    let new_http_route = NewHttpRoute {
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
    // 3. 替换 HTTP dispatcher
    let new_dispatcher = compile_http_routes(
        &mut conn,
        &ctx.storage,
        ModifyRequest {
            kind: ModifyKind::NEW,
            http_route_id: 0,
            http_route: Some(new_http_route.clone()),
        },
    )
    .await?;
    let http_route = db_create_http_route(&mut conn, &new_http_route).await?;
    install_http_dispatcher(&ctx.http_dispatcher, new_dispatcher);

    Ok(Response::<HttpRoute>::ok().payload(http_route))
}

// 删除路由
#[utoipa::path(delete, path = "/", responses((status = OK, body = Response<bool>)))]
pub async fn delete_http_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<DeleteHttpRouteRequest>,
) -> Result<Response<bool>, AppError> {
    let mut conn = ctx.db_conn().await?;

    // 1. 先检查路由是否存在
    // 2. 数据库中删除
    // 3. 替换 HTTP dispatcher
    let new_dispatcher = compile_http_routes(
        &mut conn,
        &ctx.storage,
        ModifyRequest {
            kind: ModifyKind::DELETE,
            http_route_id: request.http_route_id,
            http_route: None,
        },
    )
    .await?;
    let deleted = db_delete_http_route(&mut conn, request.http_route_id).await?;
    install_http_dispatcher(&ctx.http_dispatcher, new_dispatcher);

    if deleted {
        Ok(Response::<bool>::ok().payload(true))
    } else {
        Err(anyhow::anyhow!("http_route not found").into())
    }
}

// 查询所有 HTTP 路由
#[utoipa::path(get, path = "/", responses((status = OK, body = Response<Vec<HttpRoute>>)))]
pub async fn get_http_routes(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
) -> Result<Response<Vec<HttpRoute>>, AppError> {
    let mut conn = ctx.db_conn().await?;
    let http_routes = get_all_http_routes(&mut conn).await?;

    Ok(Response::<Vec<HttpRoute>>::ok().payload(http_routes))
}

// 更新 HTTP 路由
#[utoipa::path(patch, path = "/", responses((status = OK, body = Response<HttpRoute>)))]
pub async fn update_http_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<UpdateHttpRouteRequest>,
) -> Result<Response<HttpRoute>, AppError> {
    let updated_http_route = NewHttpRoute {
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
    // 3. 替换 HTTP dispatcher
    let new_dispatcher = compile_http_routes(
        &mut conn,
        &ctx.storage,
        ModifyRequest {
            kind: ModifyKind::REPLACE,
            http_route_id: request.http_route_id,
            http_route: Some(updated_http_route.clone()),
        },
    )
    .await?;
    let http_route =
        db_update_http_route(&mut conn, request.http_route_id, &updated_http_route).await?;
    install_http_dispatcher(&ctx.http_dispatcher, new_dispatcher);

    Ok(Response::<HttpRoute>::ok().payload(http_route))
}
