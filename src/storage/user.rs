use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::storage::validate_path_component;

/// 文件信息
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct FileInfo {
    /// 文件名
    pub name: String,
    /// 文件大小（字节）
    pub size: u64,
    /// 最后修改时间（Unix 时间戳，秒）
    pub modified_time: i64,
}

/// UserStorage - 用于给用户保存文件夹
pub struct UserStorage {
    path: PathBuf,
}

impl UserStorage {
    pub(crate) fn new(path: PathBuf) -> Self {
        UserStorage { path }
    }

    /// 递归列出所有目录及其文件
    pub fn list_all_directory(&self) -> anyhow::Result<HashMap<String, Vec<FileInfo>>> {
        let mut result = HashMap::new();
        let entries = fs::read_dir(&self.path)?;

        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                if let Some(dir_name) = entry.file_name().to_str() {
                    let dir_name_string = dir_name.to_string();

                    // 列出该目录下的所有文件
                    let files = self.list_directory(&dir_name_string)?;
                    result.insert(dir_name_string, files);
                }
            }
        }

        Ok(result)
    }

    /// 列出指定目录下文件
    pub fn list_directory(&self, directory: &str) -> anyhow::Result<Vec<FileInfo>> {
        let mut files = Vec::new();

        let path = self.path.join(directory);
        let entries = fs::read_dir(&path)?;

        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    // 获取文件元数据
                    let metadata = entry.metadata()?;
                    let size = metadata.len();
                    let modified_time =
                        metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs() as i64;

                    files.push(FileInfo {
                        name: name.to_string(),
                        size,
                        modified_time,
                    });
                }
            }
        }

        Ok(files)
    }

    /// 在根目录下创建新目录
    pub fn new_directory(&self, directory: &str) -> anyhow::Result<()> {
        validate_path_component(directory)?;

        let path = self.path.join(directory);

        // 如果目录已存在，返回错误
        fs::create_dir(&path)?;
        Ok(())
    }

    /// 删除目录或文件
    /// - filename 为 Some(name) 时，删除指定目录下的指定文件
    /// - filename 为 None 时，删除指定目录本身
    pub fn delete(&self, directory: &str, filename: Option<&str>) -> anyhow::Result<()> {
        validate_path_component(directory)?;

        match filename {
            Some(name) => {
                validate_path_component(name)?;
                let path = self.path.join(directory).join(name);
                fs::remove_file(&path)?;
            }
            None => {
                let path = self.path.join(directory);
                fs::remove_dir_all(&path)?;
            }
        }

        Ok(())
    }

    /// 重命名或移动目录/文件
    /// - `directory` + `filename`：源路径，`filename` 为 `None` 时表示目录本身，`Some(name)` 时表示目录下的文件
    /// - `new_directory` + `new_filename`：目标路径，语义同上
    pub fn rename(
        &self,
        directory: &str,
        filename: Option<&str>,
        new_directory: &str,
        new_filename: Option<&str>,
    ) -> anyhow::Result<()> {
        validate_path_component(directory)?;
        validate_path_component(new_directory)?;

        let src_path = match filename {
            Some(name) => {
                validate_path_component(name)?;
                self.path.join(directory).join(name)
            }
            None => self.path.join(directory),
        };

        let dst_path = match new_filename {
            Some(name) => {
                validate_path_component(name)?;
                self.path.join(new_directory).join(name)
            }
            None => self.path.join(new_directory),
        };

        fs::rename(&src_path, &dst_path)?;
        Ok(())
    }

    /// 一次性读取文件
    pub fn read_file(&self, directory: &str, name: &str) -> anyhow::Result<Vec<u8>> {
        validate_path_component(directory)?;
        validate_path_component(name)?;

        let path = self.path.join(directory).join(name);

        let content = fs::read(path)?;
        Ok(content)
    }

    pub fn get_absolute_path(&self, handler: &str) -> anyhow::Result<String> {
        let parts = handler.split_once("/");
        let parts = if let Some(parts) = parts {
            parts
        } else {
            anyhow::bail!("invalid handler path: {}", handler)
        };

        validate_path_component(parts.0)?;
        validate_path_component(parts.1)?;

        return Ok(self
            .path
            .join(parts.0)
            .join(parts.1)
            .to_string_lossy()
            .to_string());
    }

    /// 打开目标文件
    pub fn open_file(
        &self,
        directory: &str,
        name: &str,
        options: &mut fs::OpenOptions,
    ) -> anyhow::Result<File> {
        validate_path_component(directory)?;
        validate_path_component(name)?;

        let path = self.path.join(directory).join(name);

        let content = options.open(path)?;
        Ok(content)
    }

    /// 写入文件
    pub fn write_file(&self, directory: &str, name: &str, content: &[u8]) -> anyhow::Result<()> {
        validate_path_component(directory)?;
        validate_path_component(name)?;

        let path = self.path.join(directory).join(name);
        // 写入文件
        fs::write(&path, content)?;
        Ok(())
    }
}
