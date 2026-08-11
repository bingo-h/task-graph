//! 数据库表结构定义
//!
//! 使用简单的版本号机制管理 schema 迁移
//! 每次升级只需在 MIGRATIONS 数组里追加新的 SQL。

use anyhow::Result;
use rusqlite::Connection;

/// 所有迁移 SQL ，按版本号顺序排列
/// 版本号从 1 开始，数组下标 0 对应版本 1
/// 只追加，不修改已有条目
const MIGRATIONS: &[&str] = &[
    // 版本 1: 初始化表结构
    r#"
    CREATE TABLE IF NOT EXISTS tasks (
        uuid        TEXT PRIMARY KEY,
        description TEXT NOT NULL,
        status      TEXT NOT NULL DEFAULT 'pending',
        project     TEXT,
        priority    TEXT,
        urgency     REAL NOT NULL DEFAULT 0,
        due         TEXT,
        scheduled   TEXT,
        entry       TEXT NOT NULL,
        end         TEXT,
        tags        TEXT NOT NULL DEFAULT '[]',
        depends     TEXT NOT NULL DEFAULT '[]',
        annotations TEXT NOT NULL DEFAULT '[]',
    );
    
    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER NOT NULL
    );
    "#,
];

/// 初始化数据库 schema ，自动执行尚未应用的迁移
pub fn init(conn: &Connection) -> Result<()> {
    // 获取当前 schema 的版本
    let current_version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    // 执行所有未应用的迁移
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version > current_version {
            conn.execute_batch(sql)?;

            // 更新版本号
            conn.execute("DELETE FROM schema_version", [])?;
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [version],
            )?;
        }
    }

    Ok(())
}
