//! 任务数据库操作
//!
//! 所有函数接受 &Connection ，由调用方管理连接生命周期
//! urgency 在每次写操作后自动重新计算
//!
//! 标签不再存在 tasks 表里，而是拆到独立的 tags / task_tags 表（见 db::tag），
//! 这里的 Task.tags 是每次查询时现拼出来的标签名列表。

use crate::db::tag;
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
    /// 备注：非空时作为任务唯一的一条 annotation
    pub annotation: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub recur_rule: Option<crate::models::task::RecurRule>,
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
    /// 备注：非空时整体替换原有的那一条 annotation；为空且 clear_annotation 为 false 时不改动
    pub annotation: Option<String>,
    pub clear_annotation: bool,
    pub icon: Option<String>,
    pub clear_icon: bool,
    pub color: Option<String>,
    pub clear_color: bool,
}

/// 查询所有非删除状态的任务
pub fn list_all(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT uuid, description, status, project, priority, urgency,
             due, scheduled, created_at, end, depends, annotations,
             today_marked_date, icon, color, recur_rule
        FROM tasks WHERE status != 'deleted' ORDER BY urgency DESC",
    )?;

    let mut tasks = stmt
        .query_map([], row_to_task)?
        .collect::<Result<Vec<_>, _>>()?;

    // 标签存在独立的 task_tags 表里，一次性查出来再按 uuid 分发，
    // 避免每个任务都单独查一次
    let mut tags_by_task = tag::tags_by_task(conn)?;
    for task in tasks.iter_mut() {
        task.tags = tags_by_task.remove(&task.uuid).unwrap_or_default();
    }

    Ok(tasks)
}

/// 按 UUID 查询单个任务
pub fn get_by_uuid(conn: &Connection, uuid: &str) -> Result<Option<Task>> {
    let mut stmt = conn.prepare(
        "SELECT uuid, description, status, project, priority, urgency,
                    due, scheduled, created_at, end, depends, annotations,
                    today_marked_date, icon, color, recur_rule
              FROM tasks WHERE uuid = ?1",
    )?;

    let mut rows = stmt.query_map([uuid], row_to_task)?;
    let mut task = match rows.next().transpose()? {
        Some(t) => t,
        None => return Ok(None),
    };

    task.tags = tag::tags_for_task(conn, uuid)?;

    Ok(Some(task))
}

/// 设置/取消"今日任务"标记，标记时记录当前本地日期
///
/// 这里特意用本地时区而不是 UTC：跟这个应用别处的"今天"（截止日期、重复周期）
/// 不一样，"今日任务"是直接给人看的"今天"概念，要跟着用户本地日历天走，
/// 否则时区偏移较大的用户会在本地已经跨天之后，这个标记还因为 UTC 没跨天
/// 而一直没清掉。
pub fn set_today(conn: &Connection, uuid: &str, marked: bool) -> Result<()> {
    let date = if marked {
        Some(chrono::Local::now().format("%Y-%m-%d").to_string())
    } else {
        None
    };

    conn.execute(
        "UPDATE tasks SET today_marked_date = ?2 WHERE uuid = ?1",
        params![uuid, date],
    )?;

    Ok(())
}

/// 清除已经跨天的"今日任务"标记（不等于今天的 today_marked_date 一律清空）
pub fn reset_stale_today_marks(conn: &Connection) -> Result<()> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    conn.execute(
        "UPDATE tasks SET today_marked_date = NULL
             WHERE today_marked_date IS NOT NULL AND today_marked_date != ?1",
        params![today],
    )?;

    Ok(())
}

/// 创建新任务，返回创建后的任务结构体
pub fn create(conn: &Connection, req: &CreateTaskRequest) -> Result<Task> {
    let uuid = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let depends_json = serde_json::to_string(&req.depends)?;
    let urgency = compute_urgency(
        req.priority.as_deref(),
        req.due.as_deref(),
        &created_at,
        &req.tags,
        &req.depends,
    );

    let annotations = match req.annotation.as_deref().map(str::trim) {
        Some(text) if !text.is_empty() => vec![Annotation {
            created_at: Some(created_at.clone()),
            description: text.to_string(),
        }],
        _ => Vec::new(),
    };
    let annotations_json = serde_json::to_string(&annotations)?;

    conn.execute(
        "
            INSERT INTO tasks
                (uuid, description, status, project, priority, urgency,
                 due, scheduled, created_at, depends, annotations, icon, color)
            VALUES (?1, ?2, 'pending', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
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
            depends_json,
            annotations_json,
            req.icon,
            req.color,
        ],
    )?;

    tag::set_tags_for_task(conn, &uuid, &req.tags)?;

    // 新建时就带了周期性规则：立刻算出第一个周期（复用和 set_task_recur 命令同一套逻辑），
    // 这样表单一次提交就能直接建出一个"正在进行中"的周期性任务，不用创建后再补一次调用
    if let Some(rule) = &req.recur_rule {
        crate::db::recur::set_recur_rule(conn, &uuid, Some(rule.clone()))?;
    }

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

    let icon = if req.clear_icon {
        None
    } else {
        req.icon.as_deref().or(current.icon.as_deref())
    };

    let color = if req.clear_color {
        None
    } else {
        req.color.as_deref().or(current.color.as_deref())
    };

    let depends_json = serde_json::to_string(depends)?;
    let urgency = compute_urgency(priority, due, &current.created_at, tags, depends);

    // 备注只保留一条，每次都是整体替换（而不是追加）；
    // 没提供 annotation 时不改动，只有显式 clear_annotation 才会清空
    let trimmed_annotation = req.annotation.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let annotations = if let Some(text) = trimmed_annotation {
        vec![Annotation {
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            description: text.to_string(),
        }]
    } else if req.clear_annotation {
        Vec::new()
    } else {
        current.annotations.clone()
    };
    let annotations_json = serde_json::to_string(&annotations)?;

    conn.execute(
        "UPDATE tasks SET
                description=?2, project=?3, priority=?4, urgency=?5,
                due=?6, scheduled=?7, depends=?8, annotations=?9,
                icon=?10, color=?11
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
            depends_json,
            annotations_json,
            icon,
            color,
        ],
    )?;

    // 只有明确传了 tags 才改动标签关联，没传就保持原样
    if let Some(new_tags) = &req.tags {
        tag::set_tags_for_task(conn, uuid, new_tags)?;
    }

    Ok(get_by_uuid(conn, uuid)?.unwrap())
}

/// 只替换某个任务的 depends 字段，其余字段不变
/// （拖拽图谱里已有连线的终点、改指向/删除依赖关系时用）
pub fn set_depends(conn: &Connection, uuid: &str, depends: Vec<String>) -> Result<()> {
    update(
        conn,
        uuid,
        &UpdateTaskRequest {
            description: None,
            project: None,
            priority: None,
            due: None,
            scheduled: None,
            tags: None,
            depends: Some(depends),
            clear_project: false,
            clear_priority: false,
            clear_due: false,
            clear_scheduled: false,
            annotation: None,
            clear_annotation: false,
            icon: None,
            clear_icon: false,
            color: None,
            clear_color: false,
        },
    )?;
    Ok(())
}

/// 只替换某个任务的 project 字段，其余字段不变（批量转移项目用）
/// project 传 None 表示移到"无项目"（Inbox）
pub fn set_project(conn: &Connection, uuid: &str, project: Option<String>) -> Result<()> {
    let clear_project = project.is_none();
    update(
        conn,
        uuid,
        &UpdateTaskRequest {
            description: None,
            project,
            priority: None,
            due: None,
            scheduled: None,
            tags: None,
            depends: None,
            clear_project,
            clear_priority: false,
            clear_due: false,
            clear_scheduled: false,
            annotation: None,
            clear_annotation: false,
            icon: None,
            clear_icon: false,
            color: None,
            clear_color: false,
        },
    )?;
    Ok(())
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

    // 删除任务后，跟它相关的手动排序边（今日任务顺序 + DAG 同层纵向顺序）就没有意义了，
    // 一并清理，避免留下指向已删除任务的孤儿行
    crate::db::today_order::prune_for_task(conn, uuid)?;
    crate::db::sibling_order::prune_for_task(conn, uuid)?;

    Ok(())
}

/// 将数据库行转换为 Task 结构体
/// 注意：不包含 tags —— 标签存在独立的 task_tags 表里，调用方查完之后自行填充
fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    let depends_json: String = row.get(10)?;
    let annotations_json: String = row.get(11)?;

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

    let recur_rule_json: Option<String> = row.get(15)?;
    let recur_rule: Option<crate::models::task::RecurRule> = recur_rule_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

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
        tags: Vec::new(),
        depends,
        annotations,
        today_marked_date: row.get(12)?,
        blocking: Vec::new(),
        is_overdue: false,
        is_due_today: false,
        is_locked: false,
        is_today: false,
        total_seconds: 0,
        is_timing: false,
        active_since: None,
        icon: row.get(13)?,
        color: row.get(14)?,
        is_recurring: recur_rule.is_some(),
        recur_rule,
    })
}
