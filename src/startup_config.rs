use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct StartupConfig {
    pub db_url: String,
    pub http_server: HttpServer,
    pub dns_server: DnsServer,
}

#[derive(Clone, Deserialize)]
pub struct HttpServer {
    pub listen: String,
    pub jwt_secret: String,
    pub jwt_expire_time: i64,
    pub upload_storage_path: String,
    pub real_addr_header: String,
    pub admin_prefix: String,
}

#[derive(Clone, Deserialize)]
pub struct DnsServer {
    pub listen: String,
}

pub fn parse(cfg: &str) -> anyhow::Result<StartupConfig> {
    let cfg: StartupConfig = toml::from_str(cfg)?;
    if cfg.http_server.upload_storage_path.is_empty() {
        return Err(anyhow::anyhow!(
            "upload_storage_path in config is empty, you must specify a path"
        ));
    }
    return Ok(cfg);
}
