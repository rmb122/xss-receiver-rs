mod error;
mod header;
mod ip_value;
mod searcher;

pub use searcher::{CachePolicy, Searcher};

pub struct Locator {
    ipv4_searcher: Option<Searcher>,
    ipv6_searcher: Option<Searcher>,
}

const DATABASE_NOT_CONFIG: &str = "数据库未配置";

impl Locator {
    pub fn new(
        ipv4_db: Option<String>,
        ipv6_db: Option<String>,
        policy: CachePolicy,
    ) -> anyhow::Result<Self> {
        let ipv4_searcher = if let Some(ipv4_db) = ipv4_db {
            Some(Searcher::new(ipv4_db, policy)?)
        } else {
            None
        };

        let ipv6_searcher = if let Some(ipv6_db) = ipv6_db {
            Some(Searcher::new(ipv6_db, policy)?)
        } else {
            None
        };

        return Ok(Locator {
            ipv4_searcher: ipv4_searcher,
            ipv6_searcher: ipv6_searcher,
        });
    }

    fn format_location(location: String) -> String {
        // 中国|福建省|福州市|中国电信|CN
        // -> 中国福建省福州市中国电信

        // US|X|X|X
        // -> US X X X
        // 同时取消占位的 "0"
        let parts: Vec<_> = location.split("|").filter(|x| *x != "0").collect();

        if parts.is_empty() {
            return location;
        }

        if parts[0] == "Reserved" {
            return "局域网".to_owned();
        }

        // 去掉最后的 |CN
        let all_ascii = parts.iter().all(|x| x.is_ascii());

        if all_ascii {
            return parts[..parts.len() - 1].join(" ");
        } else {
            return parts[..parts.len() - 1].join("");
        }
    }

    pub fn locate(&self, ip: &str) -> String {
        if ip.contains(":") {
            if let Some(searcher) = &self.ipv6_searcher {
                return searcher
                    .search(ip)
                    .map(|x| Self::format_location(x))
                    .unwrap_or_else(|e| format!("数据库查询错误: {}", e));
            } else {
                return DATABASE_NOT_CONFIG.to_owned();
            }
        } else {
            if let Some(searcher) = &self.ipv4_searcher {
                return searcher
                    .search(ip)
                    .map(|x| Self::format_location(x))
                    .unwrap_or_else(|e| format!("数据库查询错误: {}", e));
            } else {
                return DATABASE_NOT_CONFIG.to_owned();
            }
        }
    }
}
