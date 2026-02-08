use sha1::Digest;
use std::path::PathBuf;
use tokio::fs;

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
    pub async fn save(&self, content: &[u8]) -> anyhow::Result<String> {
        let mut hasher = sha1::Sha1::new();
        hasher.update(content);
        let hash = hex::encode(hasher.finalize());

        let file_path = self.path.join(&hash);

        // 检查文件是否已存在
        if !fs::try_exists(&file_path).await? {
            fs::write(&file_path, content).await?;
        }

        Ok(hash)
    }

    /// 读取指定 hash 的文件内容
    pub async fn read(&self, hash: &str) -> anyhow::Result<Vec<u8>> {
        let file_path = self.path.join(validate_hex_string(hash)?);
        let content = fs::read(file_path).await?;
        Ok(content)
    }

    /// 删除指定 hash 的文件
    pub async fn delete(&self, hash: &str) -> anyhow::Result<()> {
        let file_path = self.path.join(validate_hex_string(hash)?);
        fs::remove_file(file_path).await?;
        Ok(())
    }
}
