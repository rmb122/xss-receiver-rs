use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::db::{
    dns_route::model::{DnsRoute, NewDnsRoute},
    schema::dns_route,
};

pub async fn get_all_dns_routes(conn: &mut AsyncPgConnection) -> anyhow::Result<Vec<DnsRoute>> {
    Ok(dns_route::table
        .select(DnsRoute::as_select())
        .order(dns_route::id.asc())
        .get_results(conn)
        .await?)
}

pub async fn get_all_dns_routes_except(
    conn: &mut AsyncPgConnection,
    route_id: i32,
) -> anyhow::Result<Vec<DnsRoute>> {
    Ok(dns_route::table
        .select(DnsRoute::as_select())
        .filter(dns_route::id.ne(route_id))
        .order(dns_route::id.asc())
        .get_results(conn)
        .await?)
}

pub async fn find_dns_route_by_id(
    conn: &mut AsyncPgConnection,
    route_id: i32,
) -> anyhow::Result<Option<DnsRoute>> {
    Ok(dns_route::table
        .filter(dns_route::id.eq(route_id))
        .select(DnsRoute::as_select())
        .first(conn)
        .await
        .optional()?)
}

pub async fn create_dns_route(
    conn: &mut AsyncPgConnection,
    new_route: &NewDnsRoute,
) -> anyhow::Result<DnsRoute> {
    Ok(diesel::insert_into(dns_route::table)
        .values(new_route)
        .returning(DnsRoute::as_returning())
        .get_result(conn)
        .await?)
}

pub async fn delete_dns_route(conn: &mut AsyncPgConnection, route_id: i32) -> anyhow::Result<bool> {
    let deleted_count = diesel::delete(dns_route::table.filter(dns_route::id.eq(route_id)))
        .execute(conn)
        .await?;

    Ok(deleted_count > 0)
}

pub async fn update_dns_route(
    conn: &mut AsyncPgConnection,
    route_id: i32,
    updated_route: &NewDnsRoute,
) -> anyhow::Result<DnsRoute> {
    if find_dns_route_by_id(conn, route_id).await?.is_none() {
        return Err(anyhow::anyhow!("dns route not found"));
    }

    Ok(
        diesel::update(dns_route::table.filter(dns_route::id.eq(route_id)))
            .set((
                dns_route::pattern_kind.eq(&updated_route.pattern_kind),
                dns_route::pattern.eq(&updated_route.pattern),
                dns_route::priority.eq(updated_route.priority),
                dns_route::timeout.eq(updated_route.timeout),
                dns_route::catalog.eq(&updated_route.catalog),
                dns_route::handler_kind.eq(&updated_route.handler_kind),
                dns_route::handler.eq(&updated_route.handler),
                dns_route::write_log.eq(updated_route.write_log),
                dns_route::comment.eq(&updated_route.comment),
            ))
            .returning(DnsRoute::as_returning())
            .get_result(conn)
            .await?,
    )
}
