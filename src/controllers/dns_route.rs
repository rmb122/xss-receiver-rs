use std::sync::{Arc, RwLock};

use axum::{Json, extract::State};
use chrono::Utc;
use diesel_async::AsyncPgConnection;
use serde::Deserialize;

use crate::controllers::user::LoggedUser;
use crate::db::dns_route::{
    helper::{
        create_dns_route as db_create_dns_route, delete_dns_route as db_delete_dns_route,
        find_dns_route_by_id, get_all_dns_routes, get_all_dns_routes_except,
        update_dns_route as db_update_dns_route,
    },
    model::{DnsRoute, HandlerKind, NewDnsRoute, PatternKind},
};
use crate::dispatcher::{self, DnsDispatcher, ScriptCache};
use crate::storage::Storage;
use crate::{
    Context,
    controllers::AppError,
    utils::{jwt::Claims, response::Response},
};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateDnsRouteRequest {
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

#[derive(Deserialize, utoipa::ToSchema)]
pub struct DeleteDnsRouteRequest {
    route_id: i32,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateDnsRouteRequest {
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
    route: Option<NewDnsRoute>,
}

pub async fn compile_dns_routes(
    conn: &mut AsyncPgConnection,
    storage: &Storage,
    cache: ScriptCache,
    modify: ModifyRequest,
) -> anyhow::Result<DnsDispatcher> {
    let mut routes = match modify.kind {
        ModifyKind::NEW => get_all_dns_routes(conn).await?,
        ModifyKind::REPLACE | ModifyKind::DELETE => {
            find_dns_route_by_id(conn, modify.route_id).await?;
            get_all_dns_routes_except(conn, modify.route_id).await?
        }
    };

    let routes = match modify.kind {
        ModifyKind::DELETE => routes,
        ModifyKind::NEW | ModifyKind::REPLACE => {
            let new_route = modify
                .route
                .ok_or_else(|| anyhow::anyhow!("dns route must provide while new or replace"))?;

            routes.push(DnsRoute {
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
                create_time: Utc::now(),
            });
            routes
        }
    };

    DnsDispatcher::new(
        routes
            .into_iter()
            .map(|x| dispatcher::DnsRoute::transform(x, storage, cache.clone()))
            .collect::<anyhow::Result<Vec<_>>>()?,
    )
    .map_err(|err| anyhow::anyhow!("compile new dns dispatcher failed: {:?}", err))
}

pub fn install_dns_dispatcher(holder: &Arc<RwLock<DnsDispatcher>>, new: DnsDispatcher) {
    let mut guard = holder.write().expect("lock poisoned");
    *guard = new;
}

/// 创建 DNS 路由
#[utoipa::path(post, path = "/", responses((status = OK, body = Response<DnsRoute>)))]
pub async fn create_dns_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<CreateDnsRouteRequest>,
) -> Result<Response<DnsRoute>, AppError> {
    let new_route = NewDnsRoute {
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
    let new_dispatcher = compile_dns_routes(
        &mut conn,
        &ctx.storage,
        ctx.script_cache.clone(),
        ModifyRequest {
            kind: ModifyKind::NEW,
            route_id: 0,
            route: Some(new_route.clone()),
        },
    )
    .await?;
    let route = db_create_dns_route(&mut conn, &new_route).await?;
    install_dns_dispatcher(&ctx.dns_dispatcher, new_dispatcher);

    Ok(Response::<DnsRoute>::ok().payload(route))
}

/// 删除 DNS 路由
#[utoipa::path(delete, path = "/", responses((status = OK, body = Response<bool>)))]
pub async fn delete_dns_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<DeleteDnsRouteRequest>,
) -> Result<Response<bool>, AppError> {
    let mut conn = ctx.db_conn().await?;
    let new_dispatcher = compile_dns_routes(
        &mut conn,
        &ctx.storage,
        ctx.script_cache.clone(),
        ModifyRequest {
            kind: ModifyKind::DELETE,
            route_id: request.route_id,
            route: None,
        },
    )
    .await?;
    let deleted = db_delete_dns_route(&mut conn, request.route_id).await?;
    install_dns_dispatcher(&ctx.dns_dispatcher, new_dispatcher);

    if deleted {
        Ok(Response::<bool>::ok().payload(true))
    } else {
        Err(anyhow::anyhow!("dns route not found").into())
    }
}

/// 获取 DNS 路由列表
#[utoipa::path(get, path = "/", responses((status = OK, body = Response<Vec<DnsRoute>>)))]
pub async fn get_dns_routes(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
) -> Result<Response<Vec<DnsRoute>>, AppError> {
    let mut conn = ctx.db_conn().await?;
    let routes = get_all_dns_routes(&mut conn).await?;

    Ok(Response::<Vec<DnsRoute>>::ok().payload(routes))
}

/// 更新 DNS 路由
#[utoipa::path(patch, path = "/", responses((status = OK, body = Response<DnsRoute>)))]
pub async fn update_dns_route(
    State(ctx): State<Context>,
    Claims(_user): Claims<LoggedUser>,
    Json(request): Json<UpdateDnsRouteRequest>,
) -> Result<Response<DnsRoute>, AppError> {
    let updated_route = NewDnsRoute {
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
    let new_dispatcher = compile_dns_routes(
        &mut conn,
        &ctx.storage,
        ctx.script_cache.clone(),
        ModifyRequest {
            kind: ModifyKind::REPLACE,
            route_id: request.route_id,
            route: Some(updated_route.clone()),
        },
    )
    .await?;
    let route = db_update_dns_route(&mut conn, request.route_id, &updated_route).await?;
    install_dns_dispatcher(&ctx.dns_dispatcher, new_dispatcher);

    Ok(Response::<DnsRoute>::ok().payload(route))
}
