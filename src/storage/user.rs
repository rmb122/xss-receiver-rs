use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

/// 目录项类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    File,
    Directory,
}

/// 目录项信息
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Entry {
    /// basename（不含路径）
    pub name: String,
    /// 类型
    pub kind: EntryKind,
    /// 大小，目录为 0
    pub size: u64,
    /// 最后修改时间（Unix 时间戳，秒）
    pub modified_time: i64,
}

/// UserStorage - 嵌套目录的用户文件存储
#[derive(Clone)]
pub struct UserStorage {
    path: PathBuf,
}

impl UserStorage {
    pub(crate) fn new(path: PathBuf) -> Self {
        UserStorage { path }
    }

    /// 词法校验并解析为绝对路径。
    /// 空字符串 -> root 本身。
    /// 禁止: ".", "..", 包含 '\0' 的字符串。
    fn resolve(&self, path: &str) -> anyhow::Result<PathBuf> {
        if path.contains('\0') {
            anyhow::bail!("path contains null byte");
        }

        let mut result = self.path.clone();
        if path.is_empty() {
            return Ok(result);
        }

        for segment in path.split(|c| c == '/' || c == '\\') {
            if segment.is_empty() {
                // do noting
                continue;
            }
            if segment == "." || segment == ".." {
                anyhow::bail!("invalid path segment: {}", segment);
            }
            if segment.contains('\0') {
                anyhow::bail!("path contains null byte");
            }
            result.push(segment);
        }
        Ok(result)
    }

    /// 列出 `path` 目录下的直接子项（文件 + 子目录）
    pub fn list(&self, path: &str) -> anyhow::Result<Vec<Entry>> {
        let abs = self.resolve(path)?;
        let mut result = Vec::new();

        for entry in fs::read_dir(&abs)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let name = match entry.file_name().to_str() {
                Some(s) => s.to_string(),
                None => continue,
            };

            match Self::build_entry(name, &metadata) {
                Some(e) => result.push(e),
                None => continue,
            }
        }

        Ok(result)
    }

    /// 获取单个路径的 Entry 信息
    pub fn stat(&self, path: &str) -> anyhow::Result<Entry> {
        let abs = self.resolve(path)?;
        let metadata = fs::metadata(&abs)?;
        let name = abs
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        Self::build_entry(name, &metadata)
            .ok_or_else(|| anyhow::anyhow!("path is neither a file nor a directory"))
    }

    fn build_entry(name: String, metadata: &fs::Metadata) -> Option<Entry> {
        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_file() {
            EntryKind::File
        } else {
            return None;
        };

        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };
        let modified_time = metadata
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs() as i64;

        Some(Entry {
            name,
            kind,
            size,
            modified_time,
        })
    }

    /// 递归列出所有文件的相对路径
    pub fn list_all_files(&self) -> anyhow::Result<Vec<String>> {
        let mut result = Vec::new();
        self.collect_files(&self.path, &mut Vec::new(), &mut result)?;
        Ok(result)
    }

    fn collect_files(
        &self,
        dir: &std::path::Path,
        stack: &mut Vec<String>,
        out: &mut Vec<String>,
    ) -> anyhow::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let name = match entry.file_name().to_str() {
                Some(s) => s.to_string(),
                None => continue,
            };

            if metadata.is_dir() {
                stack.push(name);
                self.collect_files(&entry.path(), stack, out)?;
                stack.pop();
            } else if metadata.is_file() {
                let mut parts = stack.clone();
                parts.push(name);
                out.push(parts.join("/"));
            }
        }
        Ok(())
    }

    /// 递归创建目录
    pub fn mkdir(&self, path: &str) -> anyhow::Result<()> {
        let abs = self.resolve(path)?;
        fs::create_dir_all(&abs)?;
        Ok(())
    }

    /// 删除文件或目录（目录递归删除）
    pub fn remove(&self, path: &str) -> anyhow::Result<()> {
        let abs = self.resolve(path)?;
        if abs == self.path {
            return Err(anyhow::anyhow!("can't remove root directory"));
        }

        let metadata = fs::metadata(&abs)?;
        if metadata.is_dir() {
            fs::remove_dir_all(&abs)?;
        } else {
            fs::remove_file(&abs)?;
        }
        Ok(())
    }

    /// 重命名或移动文件/目录。调用方保证 dst 的父目录已存在。
    pub fn rename(&self, src: &str, dst: &str) -> anyhow::Result<()> {
        let src_abs = self.resolve(src)?;
        let dst_abs = self.resolve(dst)?;
        fs::rename(&src_abs, &dst_abs)?;
        Ok(())
    }

    /// 路径是否存在
    pub fn exists(&self, path: &str) -> bool {
        match self.resolve(path) {
            Ok(abs) => abs.exists(),
            Err(_) => false,
        }
    }

    /// 打开文件
    pub fn open(&self, path: &str, options: &mut fs::OpenOptions) -> anyhow::Result<File> {
        let abs = self.resolve(path)?;
        let f = options.open(abs)?;
        Ok(f)
    }

    /// 一次性读取文件
    pub fn read(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        let abs = self.resolve(path)?;
        Ok(fs::read(abs)?)
    }

    /// 获取文件元数据
    pub fn metadata(&self, path: &str) -> anyhow::Result<fs::Metadata> {
        let abs = self.resolve(path)?;
        Ok(fs::metadata(abs)?)
    }

    /// 写入文件（若父目录不存在则报错）
    pub fn write(&self, path: &str, content: &[u8]) -> anyhow::Result<()> {
        let abs = self.resolve(path)?;
        fs::write(&abs, content)?;
        Ok(())
    }

    /// 追加内容（文件不存在则创建）
    pub fn append(&self, path: &str, content: &[u8]) -> anyhow::Result<()> {
        let abs = self.resolve(path)?;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&abs)?;
        file.write_all(content)?;
        Ok(())
    }

    /// 解析为绝对路径字符串，供 dispatcher 使用
    pub fn absolute_path(&self, path: &str) -> anyhow::Result<String> {
        let abs = self.resolve(path)?;
        Ok(abs.to_string_lossy().to_string())
    }
}
