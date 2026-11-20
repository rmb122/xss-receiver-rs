use std::net::IpAddr;

use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{AppError, Context, auth::AuthenticatedUser};
use crate::{
    db::token::{
        helper,
        model::{NewToken, TokenInfo},
    },
    utils::{
        api_token::{generate_token, hash_token},
        response::Response,
    },
};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateTokenRequest {
    name: String,
    expires_at: Option<DateTime<Utc>>,
    allowed_ip: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct DeleteTokenRequest {
    token_id: i32,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct IssuedToken {
    #[serde(flatten)]
    info: TokenInfo,
    token: String,
}

/// Issue a token through a password-login session. The secret is returned only here.
#[utoipa::path(post, path = "/", responses((status = OK, body = Response<IssuedToken>)))]
pub async fn create_token(
    State(ctx): State<Context>,
    auth: AuthenticatedUser,
    Json(request): Json<CreateTokenRequest>,
) -> Result<Response<IssuedToken>, AppError> {
    auth.require_session()?;
    let user = auth.user;
    let name = request.name.trim();
    if name.is_empty() {
        return Err(anyhow::anyhow!("token name must not be empty").into());
    }
    if request
        .expires_at
        .is_some_and(|expiry| expiry <= Utc::now())
    {
        return Err(anyhow::anyhow!("expires_at must be in the future").into());
    }
    let allowed_ip = match request.allowed_ip.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(value) => Some(
            value
                .parse::<IpAddr>()
                .map_err(|_| anyhow::anyhow!("allowed_ip must be a single IPv4 or IPv6 address"))?
                .to_canonical()
                .to_string(),
        ),
    };

    let token = generate_token();
    let token_hash = hash_token(&token);
    let mut conn = ctx.db_conn().await?;
    let info = helper::create_token(
        &mut conn,
        &NewToken {
            user_id: user.id,
            name,
            token_hash: &token_hash,
            expires_at: request.expires_at,
            allowed_ip: allowed_ip.as_deref(),
        },
    )
    .await?;
    Ok(Response::ok().payload(IssuedToken { info, token }))
}

/// List the user's tokens, or all tokens for an administrator.
#[utoipa::path(get, path = "/", responses((status = OK, body = Response<Vec<TokenInfo>>)))]
pub async fn get_tokens(
    State(ctx): State<Context>,
    AuthenticatedUser { user, .. }: AuthenticatedUser,
) -> Result<Response<Vec<TokenInfo>>, AppError> {
    let mut conn = ctx.db_conn().await?;
    Ok(Response::ok().payload(helper::get_tokens(&mut conn, user.id).await?))
}

/// Revoke an owned token. Administrators can revoke any token.
#[utoipa::path(delete, path = "/", responses((status = OK, body = Response<bool>)))]
pub async fn delete_token(
    State(ctx): State<Context>,
    AuthenticatedUser { user, .. }: AuthenticatedUser,
    Json(request): Json<DeleteTokenRequest>,
) -> Result<Response<bool>, AppError> {
    let mut conn = ctx.db_conn().await?;
    if !helper::delete_token(&mut conn, user.id, request.token_id).await? {
        return Err(anyhow::anyhow!("token not found or permission denied").into());
    }
    Ok(Response::ok().payload(true))
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::{Body, to_bytes},
        extract::ConnectInfo,
        http::{Request, StatusCode, header},
    };
    use chrono::Duration;
    use diesel::prelude::*;
    use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl, SimpleAsyncConnection};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;
    use crate::{
        controllers::{auth::LoggedUser, get_app_router},
        db::{self, schema::api_tokens, user::helper as users},
    };

    async fn with_database(
        exercise: impl AsyncFnOnce(Context) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let database_url = std::env::var("TEST_DATABASE_URL")?;
        let schema = format!(
            "token_test_{}",
            hex::encode(crate::utils::random::get_random_bytes(8))
        );
        let storage_path = std::env::temp_dir().join(&schema);
        let mut admin = AsyncPgConnection::establish(&database_url).await?;
        admin
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .await?;

        let result = async {
            let mut url = url::Url::parse(&database_url)?;
            url.query_pairs_mut()
                .append_pair("options", &format!("-csearch_path={schema}"));
            let pool = db::establish_db_connection(url.as_str()).await?;
            db::run_migrations(&pool).await?;
            users::create_init_admin_user(&pool).await?;
            let mut config =
                crate::startup_config::parse(include_str!("../../config_example.toml"))?;
            config.storage_path = storage_path.to_string_lossy().into_owned();
            config.http_server.admin_prefix = "/test-admin/".into();
            config.ip2region.ipv4_db.clear();
            config.ip2region.ipv6_db.clear();
            exercise(Context::new(&config, pool).await?).await
        }
        .await;

        admin
            .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .await?;
        if storage_path.exists() {
            std::fs::remove_dir_all(storage_path)?;
        }
        result
    }

    fn request(method: &str, path: &str, token: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(format!("/test-admin/api{path}"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .extension(ConnectInfo(
                "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
            ))
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn with_peer(mut request: Request<Body>, address: &str) -> Request<Body> {
        request.extensions_mut().insert(ConnectInfo(
            address.parse::<std::net::SocketAddr>().unwrap(),
        ));
        request
    }

    async fn call(app: &Router, request: Request<Body>) -> anyhow::Result<Value> {
        let response = app.clone().oneshot(request).await?;
        anyhow::ensure!(response.status() == StatusCode::OK);
        Ok(serde_json::from_slice(
            &to_bytes(response.into_body(), 1024 * 1024).await?,
        )?)
    }

    async fn issue(app: &Router, jwt: &str, body: Value) -> anyhow::Result<Value> {
        let response = call(app, request("POST", "/token", jwt, body)).await?;
        anyhow::ensure!(
            response["code"] == 200,
            "token creation failed: {}",
            response["msg"]
        );
        Ok(response["payload"].clone())
    }

    fn jwt(ctx: &Context, id: i32, username: &str) -> anyhow::Result<String> {
        ctx.jwt_manager.encode_token(LoggedUser {
            id,
            username: username.into(),
        })
    }

    // Run with TEST_DATABASE_URL=postgres://... cargo test controllers::token -- --ignored
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires PostgreSQL via TEST_DATABASE_URL"]
    async fn tokens_expire_revoke_and_follow_account_lifecycle() -> anyhow::Result<()> {
        with_database(async |ctx| {
            let owner = {
                let mut conn = ctx.db_conn().await?;
                users::create_user(&mut conn, "owner", "password").await?
            };
            let jwt = jwt(&ctx, owner.id, &owner.username)?;
            let app = get_app_router(ctx.clone());
            for body in [
                json!({"name": "  "}),
                json!({
                    "name": "past", "expires_at": Utc::now() - Duration::seconds(1),
                }),
            ] {
                let response = call(&app, request("POST", "/token", &jwt, body)).await?;
                anyhow::ensure!(response["code"] == 500);
            }

            let permanent = issue(&app, &jwt, json!({"name": "  agent  "})).await?;
            let token = permanent["token"].as_str().unwrap();
            let id = permanent["id"].as_i64().unwrap() as i32;
            anyhow::ensure!(permanent["expires_at"].is_null() && permanent["name"] == "agent");
            {
                let mut conn = ctx.db_conn().await?;
                let stored: String = api_tokens::table
                    .find(id)
                    .select(api_tokens::token_hash)
                    .first(&mut conn)
                    .await?;
                anyhow::ensure!(stored == hash_token(token) && stored != token);
            }
            let current = call(&app, request("GET", "/user/current", token, Value::Null)).await?;
            anyhow::ensure!(current["payload"]["id"] == owner.id);

            let expiring = issue(
                &app,
                &jwt,
                json!({
                    "name": "temporary", "expires_at": Utc::now() + Duration::hours(1),
                }),
            )
            .await?;
            let expiring_token = expiring["token"].as_str().unwrap();
            anyhow::ensure!(
                call(
                    &app,
                    request("GET", "/user/current", expiring_token, Value::Null)
                )
                .await?["code"]
                    == 200
            );
            {
                let mut conn = ctx.db_conn().await?;
                diesel::update(api_tokens::table.find(expiring["id"].as_i64().unwrap() as i32))
                    .set(api_tokens::expires_at.eq(Utc::now()))
                    .execute(&mut conn)
                    .await?;
            }
            anyhow::ensure!(
                call(
                    &app,
                    request("GET", "/user/current", expiring_token, Value::Null)
                )
                .await?["code"]
                    == 400
            );
            let listing = call(&app, request("GET", "/token", &jwt, Value::Null)).await?;
            let records = listing["payload"].as_array().unwrap();
            anyhow::ensure!(records.len() == 2 && records[0]["id"] == expiring["id"]);
            anyhow::ensure!(
                records
                    .iter()
                    .all(|record| record.get("token").is_none()
                        && record.get("token_hash").is_none())
            );

            anyhow::ensure!(
                call(
                    &app,
                    request("DELETE", "/token", &jwt, json!({"token_id": id}))
                )
                .await?["payload"]
                    == true
            );
            anyhow::ensure!(
                call(&app, request("GET", "/user/current", token, Value::Null)).await?["code"]
                    == 400
            );

            let durable = issue(&app, &jwt, json!({"name": "durable", "expires_at": null})).await?;
            let token = durable["token"].as_str().unwrap();
            {
                let mut conn = ctx.db_conn().await?;
                users::update_user(&mut conn, owner.id, Some("renamed"), Some("new password"))
                    .await?;
            }
            let mut config = (*ctx.config).clone();
            config.http_server.jwt_secret = "changed key".into();
            config.http_server.jwt_expire_time = 1;
            let restarted = get_app_router(Context::new(&config, ctx.pool.clone()).await?);
            let current = call(
                &restarted,
                request("GET", "/user/current", token, Value::Null),
            )
            .await?;
            anyhow::ensure!(current["payload"]["username"] == "renamed");
            {
                let mut conn = ctx.db_conn().await?;
                users::delete_user(&mut conn, owner.id).await?;
                let count: i64 = api_tokens::table.count().get_result(&mut conn).await?;
                anyhow::ensure!(count == 0);
            }
            anyhow::ensure!(
                call(
                    &restarted,
                    request("GET", "/user/current", token, Value::Null)
                )
                .await?["code"]
                    == 400
            );
            Ok(())
        })
        .await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires PostgreSQL via TEST_DATABASE_URL"]
    async fn token_permissions_and_existing_login_authentication() -> anyhow::Result<()> {
        with_database(async |ctx| {
            let (alice, bob) = {
                let mut conn = ctx.db_conn().await?;
                (
                    users::create_user(&mut conn, "alice", "password").await?,
                    users::create_user(&mut conn, "bob", "password").await?,
                )
            };
            let app = get_app_router(ctx.clone());
            let admin_jwt = jwt(&ctx, 1, "admin")?;
            let alice_jwt = jwt(&ctx, alice.id, "alice")?;
            let bob_jwt = jwt(&ctx, bob.id, "bob")?;
            let admin = issue(&app, &admin_jwt, json!({"name": "admin agent"})).await?;
            let alice_token = issue(&app, &alice_jwt, json!({"name": "alice agent"})).await?;
            let bob_token = issue(&app, &bob_jwt, json!({"name": "bob agent"})).await?;
            let admin = admin["token"].as_str().unwrap();
            let alice_secret = alice_token["token"].as_str().unwrap();
            let bob_secret = bob_token["token"].as_str().unwrap();

            let own = call(&app, request("GET", "/token", alice_secret, Value::Null)).await?;
            anyhow::ensure!(
                own["payload"].as_array().unwrap().len() == 1
                    && own["payload"][0]["user_id"] == alice.id
            );
            let all = call(&app, request("GET", "/token", admin, Value::Null)).await?;
            anyhow::ensure!(all["payload"].as_array().unwrap().len() == 3);
            anyhow::ensure!(
                call(&app, request("GET", "/user", alice_secret, Value::Null)).await?["code"]
                    == 500
            );
            anyhow::ensure!(
                call(&app, request("GET", "/user", admin, Value::Null)).await?["code"] == 200
            );
            anyhow::ensure!(
                call(
                    &app,
                    request("GET", "/http_route", alice_secret, Value::Null)
                )
                .await?["code"]
                    == 200
            );
            anyhow::ensure!(
                call(&app, request("GET", "/dns_log", alice_secret, Value::Null)).await?["code"]
                    == 200
            );
            anyhow::ensure!(
                call(
                    &app,
                    request("POST", "/file/list", alice_secret, json!({"path": ""}))
                )
                .await?["code"]
                    == 200
            );
            let child = call(
                &app,
                request(
                    "POST",
                    "/token",
                    alice_secret,
                    json!({"name": "another agent"}),
                ),
            )
            .await?;
            anyhow::ensure!(child["code"] == 500);

            let bob_id = json!({"token_id": bob_token["id"]});
            anyhow::ensure!(
                call(
                    &app,
                    request("DELETE", "/token", alice_secret, bob_id.clone())
                )
                .await?["code"]
                    == 500
            );
            anyhow::ensure!(
                call(
                    &app,
                    request("GET", "/user/current", bob_secret, Value::Null)
                )
                .await?["code"]
                    == 200
            );
            anyhow::ensure!(
                call(&app, request("DELETE", "/token", admin, bob_id)).await?["payload"] == true
            );
            anyhow::ensure!(
                call(
                    &app,
                    request("GET", "/user/current", bob_secret, Value::Null)
                )
                .await?["code"]
                    == 400
            );

            let login = request(
                "POST",
                "/user/login",
                "",
                json!({"username": "alice", "password": "password"}),
            );
            let response = app.clone().oneshot(login).await?;
            let cookie = response.headers()[header::SET_COOKIE]
                .to_str()?
                .split(';')
                .next()
                .unwrap()
                .to_owned();
            let mut current = request("GET", "/user/current", "", Value::Null);
            current.headers_mut().remove(header::AUTHORIZATION);
            current
                .headers_mut()
                .insert(header::COOKIE, cookie.parse()?);
            anyhow::ensure!(call(&app, current).await?["payload"]["id"] == alice.id);

            let mut current = request("GET", "/user/current", "invalid", Value::Null);
            current
                .headers_mut()
                .insert(header::COOKIE, cookie.parse()?);
            anyhow::ensure!(call(&app, current).await?["code"] == 400);
            let mut current = request("GET", "/user/current", "", Value::Null);
            current
                .headers_mut()
                .insert(header::AUTHORIZATION, admin.parse()?);
            current
                .headers_mut()
                .insert(header::COOKIE, cookie.parse()?);
            anyhow::ensure!(call(&app, current).await?["payload"]["id"] == 1);
            Ok(())
        })
        .await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires PostgreSQL via TEST_DATABASE_URL"]
    async fn api_tokens_cannot_create_credentials_or_delete_users() -> anyhow::Result<()> {
        with_database(async |ctx| {
            let app = get_app_router(ctx.clone());
            let session = jwt(&ctx, 1, "admin")?;
            let issued = issue(&app, &session, json!({"name": "admin agent"})).await?;
            let token = issued["token"].as_str().unwrap();
            let created = call(
                &app,
                request(
                    "POST",
                    "/user",
                    &session,
                    json!({
                        "username": "session_user", "password": "original password",
                    }),
                ),
            )
            .await?;
            anyhow::ensure!(created["code"] == 200);
            let user_id = created["payload"]["id"].as_i64().unwrap() as i32;

            for (method, path, body) in [
                (
                    "POST",
                    "/user",
                    json!({"username": "blocked_user", "password": "password"}),
                ),
                ("DELETE", "/user", json!({"user_id": user_id})),
                ("POST", "/token", json!({"name": "blocked token"})),
                (
                    "PATCH",
                    "/user",
                    json!({"user_id": user_id, "password": "blocked password"}),
                ),
            ] {
                let mut attempt = request(method, path, token, body);
                // A valid login cookie must not override the API token in the header.
                attempt
                    .headers_mut()
                    .insert(header::COOKIE, format!("authorization={session}").parse()?);
                anyhow::ensure!(call(&app, attempt).await?["code"] == 500);
            }

            let mut cookie_token = request("POST", "/token", "", json!({"name": "cookie token"}));
            cookie_token.headers_mut().remove(header::AUTHORIZATION);
            cookie_token
                .headers_mut()
                .insert(header::COOKIE, format!("authorization={token}").parse()?);
            anyhow::ensure!(call(&app, cookie_token).await?["code"] == 500);

            {
                let mut conn = ctx.db_conn().await?;
                anyhow::ensure!(
                    users::find_user_by_username(&mut conn, "blocked_user")
                        .await?
                        .is_none()
                );
                let user = users::get_user_by_id(&mut conn, user_id).await?.unwrap();
                anyhow::ensure!(
                    password_auth::verify_password("original password", &user.password).is_ok()
                );
                let count: i64 = api_tokens::table.count().get_result(&mut conn).await?;
                anyhow::ensure!(count == 1);
            }

            let mut cookie_session =
                request("POST", "/token", "", json!({"name": "session token"}));
            cookie_session.headers_mut().remove(header::AUTHORIZATION);
            cookie_session
                .headers_mut()
                .insert(header::COOKIE, format!("authorization={session}").parse()?);
            anyhow::ensure!(call(&app, cookie_session).await?["code"] == 200);
            anyhow::ensure!(
                call(
                    &app,
                    request(
                        "PATCH",
                        "/user",
                        &session,
                        json!({
                            "user_id": user_id, "password": "new password",
                        })
                    )
                )
                .await?["code"]
                    == 200
            );
            anyhow::ensure!(
                call(
                    &app,
                    request("DELETE", "/user", &session, json!({"user_id": user_id}))
                )
                .await?["payload"]
                    == true
            );
            Ok(())
        })
        .await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires PostgreSQL via TEST_DATABASE_URL"]
    async fn token_ip_restrictions_match_client_addresses() -> anyhow::Result<()> {
        with_database(async |ctx| {
            let app = get_app_router(ctx.clone());
            let admin = jwt(&ctx, 1, "admin")?;
            let invalid = call(
                &app,
                request(
                    "POST",
                    "/token",
                    &admin,
                    json!({
                        "name": "invalid", "allowed_ip": "192.0.2.999",
                    }),
                ),
            )
            .await?;
            anyhow::ensure!(invalid["code"] == 500);

            for (allowed_ip, matching_peer, other_peer) in [
                (
                    " 192.0.2.10 ",
                    "[::ffff:192.0.2.10]:12345",
                    "192.0.2.11:12345",
                ),
                (
                    "2001:0db8:0:0:0:0:0:10",
                    "[2001:db8::10]:12345",
                    "[2001:db8::11]:12345",
                ),
            ] {
                let issued = issue(
                    &app,
                    &admin,
                    json!({"name": "restricted", "allowed_ip": allowed_ip}),
                )
                .await?;
                let token = issued["token"].as_str().unwrap();
                let matching = with_peer(
                    request("GET", "/user/current", token, Value::Null),
                    matching_peer,
                );
                anyhow::ensure!(call(&app, matching).await?["code"] == 200);
                let other = with_peer(
                    request("GET", "/user/current", token, Value::Null),
                    other_peer,
                );
                anyhow::ensure!(call(&app, other).await?["code"] == 400);
            }

            let restricted = issue(
                &app,
                &admin,
                json!({"name": "local", "allowed_ip": "127.0.0.1"}),
            )
            .await?;
            let mut unknown = request(
                "GET",
                "/user/current",
                restricted["token"].as_str().unwrap(),
                Value::Null,
            );
            unknown
                .extensions_mut()
                .remove::<ConnectInfo<std::net::SocketAddr>>();
            anyhow::ensure!(call(&app, unknown).await?["code"] == 400);

            let unrestricted = issue(
                &app,
                &admin,
                json!({"name": "anywhere", "allowed_ip": "  "}),
            )
            .await?;
            anyhow::ensure!(unrestricted["allowed_ip"].is_null());
            let mut unknown = request(
                "GET",
                "/user/current",
                unrestricted["token"].as_str().unwrap(),
                Value::Null,
            );
            unknown
                .extensions_mut()
                .remove::<ConnectInfo<std::net::SocketAddr>>();
            anyhow::ensure!(call(&app, unknown).await?["code"] == 200);
            Ok(())
        })
        .await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires PostgreSQL via TEST_DATABASE_URL"]
    async fn token_ip_restrictions_use_only_the_configured_address_header() -> anyhow::Result<()> {
        with_database(async |ctx| {
            let app = get_app_router(ctx.clone());
            let admin = jwt(&ctx, 1, "admin")?;
            let issued = issue(
                &app,
                &admin,
                json!({"name": "proxied", "allowed_ip": "192.0.2.10"}),
            )
            .await?;
            let token = issued["token"].as_str().unwrap();

            let mut forged = request("GET", "/user/current", token, Value::Null);
            forged
                .headers_mut()
                .insert("x-real-client", "192.0.2.10:12345".parse()?);
            forged
                .headers_mut()
                .insert("x-forwarded-for", "192.0.2.10".parse()?);
            anyhow::ensure!(call(&app, forged).await?["code"] == 400);

            let mut config = (*ctx.config).clone();
            config.http_server.real_addr_header = "x-real-client".into();
            let proxied = get_app_router(Context::new(&config, ctx.pool.clone()).await?);
            let mut matching = request("GET", "/user/current", token, Value::Null);
            matching
                .headers_mut()
                .insert("x-real-client", "192.0.2.10:12345".parse()?);
            anyhow::ensure!(call(&proxied, matching).await?["code"] == 200);

            let mut other = with_peer(
                request("GET", "/user/current", token, Value::Null),
                "192.0.2.10:12345",
            );
            other
                .headers_mut()
                .insert("x-real-client", "192.0.2.11:12345".parse()?);
            anyhow::ensure!(call(&proxied, other).await?["code"] == 400);

            let mut malformed = request("GET", "/user/current", token, Value::Null);
            malformed
                .headers_mut()
                .insert("x-real-client", "not-an-address".parse()?);
            anyhow::ensure!(call(&proxied, malformed).await?["code"] == 400);
            Ok(())
        })
        .await
    }
}
