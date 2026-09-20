use std::{
    error::Error,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    str::FromStr,
};

use reqwest::{
    Url,
    dns::{Addrs, Name, Resolve, Resolving},
};

type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Debug)]
pub(super) struct FilteringResolver {
    pub(super) allow_private_network: bool,
}

impl Resolve for FilteringResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        let allow_private_network = self.allow_private_network;

        Box::pin(async move {
            let resolved = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let addresses: Vec<SocketAddr> = resolved
                .filter(|address| address_allowed(address.ip(), allow_private_network))
                .collect();

            if addresses.is_empty() {
                let error: BoxError = Box::new(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("host {host} did not resolve to an allowed address"),
                ));
                return Err(error);
            }

            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

pub(super) fn validate_url(url: &Url, allow_private_network: bool) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err("only absolute http:// and https:// URLs are supported".to_owned());
    }

    let host = url
        .host()
        .ok_or_else(|| "URL must contain a host".to_owned())?;
    let ip = match host {
        url::Host::Ipv4(ip) => Some(IpAddr::V4(ip)),
        url::Host::Ipv6(ip) => Some(IpAddr::V6(ip)),
        url::Host::Domain(_) => None,
    };

    if ip.is_some_and(|ip| !address_allowed(ip, allow_private_network)) {
        return Err(format!("target address {} is not allowed", ip.unwrap()));
    }
    Ok(())
}

fn address_allowed(ip: IpAddr, allow_private_network: bool) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            if ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast() {
                return false;
            }
            allow_private_network || is_public_ipv4(ip)
        }
        IpAddr::V6(ip) => {
            if ip.is_unspecified() || ip.is_multicast() {
                return false;
            }
            if let Some(ipv4) = ip.to_ipv4_mapped() {
                return address_allowed(IpAddr::V4(ipv4), allow_private_network);
            }
            allow_private_network || is_public_ipv6(ip)
        }
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let value = u32::from(ip);
    ![
        ("0.0.0.0", 8),
        ("10.0.0.0", 8),
        ("100.64.0.0", 10),
        ("127.0.0.0", 8),
        ("169.254.0.0", 16),
        ("172.16.0.0", 12),
        ("192.0.0.0", 24),
        ("192.0.2.0", 24),
        ("192.88.99.0", 24),
        ("192.168.0.0", 16),
        ("198.18.0.0", 15),
        ("198.51.100.0", 24),
        ("203.0.113.0", 24),
        ("224.0.0.0", 4),
        ("240.0.0.0", 4),
    ]
    .into_iter()
    .any(|(network, prefix)| {
        let network = u32::from(Ipv4Addr::from_str(network).expect("valid network constant"));
        let mask = u32::MAX << (32 - prefix);
        value & mask == network & mask
    })
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();

    // Currently allocated global unicast space is 2000::/3.
    if segments[0] & 0xe000 != 0x2000 {
        return false;
    }
    // IETF protocol assignments (2001::/23), documentation ranges, and 3fff::/20.
    if segments[0] == 0x2001 && segments[1] <= 0x01ff {
        return false;
    }
    if segments[0] == 0x2001 && segments[1] == 0x0db8 {
        return false;
    }
    // 6to4 embeds an IPv4 tunnel endpoint and must not bypass the IPv4 policy.
    if segments[0] == 0x2002 {
        return false;
    }
    if segments[0] == 0x3fff && segments[1] & 0xf000 == 0 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::{address_allowed, validate_url};

    #[test]
    fn blocks_non_public_addresses_by_default() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "169.254.169.254",
            "192.168.1.1",
            "198.18.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:7f00:1::",
            "2002:808:808::",
            "::ffff:127.0.0.1",
        ] {
            assert!(!address_allowed(address.parse::<IpAddr>().unwrap(), false));
        }
    }

    #[test]
    fn allows_public_addresses_by_default() {
        for address in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(address_allowed(address.parse::<IpAddr>().unwrap(), false));
        }
    }

    #[test]
    fn private_network_mode_still_rejects_non_unicast_targets() {
        assert!(address_allowed("127.0.0.1".parse().unwrap(), true));
        assert!(address_allowed("::1".parse().unwrap(), true));
        assert!(!address_allowed("0.0.0.0".parse().unwrap(), true));
        assert!(!address_allowed("ff02::1".parse().unwrap(), true));
    }

    #[test]
    fn blocks_obscured_private_ip_literals() {
        for url in [
            "http://127.1/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://0177.0.0.1/",
            "http://[::ffff:127.0.0.1]/",
        ] {
            let url = reqwest::Url::parse(url).unwrap();
            assert!(validate_url(&url, false).is_err(), "URL was allowed: {url}");
        }
    }
}
