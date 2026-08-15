//! 任务数据库操作
//!
//! 所有函数接受 &Connection ，由调用方管理连接生命周期
//! urgency 在每次写操作后自动重新计算

use crate::models::task::{Annotation, Priority, Task, TaskStatus};
use crate::models::urgency::compute_urgency;
use anyhow::Result;
use rusqlite::{params, Connection};
use uuid::Uuid;

/// 请球体：创建新任务
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
/// 只更新明确提供的字段，clear_* 用于显式清空可选字段
pub struct UpdateTaskRequest {
    pub description: Option<String>,
    pub project: Option<String>,
    pub priority: Option<String>,
    pub due: Option<String>,
    pub scheduled: Option<String>,
    pub tags: Option<Vec<String>>,
    pub depends: Option<Vec<String>>,
    pub clear_project: bool,
    pub clear_priority: bool,
    pub clear_due: bool,
    pub clear_scheduled: bool,
}

/// 查询所有非删除状态的任务
pub fn list_all(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT uuid, description, status, project, priority, urgency,
             due, scheduled, created_at, end, tags, depends, annotations
        FROM tasks WHERE status != 'deleted' ORDER BY urgency DESC",
    )?;

    let tasks = stmt
        .query_map([], row_to_task)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(tasks)
}

/// 按 UUID 查询单个任务
pub fn get_by_uuid(conn: &Connection, uuid: &str) -> Result<Option<Task>> {
    let mut stmt = conn.prepare(
        "SELECT uuid, description, status, project, priority, urgency,
                    due, scheduled, created_at, end, tags, depends, annotations
              FROM tasks WHERE uuid = ?1",
    )?;

    let mut rows = stmt.query_map([uuid], row_to_task)?;

    Ok(rows.next().transpose()?)
}

/// 创建新任务，返回创建后的任务结构体
pub fn create(conn: &Connection, req: &CreateTaskRequest) -> Result<Task> {
    let uuid = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let tags_json = serde_json::to_string(&req.tags)?;
    let depends_json = serde_json::to_string(&req.depends)?;
    let urgency = compute_urgency(
        req.priority.as_deref(),
        req.due.as_deref(),
        &created_at,
        &req.tags,
        &req.depends,
    );

    conn.execute(
        "
            INSERT INTO tasks
                (uuid, description, status, project, priority, urgency,
                 due, scheduled, created_at, tags, depends, annotations)
            VALUES (?1, ?2, 'pending', ?3, ?4, ?5, ?6, ?7, ?8,?9, ?10, '[]')
        ",
        params![
            uuid,
            req.description,
            req.project,
            req.priority,
            urgency,
            req.due,
            req.scheduled,
            created_at,
            tags_json,
            depends_json
        ],
    )?;

    Ok(get_by_uuid(conn, &uuid)?.unwrap())
}

/// 更新任务
pub fn update(conn: &Connection, uuid: &str, req: &UpdateTaskRequest) -> Result<Task> {
    let current =
        get_by_uuid(conn, uuid)?.ok_or_else(|| anyhow::anyhow!("任务不存在：{}", uuid))?;

    let description = req.description.as_deref().unwrap_or(&current.description);

    let project = if req.clear_project {
        None
    } else {
        req.project.as_deref().or(current.project.as_deref())
    };

    let priority = if req.clear_priority {
        None
    } else {
        req.priority
            .as_deref()
            .or_else(|| current.priority.as_ref().map(Priority::as_str))
    };

    let due = if req.clear_due {
        None
    } else {
        req.due.as_deref().or(current.due.as_deref())
    };

    let scheduled = if req.clear_scheduled {
        None
    } else {
        req.scheduled.as_deref().or(current.scheduled.as_deref())
    };

    let tags = req.tags.as_ref().unwrap_or(&current.tags);
    let depends = req.depends.as_ref().unwrap_or(&current.depends);

    let tags_json = serde_json::to_string(tags)?;
    let depends_json = serde_json::to_string(depends)?;
    let urgency = compute_urgency(priority, due, &current.created_at, tags, depends);

    conn.execute(
        "UPDATE tasks SET
                description=?2, project=?3, priority=?4, urgency=?5,
                due=?6, scheduled=?7, tags=?8, depends=?9
             WHERE uuid=?1
        ",
        params![
            uuid,
            description,
            project,
            priority,
            urgency,
            due,
            scheduled,
            tags_json,
            depends_json
        ],
    )?;

    Ok(get_by_uuid(conn, uuid)?.unwrap())
}

/// 将任务标记为完成
pub fn mark_done(conn: &Connection, uuid: &str) -> Result<()> {
    let end = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE tasks SET status='completed', end=?2, urgency=0 WHERE uuid=?1",
        params![uuid, end],
    )?;

    Ok(())
}

/// 取消完成，将任务重新标记为待办，并重新计算 urgency
pub fn mark_pending(conn: &Connection, uuid: &str) -> Result<()> {
    let current = get_by_uuid(conn, uuid)?
        .ok_or_else(|| anyhow::anyhow!("task not found: {uuid}"))?;

    let urgency = compute_urgency(
        current.priority.as_ref().map(Priority::as_str),
        current.due.as_deref(),
        &current.created_at,
        &current.tags,
        &current.depends,
    );

    conn.execute(
        "UPDATE tasks SET status='pending', end=NULL, urgency=?2 WHERE uuid=?1",
        params![uuid, urgency],
    )?;

    Ok(())
}

/// 将任务标记为删除
pub fn mark_deleted(conn: &Connection, uuid: &str) -> Result<()> {
    let end = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE tasks SET status='deleted', end=?2, urgency=0 WHERE uuid=?1",
        params![uuid, end],
    )?;

    Ok(())
}

/// 将数据库行转换为 Task 结构体
fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    let tags_json: String = row.get(10)?;
    let depends_json: String = row.get(11)?;
    let annotations_json: String = row.get(12)?;

    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    let depends: Vec<String> = serde_json::from_str(&depends_json).unwrap_or_default();
    let annotations: Vec<Annotation> = serde_json::from_str(&annotations_json).unwrap_or_default();

    let status_str: String = row.get(2)?;
    let status: TaskStatus = match status_str.as_str() {
        "completed" => TaskStatus::Completed,
        "deleted" => TaskStatus::Deleted,
        "waiting" => TaskStatus::Waiting,
        _ => TaskStatus::Pending,
    };

    let priority_str: Option<String> = row.get(4)?;
    let priority = priority_str.as_deref().and_then(|p| match p {
        "H" => Some(Priority::High),
        "M" => Some(Priority::Medium),
        "L" => Some(Priority::Low),
        _ => None,
    });

    Ok(Task {
        uuid: row.get(0)?,
        description: row.get(1)?,
        status,
        project: row.get(3)?,
        priority,
        urgency: row.get(5)?,
        due: row.get(6)?,
        scheduled: row.get(7)?,
        created_at: row.get(8)?,
        end: row.get(9)?,
        tags,
        depends,
        annotations,
        blocking: Vec::new(),
        is_overdue: false,
        is_due_today: false,
        is_locked: false,
    })
}
