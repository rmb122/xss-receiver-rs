use std::{
    fmt::{Display, Formatter},
    time::{Duration, Instant},
};

use boa_engine::object::builtins::JsUint8Array;
use boa_engine::{
    Context, JsNativeError, JsResult, JsValue, NativeFunction, js_string,
    object::ObjectInitializer, property::Attribute,
};
use boa_gc::{Finalize, Gc, Trace, empty_trace};
use moka::{
    ops::compute::{CompResult, Op},
    sync::Cache,
};

use crate::startup_config;

use super::helpers::{check_argument_count, ensure_exists, read_data_from_uint8_array};

#[derive(Debug, Clone)]
pub enum CacheErrorKind {
    Type,
    Range,
}

#[derive(Debug, Clone)]
pub struct CacheError {
    kind: CacheErrorKind,
    message: String,
}

impl CacheError {
    fn typ<T: Into<String>>(message: T) -> Self {
        Self {
            kind: CacheErrorKind::Type,
            message: message.into(),
        }
    }

    fn range<T: Into<String>>(message: T) -> Self {
        Self {
            kind: CacheErrorKind::Range,
            message: message.into(),
        }
    }

    fn into_js_error(self) -> boa_engine::JsError {
        match self.kind {
            CacheErrorKind::Type => JsNativeError::typ().with_message(self.message).into(),
            CacheErrorKind::Range => JsNativeError::range().with_message(self.message).into(),
        }
    }
}

impl Display for CacheError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CacheError {}

#[derive(Clone, Debug)]
pub enum CacheValue {
    String(String),
    Bool(bool),
    Number(f64),
    Bytes(Vec<u8>),
}

impl CacheValue {
    fn value_size(&self) -> u64 {
        match self {
            CacheValue::String(value) => value.len() as u64,
            CacheValue::Bool(_) => 1,
            CacheValue::Number(_) => 8,
            CacheValue::Bytes(value) => value.len() as u64,
        }
    }
}

#[derive(Clone, Debug)]
struct CacheEntry {
    value: CacheValue,
    expires_at: Instant,
}

impl CacheEntry {
    fn is_expired(&self, now: Instant) -> bool {
        self.expires_at <= now
    }
}

#[derive(Clone)]
pub struct ScriptCache {
    inner: Cache<String, CacheEntry>,
    max_entry_size: u64,
    max_ttl: Duration,
}

impl ScriptCache {
    pub fn new(config: &startup_config::ScriptCache) -> Self {
        let max_ttl = Duration::from_secs(config.max_ttl);
        let inner = Cache::builder()
            .max_capacity(config.max_entries)
            .time_to_live(max_ttl)
            .build();

        Self {
            inner,
            max_entry_size: config.max_entry_size,
            max_ttl,
        }
    }

    pub fn set(
        &self,
        key: String,
        value: CacheValue,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        let ttl = self.validate_ttl(ttl)?;
        self.validate_entry_size(&key, &value)?;
        self.inner.insert(
            key,
            CacheEntry {
                value,
                expires_at: Instant::now() + ttl,
            },
        );
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<CacheValue> {
        let entry = self.inner.get(key)?;
        if entry.is_expired(Instant::now()) {
            self.inner.invalidate(key);
            return None;
        }
        Some(entry.value)
    }

    pub fn delete(&self, key: &str) -> bool {
        let exists = self.get(key).is_some();
        self.inner.invalidate(key);
        exists
    }

    pub fn incr(&self, key: String, delta: f64, ttl: Option<Duration>) -> Result<f64, CacheError> {
        if !delta.is_finite() {
            return Err(CacheError::typ("delta must be a finite number"));
        }
        let ttl = ttl.map(|ttl| self.validate_ttl(Some(ttl))).transpose()?;
        self.validate_entry_size(&key, &CacheValue::Number(0.0))?;

        let max_ttl = self.max_ttl;
        let result = self.inner.entry(key).and_try_compute_with(|maybe_entry| {
            let now = Instant::now();
            let expires_at = ttl.map_or(now + max_ttl, |ttl| now + ttl);

            let value = match maybe_entry {
                Some(entry) => {
                    let old_entry = entry.into_value();
                    if old_entry.is_expired(now) {
                        delta
                    } else if let CacheValue::Number(value) = old_entry.value {
                        value + delta
                    } else {
                        return Err(CacheError::typ("cached value is not a number"));
                    }
                }
                None => delta,
            };

            if !value.is_finite() {
                return Err(CacheError::range(
                    "increment result must be a finite number",
                ));
            }

            Ok(Op::Put(CacheEntry {
                value: CacheValue::Number(value),
                expires_at,
            }))
        })?;

        Ok(match result {
            CompResult::Inserted(entry)
            | CompResult::ReplacedWith(entry)
            | CompResult::Unchanged(entry) => match entry.into_value().value {
                CacheValue::Number(value) => value,
                _ => unreachable!("incr only stores number values"),
            },
            CompResult::Removed(_) | CompResult::StillNone(_) => {
                unreachable!("incr never removes entries")
            }
        })
    }

    fn validate_ttl(&self, ttl: Option<Duration>) -> Result<Duration, CacheError> {
        let ttl = ttl.unwrap_or(self.max_ttl);
        if ttl.is_zero() {
            return Err(CacheError::range("ttl must be greater than 0"));
        }
        if ttl > self.max_ttl {
            return Err(CacheError::range("ttl must not be greater than max_ttl"));
        }
        Ok(ttl)
    }

    fn validate_entry_size(&self, key: &str, value: &CacheValue) -> Result<(), CacheError> {
        let size = key.len() as u64 + value.value_size();
        if size > self.max_entry_size {
            return Err(CacheError::range(format!(
                "cache entry size {} exceeds max_entry_size {}",
                size, self.max_entry_size
            )));
        }
        Ok(())
    }
}

pub struct ScriptCacheCell {
    cache: ScriptCache,
}

impl Finalize for ScriptCacheCell {}

// SAFETY: ScriptCache stores only Rust-owned cache data and a thread-safe Moka cache; it does
// not contain any Boa GC-managed JavaScript values.
unsafe impl Trace for ScriptCacheCell {
    empty_trace!();
}

fn get_cache_from_context(ctx: &mut Context) -> JsResult<Gc<ScriptCacheCell>> {
    ensure_exists(
        ctx.get_data::<Gc<ScriptCacheCell>>().cloned(),
        "failed to get cache from context",
    )
}

fn parse_key(value: &JsValue) -> JsResult<String> {
    Ok(ensure_exists(value.as_string(), "key must be a string")?.to_std_string_lossy())
}

fn parse_ttl(value: Option<&JsValue>) -> JsResult<Option<Duration>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_undefined() {
        return Ok(None);
    }

    let ttl = ensure_exists(value.as_number(), "ttl must be a number")?;
    if !ttl.is_finite() || ttl <= 0.0 {
        return Err(JsNativeError::range()
            .with_message("ttl must be a finite number greater than 0")
            .into());
    }
    Duration::try_from_secs_f64(ttl).map(Some).map_err(|_| {
        JsNativeError::range()
            .with_message("ttl is out of range")
            .into()
    })
}

fn parse_delta(value: Option<&JsValue>) -> JsResult<f64> {
    let Some(value) = value else {
        return Ok(1.0);
    };
    if value.is_undefined() {
        return Ok(1.0);
    }

    let delta = ensure_exists(value.as_number(), "delta must be a number")?;
    if !delta.is_finite() {
        return Err(JsNativeError::typ()
            .with_message("delta must be a finite number")
            .into());
    }
    Ok(delta)
}

fn js_value_to_cache_value(value: &JsValue, ctx: &mut Context) -> JsResult<CacheValue> {
    if let Some(value) = value.as_string() {
        return Ok(CacheValue::String(value.to_std_string_lossy()));
    }
    if let Some(value) = value.as_boolean() {
        return Ok(CacheValue::Bool(value));
    }
    if let Some(value) = value.as_number() {
        if !value.is_finite() {
            return Err(JsNativeError::typ()
                .with_message("number value must be finite")
                .into());
        }
        return Ok(CacheValue::Number(value));
    }
    if value.as_object().is_some() {
        return Ok(CacheValue::Bytes(read_data_from_uint8_array(value, ctx)?));
    }

    Err(JsNativeError::typ()
        .with_message("value must be string, boolean, number or Uint8Array")
        .into())
}

fn cache_value_to_js_value(value: CacheValue, ctx: &mut Context) -> JsResult<JsValue> {
    match value {
        CacheValue::String(value) => Ok(JsValue::from(js_string!(value))),
        CacheValue::Bool(value) => Ok(JsValue::from(value)),
        CacheValue::Number(value) => Ok(JsValue::from(value)),
        CacheValue::Bytes(value) => Ok(JsUint8Array::from_iter(value, ctx)?.into()),
    }
}

pub fn register_cache_to_context(context: &mut Context, cache: ScriptCache) {
    let data = Gc::new(ScriptCacheCell { cache });
    context.insert_data(data);

    let mut object_builder = ObjectInitializer::new(context);

    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 2)?;
            let key = parse_key(&args[0])?;
            let value = js_value_to_cache_value(&args[1], ctx)?;
            let ttl = parse_ttl(args.get(2))?;

            get_cache_from_context(ctx)?
                .cache
                .set(key, value, ttl)
                .map_err(CacheError::into_js_error)?;
            Ok(JsValue::undefined())
        }),
        js_string!("set"),
        3,
    );

    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let key = parse_key(&args[0])?;

            match get_cache_from_context(ctx)?.cache.get(&key) {
                Some(value) => cache_value_to_js_value(value, ctx),
                None => Ok(JsValue::undefined()),
            }
        }),
        js_string!("get"),
        1,
    );

    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let key = parse_key(&args[0])?;
            let deleted = get_cache_from_context(ctx)?.cache.delete(&key);
            Ok(JsValue::from(deleted))
        }),
        js_string!("delete"),
        1,
    );

    object_builder.function(
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            check_argument_count(args, 1)?;
            let key = parse_key(&args[0])?;
            let delta = parse_delta(args.get(1))?;
            let ttl = parse_ttl(args.get(2))?;
            let value = get_cache_from_context(ctx)?
                .cache
                .incr(key, delta, ttl)
                .map_err(CacheError::into_js_error)?;
            Ok(JsValue::from(value))
        }),
        js_string!("incr"),
        3,
    );

    let object = object_builder.build();
    context
        .register_global_property(
            js_string!("cache"),
            object,
            Attribute::READONLY | Attribute::ENUMERABLE,
        )
        .expect("cache property shouldn't exist");
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread, time::Duration};

    use boa_engine::{Context, Source};

    use super::{CacheValue, ScriptCache, register_cache_to_context};

    fn test_cache(max_entries: u64, max_entry_size: u64, max_ttl: u64) -> ScriptCache {
        ScriptCache::new(&crate::startup_config::ScriptCache {
            max_entries,
            max_entry_size,
            max_ttl,
        })
    }

    #[test]
    fn set_get_delete_value() {
        let cache = test_cache(10, 100, 60);
        cache
            .set(
                "x".to_string(),
                CacheValue::String("value".to_string()),
                None,
            )
            .unwrap();
        assert!(matches!(cache.get("x"), Some(CacheValue::String(value)) if value == "value"));
        assert!(cache.delete("x"));
        assert!(cache.get("x").is_none());
        assert!(!cache.delete("x"));
    }

    #[test]
    fn rejects_entry_larger_than_limit() {
        let cache = test_cache(10, 4, 60);
        assert!(
            cache
                .set(
                    "key".to_string(),
                    CacheValue::String("too long".to_string()),
                    None
                )
                .is_err()
        );
    }

    #[test]
    fn expires_by_entry_ttl() {
        let cache = test_cache(10, 100, 60);
        cache
            .set(
                "x".to_string(),
                CacheValue::Bool(true),
                Some(Duration::from_millis(20)),
            )
            .unwrap();
        thread::sleep(Duration::from_millis(40));
        assert!(cache.get("x").is_none());
    }

    #[test]
    fn rejects_ttl_greater_than_max_ttl() {
        let cache = test_cache(10, 100, 1);
        assert!(
            cache
                .set(
                    "x".to_string(),
                    CacheValue::Bool(true),
                    Some(Duration::from_secs(2)),
                )
                .is_err()
        );
    }

    #[test]
    fn incr_is_atomic_for_same_key() {
        let cache = Arc::new(test_cache(10, 100, 60));
        let mut handles = Vec::new();

        for _ in 0..16 {
            let cache = cache.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    cache.incr("x".to_string(), 1.0, None).unwrap();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert!(matches!(cache.get("x"), Some(CacheValue::Number(value)) if value == 1600.0));
    }

    #[test]
    fn js_contexts_share_cache() {
        let cache = test_cache(10, 100, 60);
        let mut http_context = Context::default();
        let mut dns_context = Context::default();
        register_cache_to_context(&mut http_context, cache.clone());
        register_cache_to_context(&mut dns_context, cache);

        let result = http_context
            .eval(Source::from_bytes(
                r#"cache.set("x", 1, 10); cache.get("x");"#,
            ))
            .unwrap();
        assert_eq!(result.as_number(), Some(1.0));

        let result = dns_context
            .eval(Source::from_bytes(r#"cache.incr("x", 2, 10);"#))
            .unwrap();
        assert_eq!(result.as_number(), Some(3.0));

        http_context
            .eval(Source::from_bytes(
                r#"cache.set("bytes", new Uint8Array([1, 2, 3]), 10);"#,
            ))
            .unwrap();
        let result = dns_context
            .eval(Source::from_bytes(
                r#"const bytes = cache.get("bytes"); bytes[0] + bytes.length;"#,
            ))
            .unwrap();
        assert_eq!(result.as_number(), Some(4.0));
    }
}
