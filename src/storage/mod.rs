mod log;
mod temp;
mod user;

pub use log::LogStorage;
pub use temp::TempStorage;
pub use user::{FileInfo, UserStorage};

use std::fs;
use std::path::PathBuf;

/// Storage 主类，管理三种不同类型的存储
pub struct Storage {
    user: UserStorage,
    log: LogStorage,
    temp: TempStorage,
}

impl Storage {
    /// 创建 Storage 实例，会自动创建 user/log/temp 三个子目录
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let base_path = PathBuf::from(path);

        // 确保基础路径存在
        fs::create_dir_all(&base_path)?;

        // 创建三个子目录
        let user_path = base_path.join("user");
        let log_path = base_path.join("log");
        let temp_path = base_path.join("temp");

        // 创建目录，如果已存在则忽略错误
        for dir_path in [&user_path, &log_path, &temp_path] {
            match fs::create_dir(dir_path) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }

        // 创建三个 Storage 实例
        let user = UserStorage::new(user_path);
        let log = LogStorage::new(log_path);
        let temp = TempStorage::new(temp_path);

        Ok(Storage { user, log, temp })
    }

    /// 获取 UserStorage 实例
    pub fn user(&self) -> &UserStorage {
        &self.user
    }

    /// 获取 LogStorage 实例
    pub fn log(&self) -> &LogStorage {
        &self.log
    }

    /// 获取 TempStorage 实例
    pub fn temp(&self) -> &TempStorage {
        &self.temp
    }
}

pub(crate) fn validate_hex_string(id: &str) -> anyhow::Result<&str> {
    // 验证长度必须为 40
    if id.len() != 40 {
        anyhow::bail!("invalid id format");
    }

    // 验证每个字符都是 hex 字符（0-9, a-f, A-F）
    for c in id.chars() {
        if !c.is_ascii_hexdigit() {
            anyhow::bail!("invalid id format");
        }
    }

    Ok(id)
}

/// 验证路径组件的安全性，防止目录穿越攻击
pub fn validate_path_component(component: &str) -> anyhow::Result<()> {
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
