use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct StartupConfig {
    pub db_url: String,
    pub storage_path: String,

    pub ip2region: Ip2Region,
    pub http_server: HttpServer,
    pub dns_server: DnsServer,
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

pub fn parse(cfg: &str) -> anyhow::Result<StartupConfig> {
    let cfg: StartupConfig = toml::from_str(cfg)?;
    if cfg.storage_path.is_empty() {
        return Err(anyhow::anyhow!(
            "upload_storage_path in config is empty, you must specify a path"
        ));
    }
    return Ok(cfg);
}
