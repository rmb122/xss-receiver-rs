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
        // 国家|省份|城市|ISP|国家代码
        let mut parts: Vec<_> = location.split('|').collect();
        if parts.first() == Some(&"Reserved") {
            return "局域网".to_owned();
        }

        // 先移除国家代码，避免代码为占位值时误删最后一个有效字段。
        parts.pop();
        // 直辖市等同名省市只显示一次，国家和 ISP 不参与去重。
        if parts.len() >= 3 && parts[1] == parts[2] {
            parts.remove(2);
        }
        parts.retain(|part| !part.is_empty() && *part != "0");

        let separator = if parts.iter().all(|part| part.is_ascii()) {
            " "
        } else {
            ""
        };
        parts.join(separator)
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

#[cfg(test)]
mod tests {
    use super::Locator;

    #[test]
    fn format_location_handles_upstream_region_fields() {
        for (location, expected) in [
            ("中国|北京市|北京市|联通|CN", "中国北京市联通"),
            ("中国|上海市|上海市|电信|CN", "中国上海市电信"),
            ("中国|福建省|福州市|电信|CN", "中国福建省福州市电信"),
            (
                "United States|California|0|Google LLC|US",
                "United States California Google LLC",
            ),
            ("Singapore|Singapore|Singapore|0|SG", "Singapore Singapore"),
            ("中国|北京市|北京市|北京市|CN", "中国北京市北京市"),
            ("中国|0||联通|CN", "中国联通"),
            ("中国|北京市|北京市|联通|0", "中国北京市联通"),
            ("中国|北京市|北京市|联通|", "中国北京市联通"),
            ("Reserved|Reserved|Reserved|0|0", "局域网"),
            ("0|0|0|0|0", ""),
            ("||||", ""),
            ("", ""),
        ] {
            assert_eq!(
                Locator::format_location(location.to_owned()),
                expected,
                "{location}"
            );
        }
    }
}
