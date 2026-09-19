use super::{
    LogFilter,
    parser::{self, Expression, Operator, Value},
    time::parse_time,
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
fn rejects_incomplete_expressions_and_reports_character_positions() {
    for input in [
        "id = 1 &&",
        "(id = 1",
        "id == 1",
        "id = 1 trailing",
        "contains(path)",
        r#"path = "unterminated"#,
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
        ("client_ip = null", "requires a double-quoted string"),
    ] {
        let filter = LogFilter::parse(Some(input), None).unwrap();
        let error = filter.http().err().expect("invalid HTTP filter accepted");
        assert!(error.to_string().contains(expected), "{input}: {error}");
    }
    let filter = LogFilter::parse(Some(r#"method = "GET""#), None).unwrap();
    assert!(
        filter
            .dns()
            .err()
            .unwrap()
            .to_string()
            .contains("unknown DNS log field")
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
