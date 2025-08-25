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
    pub upload_storage_path: String,
    pub real_addr_header: String,
    pub admin_prefix: String,
}

#[derive(Clone, Deserialize)]
pub struct DnsServer {
    pub listen: String,
}

pub fn parse(cfg: &str) -> anyhow::Result<StartupConfig> {
    return Ok(toml::from_str(cfg)?);
}
