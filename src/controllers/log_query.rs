use anyhow::{Result, ensure};
use serde::Deserialize;

use crate::db::log_filter::LogFilter;

#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct LogQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    /// Filter with =, !=, <, <=, >, >=, &&, ||, !, parentheses and contains(field, "text").
    /// Common fields: id, client_ip, client_port, location, create_time, error_log.
    /// HTTP also supports method, path, raw_query, parsed_body_type; DNS supports query_name, query_type, query_class.
    /// Strings use JSON double quotes. Text supports =, != and case-sensitive contains; integers and times support all comparisons.
    /// Use error_log = null or error_log != null to check for missing errors. Empty filters match all logs.
    pub filter: Option<String>,
    /// IANA timezone, e.g. Asia/Shanghai. Required for filter times without an explicit RFC3339 offset.
    /// Local times accept YYYY-MM-DD (midnight) or YYYY-MM-DD HH:mm:ss with optional fractional seconds; T may replace the space.
    /// Ambiguous or nonexistent local times must use an explicit offset.
    pub timezone: Option<String>,
}

impl LogQuery {
    pub fn filter(&self) -> Result<LogFilter> {
        ensure!(self.page >= 1, "page must be greater than 0");
        ensure!(
            (1..=500).contains(&self.page_size),
            "page_size must be between 1 and 500"
        );
        ensure!(
            (self.page - 1).checked_mul(self.page_size).is_some(),
            "page is too large"
        );
        LogFilter::parse(self.filter.as_deref(), self.timezone.as_deref())
    }
}

fn default_page() -> i64 {
    1
}

fn default_page_size() -> i64 {
    20
}
