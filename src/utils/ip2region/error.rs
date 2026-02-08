#[derive(Debug, thiserror::Error)]
pub enum MakerError {
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Header parse error: {0}")]
    HeaderParsed(String),

    #[error("Ipaddr parse error: {0}")]
    IpaddrParseError(#[from] std::net::AddrParseError),

    #[error("Try from int failed")]
    TryFromIntError(#[from] std::num::TryFromIntError),

    #[error("Try from slice failed")]
    TryFromSliceFailed(#[from] std::array::TryFromSliceError),
}

#[derive(Debug, thiserror::Error)]
pub enum Ip2RegionError {
    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("From UTF-8 error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Parse invalid IP address")]
    ParseIpaddressFailed,

    #[error("Searcher load IPv4 data, couldn't search IPv6 data")]
    OnlyIPv4Version,

    #[error("Searcher load IPv6 data, couldn't search IPv4 data")]
    OnlyIPv6Version,

    #[error("Try from slice failed")]
    TryFromSliceFailed(#[from] std::array::TryFromSliceError),

    #[error("Maker crate error: {0}")]
    MakerError(#[from] MakerError),
}

pub type Result<T> = std::result::Result<T, Ip2RegionError>;
