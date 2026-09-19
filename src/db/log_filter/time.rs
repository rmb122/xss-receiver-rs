use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

pub(super) fn parse_time(value: &str, timezone: Option<Tz>) -> Result<DateTime<Utc>> {
    if let Ok(time) = DateTime::parse_from_rfc3339(value) {
        return Ok(time.with_timezone(&Utc));
    }
    let local_time = ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"]
        .into_iter()
        .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()?
                .and_hms_opt(0, 0, 0)
        })
        .ok_or_else(|| anyhow!("invalid time; use RFC3339, YYYY-MM-DD or YYYY-MM-DD HH:mm:ss"))?;
    let timezone = timezone.ok_or_else(|| {
        anyhow!("time without an offset requires the timezone parameter, e.g. Asia/Shanghai")
    })?;
    match timezone.from_local_datetime(&local_time) {
        LocalResult::Single(time) => Ok(time.with_timezone(&Utc)),
        LocalResult::Ambiguous(_, _) => {
            bail!("time is ambiguous in {timezone}; specify an explicit RFC3339 offset")
        }
        LocalResult::None => {
            bail!("time does not exist in {timezone}; specify an explicit RFC3339 offset")
        }
    }
}
