mod parser;
mod time;

#[cfg(test)]
mod database_tests;
#[cfg(test)]
mod tests;

use anyhow::{Result, anyhow};
use chrono_tz::Tz;
use diesel::{
    dsl::not,
    pg::Pg,
    prelude::*,
    sql_types::{Bool, Integer, Nullable, Text},
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
            Operator::Contains => return Err(condition.error("contains requires a text field")),
        })
    }};
}

macro_rules! text {
    ($column:expr, $condition:expr) => {{
        let condition = $condition;
        let value = condition.string()?.to_owned();
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
