use sha1::Digest;
use std::{fs, path::Path};

pub fn persist_upload_file(content: &[u8], upload_dir: &str) -> anyhow::Result<String> {
    let mut hasher = sha1::Sha1::new();
    hasher.update(content);
    let hash = hex::encode(hasher.finalize());
    // hash 文件名作为
    fs::write(Path::new(upload_dir).join(&hash), content)?;
    return Ok(hash);
}
