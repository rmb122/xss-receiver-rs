use sha2::{Digest, Sha256};

use super::random::get_random_bytes;

pub const TOKEN_PREFIX: &str = "xrt_";

pub fn generate_token() -> String {
    format!("{TOKEN_PREFIX}{}", hex::encode(get_random_bytes(32)))
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
