use std::path::PathBuf;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

use crate::storage::validate_hex_string;
use crate::utils::random::get_random_bytes;

/// TempStorage - 用于分片上传文件，最终 merge 成一个完整的文件
pub struct TempStorage {
    path: PathBuf,
}

impl TempStorage {
    pub(crate) fn new(path: PathBuf) -> Self {
        TempStorage { path }
    }

    /// 生成随机的 40 长度 hex id (20 字节)
    fn generate_id() -> String {
        hex::encode(get_random_bytes(20))
    }

    /// 保存内容并返回随机生成的 id（文件名）
    pub async fn save(&self, content: &[u8]) -> anyhow::Result<String> {
        let id = Self::generate_id();
        let file_path = self.path.join(&id);

        fs::write(&file_path, content).await?;

        Ok(id)
    }

    /// 合并多个临时文件到目标路径
    /// ids: 需要合并的临时文件 id 列表（按顺序）
    /// target_path: 目标文件的完整路径
    pub async fn merge(&self, ids: &[String], mut target_file: File) -> anyhow::Result<()> {
        // 先验证都是有效的 id
        for id in ids {
            validate_hex_string(id)?;
        }

        // 按顺序读取并写入每个分片
        for id in ids {
            let chunk_path = self.path.join(id);
            let chunk_content = fs::read(&chunk_path).await?;
            target_file.write_all(&chunk_content).await?;
        }

        target_file.flush().await?;

        // 删除临时文件
        for id in ids {
            let chunk_path = self.path.join(id);
            let _ = fs::remove_file(&chunk_path).await; // 忽略删除错误
        }

        Ok(())
    }

    /// 读取指定 id 的临时文件内容
    pub async fn read(&self, id: &str) -> anyhow::Result<Vec<u8>> {
        let file_path = self.path.join(validate_hex_string(id)?);
        let content = fs::read(file_path).await?;
        Ok(content)
    }

    /// 删除指定 id 的临时文件
    pub async fn delete(&self, id: &str) -> anyhow::Result<()> {
        let file_path = self.path.join(validate_hex_string(id)?);
        fs::remove_file(file_path).await?;
        Ok(())
    }
}
