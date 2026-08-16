//! 数据库连接管理。
//!
//! 使用 SQLite 作为本地存储，通过 rusqlite 的 `bundled` feature
//! 将 SQLite 编译进二进制，用户无需安装任何依赖。
//!
//! 数据库文件位置（按优先级）：
//!   1. `TASK_GRAPH_DATA` 环境变量指定的路径
//!   2. 可执行文件所在目录（便携模式）
//!   3. 若目录 2 不可写（例如 NixOS 下二进制位于只读的 `/nix/store`），
//!      回退到用户数据目录：Linux 遵循 XDG Base Directory 规范
//!      （`$XDG_DATA_HOME` 或 `~/.local/share`），macOS/Windows 使用各自的标准数据目录

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub mod project;
pub mod schema;
pub mod tag;
pub mod task;
pub mod time_entry;

/// 获取数据库文件路径
///
/// 可写性探测只在进程内做一次并缓存，避免每次打开连接都触发一次文件 I/O。
pub fn db_path() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();

    PATH.get_or_init(|| {
        if let Ok(path) = std::env::var("TASK_GRAPH_DATA") {
            return PathBuf::from(path);
        }

        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        if is_writable(&exe_dir) {
            exe_dir.join("tasks.db")
        } else {
            fallback_data_dir().join("tasks.db")
        }
    })
    .clone()
}

/// 探测目录是否可写：尝试创建一个临时文件，成功后立即删除
fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".task-graph-write-test");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// 回退到用户标准数据目录（XDG Base Directory 规范 / 各平台等价目录）
fn fallback_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("task-graph")
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
