//! 任务计时记录数据库操作
//!
//! 全局同一时刻只允许一个任务处于计时中：开始新的计时前，
//! 会先自动结束当前正在进行的计时段（如果存在）。

use crate::models::time_entry::TimeEntry;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

/// 结束当前正在进行的计时段（如果存在），返回被结束的那条记录的 id，
/// 供调用方在停止计时后弹窗询问这段专注的回忆总结
pub fn stop_active(conn: &Connection) -> Result<Option<i64>> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM time_entries WHERE end IS NULL",
            [],
            |row| row.get(0),
        )
        .optional()?;

    if id.is_some() {
        let end = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE time_entries SET end=?1 WHERE end IS NULL",
            params![end],
        )?;
    }

    Ok(id)
}

/// 为指定任务开始计时，先结束其他正在进行的计时段
pub fn start(conn: &Connection, task_uuid: &str) -> Result<()> {
    stop_active(conn)?;

    let start = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO time_entries (task_uuid, start, end) VALUES (?1, ?2, NULL)",
        params![task_uuid, start],
    )?;

    Ok(())
}

/// 查询所有任务的全部计时记录，按开始时间倒序（用于首页仪表盘按天聚合）
pub fn list_all(conn: &Connection) -> Result<Vec<TimeEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_uuid, start, end, note_title, note_body
         FROM time_entries ORDER BY start DESC",
    )?;

    let entries = stmt
        .query_map([], row_to_entry)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(entries)
}

/// 查询某任务的所有计时记录，按开始时间倒序
pub fn list_by_task(conn: &Connection, task_uuid: &str) -> Result<Vec<TimeEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_uuid, start, end, note_title, note_body FROM time_entries
         WHERE task_uuid = ?1 ORDER BY start DESC",
    )?;

    let entries = stmt
        .query_map(params![task_uuid], row_to_entry)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(entries)
}

/// 保存某条计时记录的回忆总结（标题 + 正文），事后可反复修改
pub fn save_note(conn: &Connection, id: i64, title: Option<&str>, body: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE time_entries SET note_title = ?2, note_body = ?3 WHERE id = ?1",
        params![id, title, body],
    )?;
    Ok(())
}

/// 删除某条计时记录（不可恢复）
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM time_entries WHERE id = ?1", params![id])?;
    Ok(())
}

/// 查询所有任务的累计计时秒数（含正在进行中的计时段），
/// 顺带取出当前正在计时的 task_uuid 及这一段的开始时间（用于悬浮状态栏显示本次专注时长），
/// 复用同一次对 time_entries 的扫描，不再额外查询
/// 返回 (task_uuid -> total_seconds, 当前计时中的 task_uuid, 当前这一段的开始时间)
pub fn totals_by_task(
    conn: &Connection,
) -> Result<(
    std::collections::HashMap<String, i64>,
    Option<String>,
    Option<String>,
)> {
    let mut stmt = conn.prepare("SELECT task_uuid, start, end FROM time_entries")?;
    let now = chrono::Utc::now();

    let mut totals = std::collections::HashMap::new();
    let mut active_task: Option<String> = None;
    let mut active_since: Option<String> = None;

    let rows = stmt.query_map([], |row| {
        let task_uuid: String = row.get(0)?;
        let start: String = row.get(1)?;
        let end: Option<String> = row.get(2)?;
        Ok((task_uuid, start, end))
    })?;

    for row in rows {
        let (task_uuid, start, end) = row?;

        let start_dt = chrono::DateTime::parse_from_rfc3339(&start)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or(now);

        let end_dt = match &end {
            Some(e) => chrono::DateTime::parse_from_rfc3339(e)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or(now),
            None => {
                active_task = Some(task_uuid.clone());
                active_since = Some(start.clone());
                now
            }
        };

        let seconds = (end_dt - start_dt).num_seconds().max(0);
        *totals.entry(task_uuid).or_insert(0) += seconds;
    }

    Ok((totals, active_task, active_since))
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<TimeEntry> {
    Ok(TimeEntry {
        id: row.get(0)?,
        task_uuid: row.get(1)?,
        start: row.get(2)?,
        end: row.get(3)?,
        note_title: row.get(4)?,
        note_body: row.get(5)?,
    })
}
