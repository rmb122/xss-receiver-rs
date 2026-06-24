use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct StartupConfig {
    pub db_url: String,
    pub storage_path: String,

    pub ip2region: Ip2Region,
    pub http_server: HttpServer,
    pub dns_server: DnsServer,
    #[serde(default)]
    pub script_cache: ScriptCache,
}

#[derive(Clone, Deserialize)]
pub struct Ip2Region {
    pub ipv4_db: String,
    pub ipv6_db: String,
}

#[derive(Clone, Deserialize)]
pub struct HttpServer {
    pub listen: String,
    pub openapi: bool,
    pub jwt_secret: String,
    pub jwt_expire_time: i64,
    pub real_addr_header: String,
    pub admin_prefix: String,
    pub max_body_size: i64,
}

#[derive(Clone, Deserialize)]
pub struct DnsServer {
    pub listen: String,
}

#[derive(Clone, Deserialize)]
pub struct ScriptCache {
    #[serde(default = "default_script_cache_max_entries")]
    pub max_entries: u64,
    #[serde(default = "default_script_cache_max_entry_size")]
    pub max_entry_size: u64,
    #[serde(default = "default_script_cache_max_ttl")]
    pub max_ttl: u64,
}

impl Default for ScriptCache {
    fn default() -> Self {
        Self {
            max_entries: default_script_cache_max_entries(),
            max_entry_size: default_script_cache_max_entry_size(),
            max_ttl: default_script_cache_max_ttl(),
        }
    }
}

fn default_script_cache_max_entries() -> u64 {
    1024
}

fn default_script_cache_max_entry_size() -> u64 {
    65535
}

fn default_script_cache_max_ttl() -> u64 {
    3600
}

pub fn parse(cfg: &str) -> anyhow::Result<StartupConfig> {
    let cfg: StartupConfig = toml::from_str(cfg)?;
    if cfg.storage_path.is_empty() {
        return Err(anyhow::anyhow!(
            "upload_storage_path in config is empty, you must specify a path"
        ));
    }
    if cfg.script_cache.max_entries == 0 {
        return Err(anyhow::anyhow!(
            "script_cache.max_entries must be greater than 0"
        ));
    }
    if cfg.script_cache.max_entry_size == 0 {
        return Err(anyhow::anyhow!(
            "script_cache.max_entry_size must be greater than 0"
        ));
    }
    if cfg.script_cache.max_ttl == 0 {
        return Err(anyhow::anyhow!(
            "script_cache.max_ttl must be greater than 0"
        ));
    }
    return Ok(cfg);
}
