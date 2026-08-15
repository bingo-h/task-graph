//! 数据库连接管理。
//!
//! 使用 SQLite 作为本地存储，通过 rusqlite 的 `bundled` feature
//! 将 SQLite 编译进二进制，用户无需安装任何依赖。
//!
//! 数据库文件位置（按优先级）：
//!   1. `TASK_WEB_DATA` 环境变量指定的路径
//!   2. `~/.local/share/task-web/tasks.db`（Linux）
//!   3. `%APPDATA%\task-web\tasks.db`（Windows）

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;

pub mod project;
pub mod schema;
pub mod task;

/// 获取数据库文件路径
pub fn db_path() -> PathBuf {
    if let Ok(path) = std::env::var("TASK_WEB_DATA") {
        return PathBuf::from(path);
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    exe_dir.join("tasks.db")
}

/// 打开数据库连接并确保表结构已经初始化
pub fn open() -> Result<Connection> {
    let path = db_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建数据目录：{:?}", parent))?;
    }

    let conn = Connection::open(&path).with_context(|| format!("无法打开数据库：{:?}", path))?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    schema::init(&conn)?;

    Ok(conn)
}
