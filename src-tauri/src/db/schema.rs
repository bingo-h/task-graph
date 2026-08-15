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
        created_at  TEXT NOT NULL,
        end         TEXT,
        tags        TEXT NOT NULL DEFAULT '[]',
        depends     TEXT NOT NULL DEFAULT '[]',
        annotations TEXT NOT NULL DEFAULT '[]'
    );
    
    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER NOT NULL
    );
    "#,
    // 版本 2: 任务计时记录表
    r#"
    CREATE TABLE IF NOT EXISTS time_entries (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        task_uuid   TEXT NOT NULL,
        start       TEXT NOT NULL,
        end         TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_time_entries_task_uuid
        ON time_entries (task_uuid);
    "#,
    // 版本 3: 项目表，允许项目独立于任务存在
    r#"
    CREATE TABLE IF NOT EXISTS projects (
        path       TEXT PRIMARY KEY,
        created_at TEXT NOT NULL
    );
    "#,
    // 版本 4: 项目归档状态
    r#"
    ALTER TABLE projects ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
    "#,
    // 版本 5: 项目阶段（计划中 / 进行中），未设置时默认视为进行中
    r#"
    ALTER TABLE projects ADD COLUMN stage TEXT NOT NULL DEFAULT 'active';
    "#,
    // 版本 6: 项目废纸篓（软删除），trashed_at 用于计算自动彻底删除的到期时间
    r#"
    ALTER TABLE projects ADD COLUMN trashed INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE projects ADD COLUMN trashed_at TEXT;
    "#,
    // 版本 7: 计时记录的回忆总结（每段专注结束时可填写标题+正文，事后也能修改）
    r#"
    ALTER TABLE time_entries ADD COLUMN note_title TEXT;
    ALTER TABLE time_entries ADD COLUMN note_body TEXT;
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
