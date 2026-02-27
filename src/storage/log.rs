use sha1::Digest;
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;

use crate::storage::validate_hex_string;

/// LogStorage - 用于保存 http_log 中上传的文件
pub struct LogStorage {
    path: PathBuf,
}

impl LogStorage {
    pub(crate) fn new(path: PathBuf) -> Self {
        LogStorage { path }
    }

    /// 保存文件内容，计算 hash 并返回文件名
    /// 如果文件已存在则不重复保存
    pub fn save(&self, content: &[u8]) -> anyhow::Result<String> {
        let mut hasher = sha1::Sha1::new();
        hasher.update(content);
        let hash = hex::encode(hasher.finalize());

        let file_path = self.path.join(&hash);

        // 检查文件是否已存在
        if !file_path.exists() {
            fs::write(&file_path, content)?;
        }

        Ok(hash)
    }

    /// 打开文件
    pub fn open(&self, hash: &str, options: &mut OpenOptions) -> anyhow::Result<File> {
        let file_path = self.path.join(validate_hex_string(hash)?);
        let content = options.open(file_path)?;
        Ok(content)
    }
}
