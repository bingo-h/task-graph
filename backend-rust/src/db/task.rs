//! 任务数据库操作
//!
//! 所有函数接受 &Connection ，由调用方管理连接生命周期
//! urgency 在每次写操作后自动重新计算
//!
use anyhow::Result;
use rusqlite::Connection;

// ----------------------------------------
// 查询操作
// ----------------------------------------
pub fn list_all(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT uuid, description, status, project, priority, urgency, due, scheduled, entry, end, tags, depends, annotations
        FROM tasks
        WHERE status != 'deleted'
        ORDER BY urgency DESC
        "
    )?;

    let tasks = stmt
        .query_map([], row_to_task)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(tasks)
}

/// 按 UUID 查询单个任务
pub fn get_by_uuid(conn: &Connection, uuid: &str) -> Result<Option<Task>> {
    let mut stmt = conn.prepare(
        "SELECT uuid, description, status, project, priority, urgency, due, scheduled, entry, end, tags, depends, annotations FROM tasks WHERE uuid = ?1"
    )?;

    let mut rows = stmt.query_map([uuid], row_to_task)?;

    Ok(rows.next().transpose()?)
}

/// 请求体：创建任务
pub struct CreateTaskRequest {
    pub description: String,
    pub project: Option<String>,
    pub priority: Option<String>,
    pub due: Option<String>,
    pub scheduled: Option<String>,
    pub tags: Vec<String>,
    pub depends: Vec<String>,
}

/// 请求体：更新任务
pub struct UpdateTaskRequest {
    pub description: Option<String>,
    pub project: Option<String>,
    pub priority: Option<String>,
    pub due: Option<String>,
    pub scheduled: Option<String>,
    pub tags: Option<Vec<String>>,
    pub depends: Option<Vec<String>>,
    /// 显式清空 project 字段（设为 NULL）
    pub clear_project: bool,
    /// 显式清空 priority 字段
    pub clear_priority: bool,
    /// 显式清空 due 字段
    pub clear_due: bool,
    /// 显式清空 scheduled 字段
    pub clear_scheduled: bool,
}

/// 将数据库行转换为 Task 结构体
fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    let tags_json: String = row.get(10)?;
    let depends_json: String = row.get(11)?;
    let annotations_json: String = row.get(12)?;

    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    let depends_json: Vec<String> = serde_json::from_str(&depends_json).unwrap_or_default();
    let annotations: Vec<String> = serde_json::from_str(&annotations_json).unwrap_or_default();

    let status_str: String = row.get(2)?;
    let status = match status_str.as_str() {
        "completed" => TaskStatus::Completed,
    };
}
