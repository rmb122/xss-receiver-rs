use anyhow::Result;
use chrono::{TimeZone, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};

use super::LogFilter;
use crate::{
    db::{
        dns_log::{
            helper::{get_dns_logs_paginated, insert_dns_log},
            model::NewDnsLog,
        },
        http_log::{
            helper::{get_http_logs_paginated, insert_http_log},
            model::{BodyKind, NewHttpLog},
        },
        schema::{dns_log, http_log},
    },
    utils::diesel_bytea,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires TEST_DATABASE_URL pointing to a disposable PostgreSQL database"]
async fn filters_http_and_dns_before_counting_and_paginating() -> Result<()> {
    let pool = crate::db::establish_db_connection(&std::env::var("TEST_DATABASE_URL")?).await?;
    crate::db::run_migrations(&pool).await?;
    let mut conn = pool.get().await?;
    conn.begin_test_transaction().await?;

    let mut http_ids = Vec::new();
    for (index, (ip, method, path, error)) in [
        ("192.0.2.1", "GET", "/api/100%_done", None),
        ("192.0.2.2", "POST", "/other", None),
        ("192.0.2.1", "POST", "/api/data", Some("script timeout")),
        ("192.0.2.1", "POST", "/api/100Xdone", Some("bad input")),
        ("192.0.2.1", "GET", "/api/quote\"' OR 1=1 --", None),
    ]
    .into_iter()
    .enumerate()
    {
        let id = insert_http_log(
            &mut conn,
            &NewHttpLog {
                client_ip: ip.into(),
                client_port: 12345,
                location: "test".into(),
                method: method.into(),
                path: path.into(),
                raw_query: String::new(),
                parsed_query: Default::default(),
                header: Default::default(),
                parsed_body_type: if index == 2 {
                    BodyKind::JSON
                } else {
                    BodyKind::NONE
                },
                parsed_body: Default::default(),
                raw_body: Vec::new(),
                file: Default::default(),
                extra_info: diesel_bytea::Json::new(serde_json::Value::Null),
                error_log: error.map(str::to_owned),
            },
        )
        .await?;
        diesel::update(http_log::table.find(id))
            .set(
                http_log::create_time.eq(Utc
                    .with_ymd_and_hms(2026, 9, 19, index as u32 + 1, 0, 0)
                    .unwrap()),
            )
            .execute(&mut conn)
            .await?;
        http_ids.push(id);
    }

    let expression = r#"client_ip = "192.0.2.1" && contains(path, "/api") && create_time < "2026-09-19 12:00:00""#;
    let filter = LogFilter::parse(Some(expression), Some("Asia/Shanghai"))?;
    for (page, expected) in [(1, vec![http_ids[2]]), (2, vec![http_ids[0]]), (3, vec![])] {
        let (logs, total) = get_http_logs_paginated(&mut conn, page, 1, &filter).await?;
        assert_eq!(total, 2);
        assert_eq!(
            logs.into_iter().map(|log| log.id).collect::<Vec<_>>(),
            expected
        );
    }

    for (expression, expected) in [
        (
            r#"method = "GET" || method = "POST" && error_log != null"#,
            vec![http_ids[4], http_ids[3], http_ids[2], http_ids[0]],
        ),
        (
            r#"(method = "GET" || method = "POST") && error_log != null"#,
            vec![http_ids[3], http_ids[2]],
        ),
        (r#"!contains(error_log, "timeout")"#, vec![http_ids[3]]),
        (r#"contains(path, "%_")"#, vec![http_ids[0]]),
        (r#"path = "/api/quote\"' OR 1=1 --""#, vec![http_ids[4]]),
        (r#"parsed_body_type = "JSON""#, vec![http_ids[2]]),
        (r#"contains(path, "API")"#, vec![]),
    ] {
        assert_http_ids(&mut conn, expression, &expected).await?;
    }
    assert_http_ids(
        &mut conn,
        "   ",
        &http_ids.iter().rev().copied().collect::<Vec<_>>(),
    )
    .await?;

    let mut dns_ids = Vec::new();
    for (index, (name, kind)) in [
        ("a.example.com.", "A"),
        ("b.other.test.", "AAAA"),
        ("c.example.com.", "A"),
    ]
    .into_iter()
    .enumerate()
    {
        let id = insert_dns_log(
            &mut conn,
            &NewDnsLog {
                client_ip: "192.0.2.1".into(),
                client_port: 5353,
                location: "test".into(),
                query_name: name.into(),
                query_type: kind.into(),
                query_class: "IN".into(),
                extra_info: diesel_bytea::Json::new(serde_json::Value::Null),
                error_log: None,
            },
        )
        .await?;
        diesel::update(dns_log::table.find(id))
            .set(
                dns_log::create_time.eq(Utc
                    .with_ymd_and_hms(2026, 9, 19, index as u32 + 1, 0, 0)
                    .unwrap()),
            )
            .execute(&mut conn)
            .await?;
        dns_ids.push(id);
    }
    let filter = LogFilter::parse(
        Some(r#"query_type = "A" && contains(query_name, "example.com")"#),
        None,
    )?;
    let (logs, total) = get_dns_logs_paginated(&mut conn, 2, 1, &filter).await?;
    assert_eq!(total, 2);
    assert_eq!(
        logs.iter().map(|log| log.id).collect::<Vec<_>>(),
        vec![dns_ids[0]]
    );
    let filter = LogFilter::parse(
        Some(r#"query_type = "A" && create_time >= "2026-09-19 10:00:00""#),
        Some("Asia/Shanghai"),
    )?;
    let (logs, total) = get_dns_logs_paginated(&mut conn, 1, 20, &filter).await?;
    assert_eq!(total, 1);
    assert_eq!(
        logs.iter().map(|log| log.id).collect::<Vec<_>>(),
        vec![dns_ids[2]]
    );
    Ok(())
}

async fn assert_http_ids(
    conn: &mut AsyncPgConnection,
    expression: &str,
    expected: &[i32],
) -> Result<()> {
    let filter = LogFilter::parse(Some(expression), None)?;
    let (logs, total) = get_http_logs_paginated(conn, 1, 20, &filter).await?;
    assert_eq!(total, expected.len() as i64, "{expression}");
    assert_eq!(
        logs.iter().map(|log| log.id).collect::<Vec<_>>(),
        expected,
        "{expression}"
    );
    Ok(())
}
