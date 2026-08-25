use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct StartupConfig {
    pub db_url: String,
    pub storage_path: String,

    pub ip2region: Ip2Region,
    pub http_server: HttpServer,
    pub dns_server: DnsServer,
    #[serde(default)]
    pub script: Script,
}

#[derive(Clone, Deserialize, Default)]
pub struct Script {
    #[serde(default)]
    pub cache: ScriptCache,
    #[serde(default)]
    pub http: ScriptHttp,
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

#[derive(Clone, Deserialize)]
pub struct ScriptHttp {
    #[serde(default)]
    pub allow_private_network: bool,
    #[serde(default = "default_script_http_timeout")]
    pub timeout: u64,
    #[serde(default = "default_script_http_max_response_size")]
    pub max_response_size: u64,
    #[serde(default = "default_script_http_max_redirects")]
    pub max_redirects: usize,
}

impl Default for ScriptHttp {
    fn default() -> Self {
        Self {
            allow_private_network: false,
            timeout: default_script_http_timeout(),
            max_response_size: default_script_http_max_response_size(),
            max_redirects: default_script_http_max_redirects(),
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

fn default_script_http_timeout() -> u64 {
    16_000
}

fn default_script_http_max_response_size() -> u64 {
    8 * 1024 * 1024
}

fn default_script_http_max_redirects() -> usize {
    8
}

pub fn parse(cfg: &str) -> anyhow::Result<StartupConfig> {
    let cfg: StartupConfig = toml::from_str(cfg)?;
    if cfg.storage_path.is_empty() {
        return Err(anyhow::anyhow!(
            "upload_storage_path in config is empty, you must specify a path"
        ));
    }
    if cfg.script.cache.max_entries == 0 {
        return Err(anyhow::anyhow!(
            "script.cache.max_entries must be greater than 0"
        ));
    }
    if cfg.script.cache.max_entry_size == 0 {
        return Err(anyhow::anyhow!(
            "script.cache.max_entry_size must be greater than 0"
        ));
    }
    if cfg.script.cache.max_ttl == 0 {
        return Err(anyhow::anyhow!(
            "script.cache.max_ttl must be greater than 0"
        ));
    }
    if cfg.script.http.timeout == 0 {
        return Err(anyhow::anyhow!(
            "script.http.timeout must be greater than 0"
        ));
    }
    if cfg.script.http.max_response_size == 0 {
        return Err(anyhow::anyhow!(
            "script.http.max_response_size must be greater than 0"
        ));
    }
    return Ok(cfg);
}

#[cfg(test)]
mod tests {
    use super::parse;

    const CONFIG_PREFIX: &str = r#"
db_url = "postgres://localhost/test"
storage_path = "/tmp"

[http_server]
listen = "127.0.0.1:8000"
openapi = false
jwt_secret = "test"
jwt_expire_time = 60
real_addr_header = ""
admin_prefix = "/admin"
max_body_size = 1024

[dns_server]
listen = ""

[ip2region]
ipv4_db = ""
ipv6_db = ""
"#;

    #[test]
    fn script_configuration_uses_nested_tables() {
        let config = parse(&format!(
            r#"{CONFIG_PREFIX}
[script.cache]
max_entries = 10
max_entry_size = 20
max_ttl = 30

[script.http]
allow_private_network = true
timeout = 1234
max_response_size = 5678
max_redirects = 2
"#
        ))
        .unwrap();

        assert_eq!(config.script.cache.max_entries, 10);
        assert_eq!(config.script.cache.max_entry_size, 20);
        assert_eq!(config.script.cache.max_ttl, 30);
        assert!(config.script.http.allow_private_network);
        assert_eq!(config.script.http.timeout, 1234);
        assert_eq!(config.script.http.max_response_size, 5678);
        assert_eq!(config.script.http.max_redirects, 2);
    }

    #[test]
    fn script_configuration_has_defaults() {
        let config = parse(CONFIG_PREFIX).unwrap();
        assert_eq!(config.script.cache.max_entries, 1024);
        assert_eq!(config.script.http.timeout, 16_000);
        assert_eq!(config.script.http.max_response_size, 8 * 1024 * 1024);
        assert_eq!(config.script.http.max_redirects, 8);
        assert!(!config.script.http.allow_private_network);
    }

    #[test]
    fn rejects_zero_http_limits() {
        let error = parse(&format!(
            r#"{CONFIG_PREFIX}
[script.http]
timeout = 0
"#
        ))
        .err()
        .unwrap();
        assert!(error.to_string().contains("script.http.timeout"));
    }

    #[test]
    fn bundled_configuration_examples_are_valid() {
        parse(include_str!("../config_example.toml")).unwrap();
        parse(include_str!("../docker/config.toml")).unwrap();
    }
}
