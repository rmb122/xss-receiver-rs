use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::{self, File};

/// UserStorage - 用于给用户保存文件夹
pub struct UserStorage {
    path: PathBuf,
}

impl UserStorage {
    pub(crate) fn new(path: PathBuf) -> Self {
        UserStorage { path }
    }

    /// 验证路径组件的安全性，防止目录穿越攻击
    fn validate_path_component(component: &str) -> anyhow::Result<()> {
        if component.is_empty() {
            anyhow::bail!("path component cannot be empty");
        }
        if component == ".." || component == "." {
            anyhow::bail!("invalid path component");
        }
        if component.contains('/') || component.contains('\\') {
            anyhow::bail!("path component contains directory traversal");
        }
        if component.contains('\0') {
            anyhow::bail!("path component contains null byte");
        }
        Ok(())
    }

    /// 递归列出所有目录及其文件
    pub async fn list_directory(&self) -> anyhow::Result<HashMap<String, Vec<String>>> {
        let mut result = HashMap::new();
        let mut entries = fs::read_dir(&self.path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                if let Some(dir_name) = entry.file_name().to_str() {
                    let dir_name_string = dir_name.to_string();

                    // 列出该目录下的所有文件
                    let files = self.list_directory_file(&dir_name_string).await?;
                    result.insert(dir_name_string, files);
                }
            }
        }

        Ok(result)
    }

    /// 列出指定目录下文件
    pub async fn list_directory_file(&self, directory: &str) -> anyhow::Result<Vec<String>> {
        let mut files = Vec::new();

        let path = self.path.join(directory);
        let mut entries = fs::read_dir(&path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if file_type.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    files.push(name.to_string());
                }
            }
        }

        Ok(files)
    }

    /// 在根目录下创建新目录
    pub async fn new_directory(&self, directory: &str) -> anyhow::Result<()> {
        Self::validate_path_component(directory)?;

        let path = self.path.join(directory);

        // 如果目录已存在不报错
        match fs::create_dir(&path).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// 删除指定目录
    pub async fn delete_directory(&self, directory: &str) -> anyhow::Result<()> {
        Self::validate_path_component(directory)?;

        let path = self.path.join(directory);

        fs::remove_dir_all(&path).await?;
        Ok(())
    }

    /// 一次性读取文件
    pub async fn read_file(&self, directory: &str, name: &str) -> anyhow::Result<Vec<u8>> {
        Self::validate_path_component(directory)?;
        Self::validate_path_component(name)?;

        let path = self.path.join(directory).join(name);

        let content = fs::read(path).await?;
        Ok(content)
    }

    /// 打开目标文件
    pub async fn open_file(
        &self,
        directory: &str,
        name: &str,
        options: &mut fs::OpenOptions,
    ) -> anyhow::Result<File> {
        Self::validate_path_component(directory)?;
        Self::validate_path_component(name)?;

        let path = self.path.join(directory).join(name);

        let content = options.open(path).await?;
        Ok(content)
    }

    /// 写入文件，如果目录不存在则自动创建
    pub async fn write_file(
        &self,
        directory: &str,
        name: &str,
        content: &[u8],
    ) -> anyhow::Result<()> {
        Self::validate_path_component(directory)?;
        Self::validate_path_component(name)?;

        let path = self.path.join(directory).join(name);
        // 写入文件
        fs::write(&path, content).await?;
        Ok(())
    }

    /// 删除指定文件
    pub async fn delete_file(&self, directory: &str, name: &str) -> anyhow::Result<()> {
        Self::validate_path_component(directory)?;
        Self::validate_path_component(name)?;

        let path = self.path.join(directory).join(name);

        fs::remove_file(&path).await?;
        Ok(())
    }
}
