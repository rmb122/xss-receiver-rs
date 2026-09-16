use std::borrow::Cow;
use std::fmt::Display;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::net::IpAddr;
use std::path::Path;
use std::sync::OnceLock;

use crate::utils::ip2region::header::{
    HEADER_INFO_LENGTH, Header, IpVersion, VECTOR_INDEX_COLS, VECTOR_INDEX_LENGTH,
    VECTOR_INDEX_SIZE,
};

use crate::utils::ip2region::error::{Ip2RegionError, Result};
use crate::utils::ip2region::ip_value::{CompareExt, IpValueExt};

pub struct Searcher {
    pub filepath: String,
    pub cache_policy: CachePolicy,
    pub header: Header,
    vector_cache: OnceLock<Vec<u8>>,
    full_cache: OnceLock<Vec<u8>>,
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum CachePolicy {
    NoCache,
    VectorIndex,
    FullMemory,
}

impl Searcher {
    pub fn new(filepath: String, cache_policy: CachePolicy) -> Result<Self> {
        let mut file = File::open(Path::new(&filepath))?;
        let mut buf = [0; HEADER_INFO_LENGTH];
        file.read_exact(&mut buf)?;

        let header = Header::try_from(&buf)?;

        Ok(Self {
            filepath,
            cache_policy,
            header,
            vector_cache: OnceLock::new(),
            full_cache: OnceLock::new(),
        })
    }

    pub fn search<T>(&self, ip: T) -> Result<String>
    where
        T: IpValueExt + Display,
    {
        let ip = ip.to_ipaddr()?;

        let (il0, il1) = match (ip, self.header.ip_version()) {
            (IpAddr::V6(ip), IpVersion::V6) => (ip.octets()[0], ip.octets()[1]),
            (IpAddr::V4(ip), IpVersion::V4) => (ip.octets()[0], ip.octets()[1]),
            (_, IpVersion::V4) => return Err(Ip2RegionError::OnlyIPv4Version),
            (_, IpVersion::V6) => return Err(Ip2RegionError::OnlyIPv6Version),
        };

        let start_point = VECTOR_INDEX_SIZE * ((il0 as usize) * VECTOR_INDEX_COLS + (il1 as usize));
        let vector_index = self.vector_index()?;
        let start_ptr =
            u32::from_le_bytes(vector_index[start_point..start_point + 4].try_into()?) as usize;
        let end_ptr =
            u32::from_le_bytes(vector_index[start_point + 4..start_point + 8].try_into()?) as usize;

        // As in upstream, zero vector pointers mean this prefix has no source data.
        if start_ptr == 0 || end_ptr == 0 {
            return Ok(String::new());
        }

        // Binary search the segment index to get the region
        let segment_index_size = self.header.segment_index_size();
        let ip_bytes_len = self.header.ip_bytes_len();
        let ip_end_offset = ip_bytes_len * 2;

        let mut left: usize = 0;
        let mut right: usize = (end_ptr - start_ptr) / segment_index_size;

        while left <= right {
            let mid = (left + right) >> 1;
            let offset = start_ptr + mid * segment_index_size;
            let buffer_ip_value = self.read_buf(offset, segment_index_size)?;
            if ip.ip_lt(Cow::Borrowed(&buffer_ip_value[0..ip_bytes_len])) {
                let Some(m) = mid.checked_sub(1) else { break };
                right = m;
            } else if ip.ip_gt(Cow::Borrowed(&buffer_ip_value[ip_bytes_len..ip_end_offset])) {
                left = mid + 1;
            } else {
                let data_length = u16::from_le_bytes([
                    buffer_ip_value[ip_end_offset],
                    buffer_ip_value[ip_end_offset + 1],
                ]);
                let data_offset = u32::from_le_bytes(
                    buffer_ip_value[ip_end_offset + 2..ip_end_offset + 6].try_into()?,
                );
                let result = String::from_utf8(
                    self.read_buf(data_offset as usize, data_length as usize)?
                        .to_vec(),
                )?;
                return Ok(result);
            }
        }
        // From xdb 3.0 version no matched IP result change to empty string,
        // so users should check string is empty.
        //
        // Err(Ip2RegionError::NoMatchedIP)
        Ok(String::new())
    }

    pub fn vector_index(&self) -> Result<Cow<'_, [u8]>> {
        if self.cache_policy.eq(&CachePolicy::NoCache) {
            return self.read_buf(HEADER_INFO_LENGTH, VECTOR_INDEX_LENGTH);
        }

        match self.vector_cache.get() {
            None => {
                let data = self
                    .read_buf(HEADER_INFO_LENGTH, VECTOR_INDEX_LENGTH)?
                    .to_vec();
                let _ = self.vector_cache.set(data).inspect_err(|_| {});

                // Safety: vector cache checked and set for empty before
                let cache = self.vector_cache.get().unwrap();
                Ok(Cow::Borrowed(cache))
            }
            Some(cache) => Ok(Cow::Borrowed(cache)),
        }
    }

    pub fn read_buf(&self, offset: usize, size: usize) -> Result<Cow<'_, [u8]>> {
        if self.cache_policy.ne(&CachePolicy::FullMemory) {
            let mut file = File::open(&self.filepath)?;
            file.seek(SeekFrom::Start(offset as u64))?;

            let mut buf = vec![0u8; size];
            file.take(size as u64).read_exact(&mut buf)?;
            return Ok(Cow::from(buf));
        }

        match self.full_cache.get() {
            None => {
                let mut file = File::open(&self.filepath)?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                let _ = self.full_cache.set(buf).inspect_err(|_| {});

                // Safety: FULL_CACHE checked and set for empty before
                let cache = self.full_cache.get().unwrap();
                Ok(Cow::from(&cache[offset..offset + size]))
            }
            Some(cache) => {
                let data = Cow::from(&cache[offset..offset + size]);
                Ok(data)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{BufRead, BufReader, Write};
    use std::path::PathBuf;
    use std::str::FromStr;

    use super::*;

    // Test ipv6 need after run command `git lfs pull`
    const IPV4_XDB_PATH: &str = "../../../data/ip2region_v4.xdb";
    const IPV4_CHECK_PATH: &str = "../../../data/ipv4_source.txt";
    const IPV6_XDB_PATH: &str = "../../../data/ip2region_v6.xdb";
    const IPV6_CHECK_PATH: &str = "../../../data/ipv6_source.txt";

    struct TestXdb(PathBuf);

    impl TestXdb {
        fn new(bytes: &[u8]) -> Self {
            let path = std::env::temp_dir().join(format!(
                "xss-receiver-ip2region-{}-{}.xdb",
                std::process::id(),
                rand::random::<u64>()
            ));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            file.write_all(bytes).unwrap();
            Self(path)
        }
    }

    impl Drop for TestXdb {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).unwrap();
        }
    }

    #[test]
    fn missing_vector_returns_empty_for_all_ip_versions_and_cache_policies() {
        const REGION: &str = "中国|广东省|深圳市|0|CN";

        for (start_ip, end_ip, matching_ip, missing_ip) in [
            ("1.1.1.0", "1.1.1.255", "1.1.1.1", "1.0.0.1"),
            ("2001:db8::100", "2001:db8::1ff", "2001:db8::123", "400::1"),
        ] {
            let start_ip: IpAddr = start_ip.parse().unwrap();
            let end_ip: IpAddr = end_ip.parse().unwrap();
            let missing_ip: IpAddr = missing_ip.parse().unwrap();
            let data_offset = (HEADER_INFO_LENGTH + VECTOR_INDEX_LENGTH) as u32;
            let index_offset = data_offset + REGION.len() as u32;

            for (missing_start, missing_end) in [(0, 0), (0, index_offset), (index_offset, 0)] {
                let mut bytes = vec![0; data_offset as usize];
                bytes[0..2].copy_from_slice(&3u16.to_le_bytes());
                bytes[2..4].copy_from_slice(&1u16.to_le_bytes());
                bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
                bytes[8..12].copy_from_slice(&index_offset.to_le_bytes());
                bytes[12..16].copy_from_slice(&index_offset.to_le_bytes());
                let version: u16 = if start_ip.is_ipv4() { 4 } else { 6 };
                bytes[16..18].copy_from_slice(&version.to_le_bytes());
                bytes[18..20].copy_from_slice(&4u16.to_le_bytes());
                // Poison reserved header bytes: dereferencing a zero vector must
                // not treat the header as an IPv6 segment with a data pointer.
                bytes[32..34].copy_from_slice(&1u16.to_le_bytes());
                bytes[34..38].copy_from_slice(&u32::MAX.to_le_bytes());

                for (ip, first, last) in [
                    (start_ip, index_offset, index_offset),
                    (missing_ip, missing_start, missing_end),
                ] {
                    let prefix = match ip {
                        IpAddr::V4(ip) => [ip.octets()[0], ip.octets()[1]],
                        IpAddr::V6(ip) => [ip.octets()[0], ip.octets()[1]],
                    };
                    let offset = HEADER_INFO_LENGTH
                        + VECTOR_INDEX_SIZE
                            * (prefix[0] as usize * VECTOR_INDEX_COLS + prefix[1] as usize);
                    bytes[offset..offset + 4].copy_from_slice(&first.to_le_bytes());
                    bytes[offset + 4..offset + 8].copy_from_slice(&last.to_le_bytes());
                }

                bytes.extend_from_slice(REGION.as_bytes());
                for ip in [start_ip, end_ip] {
                    match ip {
                        IpAddr::V4(ip) => bytes.extend(u32::from(ip).to_le_bytes()),
                        IpAddr::V6(ip) => bytes.extend(ip.octets()),
                    }
                }
                bytes.extend((REGION.len() as u16).to_le_bytes());
                bytes.extend(data_offset.to_le_bytes());
                let database = TestXdb::new(&bytes);

                for policy in [
                    CachePolicy::NoCache,
                    CachePolicy::VectorIndex,
                    CachePolicy::FullMemory,
                ] {
                    let searcher =
                        Searcher::new(database.0.to_string_lossy().into_owned(), policy).unwrap();
                    assert_eq!(searcher.search(matching_ip).unwrap(), REGION);
                    assert_eq!(
                        searcher.search(missing_ip.to_string().as_str()).unwrap(),
                        "",
                        "{missing_ip}, {policy:?}, vector=({missing_start}, {missing_end})"
                    );
                }
            }
        }
    }

    ///test all types find correct
    #[test]
    fn test_multi_type_ip() {
        for cache_policy in [
            CachePolicy::NoCache,
            CachePolicy::FullMemory,
            CachePolicy::VectorIndex,
        ] {
            let searcher = Searcher::new(IPV4_XDB_PATH.to_owned(), cache_policy).unwrap();
            searcher.search("1.0.1.0").unwrap();
            searcher.search("1.0.1.2").unwrap();
            searcher.search(0u32).unwrap();

            let searcher = Searcher::new(IPV6_XDB_PATH.to_owned(), cache_policy).unwrap();
            searcher.search("2c0f:fff1::").unwrap();
            searcher.search("2c0f:fff1::1").unwrap();
            searcher.search(111u128).unwrap();
        }
    }

    fn match_ip_correct(xdb_filepath: &str, check_path: &str, cache_policy: CachePolicy) {
        let searcher = Searcher::new(xdb_filepath.to_owned(), cache_policy).unwrap();

        let file = File::open(check_path).unwrap();
        let reader = BufReader::new(file);

        for line in reader.lines().take(10_000) {
            let line = line.unwrap();

            if !line.contains("|") {
                continue;
            }

            let ip_test_line = line.splitn(3, "|").collect::<Vec<&str>>();
            let start_ip = IpAddr::from_str(ip_test_line[0]).unwrap();
            let end_ip = IpAddr::from_str(ip_test_line[1]).unwrap();
            for _ in 0..3 {
                let result = match (start_ip, end_ip) {
                    (IpAddr::V4(start), IpAddr::V4(end)) => {
                        let value = rand::random_range(u32::from(start)..u32::from(end) + 1);
                        searcher.search(value).unwrap()
                    }
                    (IpAddr::V6(start), IpAddr::V6(end)) => {
                        let value = rand::random_range(u128::from(start)..u128::from(end) + 1);
                        searcher.search(value).unwrap()
                    }
                    _ => panic!("invalid ip address"),
                };
                assert_eq!(result.as_str(), ip_test_line[2])
            }
        }
    }

    #[test]
    fn test_match_ip_correct() {
        for cache_policy in [
            CachePolicy::NoCache,
            CachePolicy::FullMemory,
            CachePolicy::VectorIndex,
        ] {
            match_ip_correct(IPV4_XDB_PATH, IPV4_CHECK_PATH, cache_policy);
            match_ip_correct(IPV6_XDB_PATH, IPV6_CHECK_PATH, cache_policy);
        }
    }
}
