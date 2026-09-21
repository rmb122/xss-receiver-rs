mod parser;
mod time;

use anyhow::{Result, anyhow};
use chrono_tz::Tz;
use diesel::{
    dsl::not,
    pg::Pg,
    prelude::*,
    sql_types::{Binary, Bool, Integer, Nullable, Text},
};

use crate::db::{
    http_log::model::BodyKind,
    schema::{dns_log, http_log},
};
use parser::{Condition, Expression, Operator, Value};
use time::parse_time;

type Predicate<Table> = Box<dyn BoxableExpression<Table, Pg, SqlType = Nullable<Bool>>>;

diesel::define_sql_function! {
    fn strpos(value: Nullable<Text>, substring: Text) -> Nullable<Integer>;
}

diesel::define_sql_function! {
    #[sql_name = "pg_catalog.position"]
    fn byte_position(value: Binary, substring: Binary) -> Integer;
}

// Keep column types visible to Diesel while sharing the operator mapping.
macro_rules! compare {
    ($column:expr, $condition:expr, $value:expr) => {{
        let condition = $condition;
        let value = $value;
        Ok::<Predicate<_>, anyhow::Error>(match condition.operator {
            Operator::Equal => Box::new($column.eq(value).nullable()),
            Operator::NotEqual => Box::new($column.ne(value).nullable()),
            Operator::Less => Box::new($column.lt(value).nullable()),
            Operator::LessEqual => Box::new($column.le(value).nullable()),
            Operator::Greater => Box::new($column.gt(value).nullable()),
            Operator::GreaterEqual => Box::new($column.ge(value).nullable()),
            Operator::Contains => {
                return Err(condition.error("contains requires a text field or raw_body"));
            }
        })
    }};
}

macro_rules! text {
    ($column:expr, $condition:expr) => {{
        let condition = $condition;
        let value = condition.string()?.to_owned();
        if value.contains('\0') {
            return Err(condition.error("text filters cannot contain a null character"));
        }
        match condition.operator {
            Operator::Equal | Operator::NotEqual => compare!($column, condition, value),
            Operator::Contains => {
                Ok::<Predicate<_>, anyhow::Error>(Box::new(strpos($column.nullable(), value).gt(0)))
            }
            _ => Err(condition.error("text fields support only =, != and contains")),
        }
    }};
}

macro_rules! error_log {
    ($column:expr, $condition:expr) => {{
        let condition = $condition;
        if matches!(condition.value, Value::Null) {
            match condition.operator {
                Operator::Equal => {
                    Ok::<Predicate<_>, anyhow::Error>(Box::new($column.is_null().nullable()))
                }
                Operator::NotEqual => Ok(Box::new($column.is_not_null().nullable())),
                _ => Err(condition.error("null supports only = and !=")),
            }
        } else {
            text!($column, condition)
        }
    }};
}

pub struct LogFilter {
    expression: Option<Expression>,
    timezone: Option<Tz>,
}

impl LogFilter {
    pub fn parse(input: Option<&str>, timezone: Option<&str>) -> Result<Self> {
        let expression = parser::parse(input.unwrap_or_default())?;
        let timezone = timezone
            .map(|value| {
                value
                    .parse::<Tz>()
                    .map_err(|_| anyhow!("unknown timezone: {value}"))
            })
            .transpose()?;
        Ok(Self {
            expression,
            timezone,
        })
    }

    pub fn http(&self) -> Result<Option<Predicate<http_log::table>>> {
        self.expression
            .as_ref()
            .map(|expression| {
                compile(expression, &|condition| match condition.field.as_str() {
                    "id" => compare!(http_log::id, condition, condition.integer()?),
                    "client_port" => {
                        compare!(http_log::client_port, condition, condition.integer()?)
                    }
                    "client_ip" => text!(http_log::client_ip, condition),
                    "location" => text!(http_log::location, condition),
                    "method" => text!(http_log::method, condition),
                    "path" => text!(http_log::path, condition),
                    "raw_query" => text!(http_log::raw_query, condition),
                    "raw_body" => {
                        let value = condition.string()?.as_bytes().to_vec();
                        match condition.operator {
                            Operator::Equal | Operator::NotEqual => {
                                compare!(http_log::raw_body, condition, value)
                            }
                            Operator::Contains => Ok(Box::new(
                                byte_position(http_log::raw_body, value).gt(0).nullable(),
                            )),
                            _ => Err(condition.error("raw_body supports only =, != and contains")),
                        }
                    }
                    "error_log" => error_log!(http_log::error_log, condition),
                    "create_time" => {
                        let value = parse_time(condition.string()?, self.timezone)
                            .map_err(|error| condition.error(error))?;
                        compare!(http_log::create_time, condition, value)
                    }
                    "parsed_body_type" => {
                        if !matches!(condition.operator, Operator::Equal | Operator::NotEqual) {
                            return Err(condition.error("parsed_body_type supports only = and !="));
                        }
                        let value = match condition.string()? {
                            "NONE" => BodyKind::NONE,
                            "FAILED" => BodyKind::FAILED,
                            "FORM" => BodyKind::FORM,
                            "JSON" => BodyKind::JSON,
                            _ => {
                                return Err(condition
                                    .error("parsed_body_type must be NONE, FAILED, FORM or JSON"));
                            }
                        };
                        compare!(http_log::parsed_body_type, condition, value)
                    }
                    _ => {
                        Err(condition.error(format!("unknown HTTP log field: {}", condition.field)))
                    }
                })
            })
            .transpose()
    }

    pub fn dns(&self) -> Result<Option<Predicate<dns_log::table>>> {
        self.expression
            .as_ref()
            .map(|expression| {
                compile(expression, &|condition| match condition.field.as_str() {
                    "id" => compare!(dns_log::id, condition, condition.integer()?),
                    "client_port" => {
                        compare!(dns_log::client_port, condition, condition.integer()?)
                    }
                    "client_ip" => text!(dns_log::client_ip, condition),
                    "location" => text!(dns_log::location, condition),
                    "query_name" => text!(dns_log::query_name, condition),
                    "query_type" => text!(dns_log::query_type, condition),
                    "query_class" => text!(dns_log::query_class, condition),
                    "error_log" => error_log!(dns_log::error_log, condition),
                    "create_time" => {
                        let value = parse_time(condition.string()?, self.timezone)
                            .map_err(|error| condition.error(error))?;
                        compare!(dns_log::create_time, condition, value)
                    }
                    _ => {
                        Err(condition.error(format!("unknown DNS log field: {}", condition.field)))
                    }
                })
            })
            .transpose()
    }
}

fn compile<Table: 'static>(
    expression: &Expression,
    condition: &impl Fn(&Condition) -> Result<Predicate<Table>>,
) -> Result<Predicate<Table>> {
    Ok(match expression {
        Expression::Condition(value) => return condition(value),
        Expression::And(left, right) => {
            Box::new(compile(left, condition)?.and(compile(right, condition)?))
        }
        Expression::Or(left, right) => {
            Box::new(compile(left, condition)?.or(compile(right, condition)?))
        }
        Expression::Not(value) => Box::new(not(compile(value, condition)?)),
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use chrono::{TimeZone, Utc};
    use diesel::prelude::*;
    use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};

    use super::{
        LogFilter,
        parser::{self, Expression, Operator, Value},
        time::parse_time,
    };
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

    #[test]
    fn logical_precedence_and_json_string_escaping() {
        let expression = parser::parse(r#"id = 1 || id = 2 && !(path = "quote\"\\\u4e2d")"#)
            .unwrap()
            .unwrap();
        let Expression::Or(_, right) = expression else {
            panic!("expected OR at the root")
        };
        let Expression::And(_, right) = *right else {
            panic!("AND must bind before OR")
        };
        let Expression::Not(inner) = *right else {
            panic!("expected negated group")
        };
        let Expression::Condition(condition) = *inner else {
            panic!("expected comparison")
        };
        assert_eq!(condition.operator, Operator::Equal);
        assert!(matches!(condition.value, Value::String(value) if value == "quote\"\\中"));
    }

    #[test]
    fn parses_single_and_double_quoted_strings_with_escapes() {
        for (input, expected) in [
            (
                r#"path = 'it\'s "quoted"\\\n\u4e2d'"#,
                "it's \"quoted\"\\\n中",
            ),
            (
                r#"path = "it\'s \"quoted\"\\\n\u4e2d""#,
                "it's \"quoted\"\\\n中",
            ),
            (r#"contains(raw_body, '\uD83D\uDE00\u0000')"#, "\u{1f600}\0"),
            (r#"path = '\\u4e2d\\'"#, "\\u4e2d\\"),
        ] {
            let Expression::Condition(condition) = parser::parse(input).unwrap().unwrap() else {
                panic!("expected comparison: {input}")
            };
            assert_eq!(condition.string().unwrap(), expected, "{input}");
        }
    }

    #[test]
    fn rejects_incomplete_expressions_and_reports_character_positions() {
        for input in [
            "id = 1 &&",
            "(id = 1",
            "id == 1",
            "id = 1 trailing",
            "contains(path)",
            r#"path = "unterminated"#,
            r#"path = 'mismatched""#,
            r#"path = 'trailing\'"#,
            r#"path = '\u123'"#,
            r#"path = '\q'"#,
            "id = 2147483648",
            "id = 1.5",
        ] {
            assert!(parser::parse(input).is_err(), "accepted: {input}");
        }
        let error = parser::parse("path = \"中文\" && ?").unwrap_err();
        assert!(error.to_string().contains("character 16"), "{error}");
        assert!(parser::parse(&format!("{}id = 1{}", "(".repeat(65), ")".repeat(65))).is_err());
    }

    #[test]
    fn validates_fields_types_and_operators_before_querying() {
        for (input, expected) in [
            (r#"query_name = "example.com""#, "unknown HTTP log field"),
            (r#"client_port = "80""#, "requires an integer"),
            (r#"contains(id, "1")"#, "requires an integer"),
            (r#"path < "abc""#, "text fields support only"),
            (
                r#"parsed_body_type = "XML""#,
                "must be NONE, FAILED, FORM or JSON",
            ),
            ("error_log < null", "null supports only"),
            ("client_ip = null", "requires a quoted string"),
            (r#"raw_body > "abc""#, "raw_body supports only"),
            ("raw_body = null", "requires a quoted string"),
            ("contains(raw_body, 1)", "requires a quoted string"),
        ] {
            let filter = LogFilter::parse(Some(input), None).unwrap();
            let error = filter.http().err().expect("invalid HTTP filter accepted");
            assert!(error.to_string().contains(expected), "{input}: {error}");
        }
        for input in [r#"method = "GET""#, r#"contains(raw_body, "asd")"#] {
            let filter = LogFilter::parse(Some(input), None).unwrap();
            assert!(
                filter
                    .dns()
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("unknown DNS log field")
            );
        }
    }

    #[test]
    fn rejects_null_characters_in_text_fields_during_type_validation() {
        for input in [r#"path = "\u0000""#, r#"contains(error_log, "\u0000")"#] {
            let filter = LogFilter::parse(Some(input), None).unwrap();
            let error = filter.http().err().expect("NUL accepted in a text filter");
            assert!(
                error
                    .to_string()
                    .contains("text filters cannot contain a null character")
            );
        }
        let filter = LogFilter::parse(Some(r#"contains(query_name, "\u0000")"#), None).unwrap();
        let error = filter
            .dns()
            .err()
            .expect("NUL accepted in a DNS text filter");
        assert!(
            error
                .to_string()
                .contains("text filters cannot contain a null character")
        );
    }

    #[test]
    fn resolves_local_dates_and_times_using_the_requested_timezone() {
        let timezone = Some("Asia/Shanghai".parse().unwrap());
        for (input, expected) in [
            ("2026-09-19", "2026-09-18T16:00:00+00:00"),
            ("2026-09-19 12:00:00.125", "2026-09-19T04:00:00.125+00:00"),
            ("2026-09-19T12:00:00", "2026-09-19T04:00:00+00:00"),
            ("2026-09-19T12:00:00-07:00", "2026-09-19T19:00:00+00:00"),
        ] {
            assert_eq!(parse_time(input, timezone).unwrap().to_rfc3339(), expected);
        }
        assert!(
            parse_time("2026-09-19", None)
                .unwrap_err()
                .to_string()
                .contains("requires the timezone")
        );
        assert!(parse_time("2026-02-30", timezone).is_err());
    }

    #[test]
    fn handles_daylight_saving_dates_without_using_todays_offset() {
        let timezone = Some("America/Los_Angeles".parse().unwrap());
        assert_eq!(
            parse_time("2026-01-15 12:00:00", timezone)
                .unwrap()
                .to_rfc3339(),
            "2026-01-15T20:00:00+00:00"
        );
        assert_eq!(
            parse_time("2026-07-15 12:00:00", timezone)
                .unwrap()
                .to_rfc3339(),
            "2026-07-15T19:00:00+00:00"
        );
        assert!(
            parse_time("2026-03-08 02:30:00", timezone)
                .unwrap_err()
                .to_string()
                .contains("does not exist")
        );
        assert!(
            parse_time("2026-11-01 01:30:00", timezone)
                .unwrap_err()
                .to_string()
                .contains("ambiguous")
        );
        assert_eq!(
            parse_time("2026-11-01T01:30:00-07:00", timezone)
                .unwrap()
                .to_rfc3339(),
            "2026-11-01T08:30:00+00:00"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires TEST_DATABASE_URL pointing to a disposable PostgreSQL database"]
    async fn filters_http_and_dns_before_counting_and_paginating() -> Result<()> {
        let pool = crate::db::establish_db_connection(&std::env::var("TEST_DATABASE_URL")?).await?;
        crate::db::run_migrations(&pool).await?;
        let mut conn = pool.get().await?;
        conn.begin_test_transaction().await?;

        let raw_bodies = [
            b"asd".to_vec(),
            b"prefix asd suffix".to_vec(),
            b"\xff\0asd\xfe".to_vec(),
            "中文%_\\\"' OR 1=1 --".as_bytes().to_vec(),
            Vec::new(),
        ];
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
                    raw_body: raw_bodies[index].clone(),
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
            (r#"raw_body = "asd""#, vec![http_ids[0]]),
            (
                r#"raw_body != """#,
                vec![http_ids[3], http_ids[2], http_ids[1], http_ids[0]],
            ),
            (
                r#"contains(raw_body, "asd")"#,
                vec![http_ids[2], http_ids[1], http_ids[0]],
            ),
            (r#"contains(raw_body, "\u0000")"#, vec![http_ids[2]]),
            (
                r#"contains(raw_body, "中文%_\\\"' OR 1=1 --")"#,
                vec![http_ids[3]],
            ),
        ] {
            assert_http_ids(&mut conn, expression, &expected).await?;
        }
        assert_http_ids(
            &mut conn,
            "   ",
            &http_ids.iter().rev().copied().collect::<Vec<_>>(),
        )
        .await?;

        let filter = LogFilter::parse(
            Some(r#"contains(raw_body, "asd") && method = "POST""#),
            None,
        )?;
        let (logs, total) = get_http_logs_paginated(&mut conn, 2, 1, &filter).await?;
        assert_eq!(total, 2);
        assert_eq!(
            logs.iter().map(|log| log.id).collect::<Vec<_>>(),
            vec![http_ids[1]]
        );

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
}
