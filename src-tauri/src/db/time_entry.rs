//! 任务计时记录数据库操作
//!
//! 全局同一时刻只允许一个任务处于计时中：开始新的计时前，
//! 会先自动结束当前正在进行的计时段（如果存在）。

use crate::models::time_entry::TimeEntry;
use anyhow::Result;
use rusqlite::{params, Connection};

/// 结束当前正在进行的计时段（如果存在）
pub fn stop_active(conn: &Connection) -> Result<()> {
    let end = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE time_entries SET end=?1 WHERE end IS NULL",
        params![end],
    )?;
    Ok(())
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

/// 查询某任务的所有计时记录，按开始时间倒序
pub fn list_by_task(conn: &Connection, task_uuid: &str) -> Result<Vec<TimeEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_uuid, start, end FROM time_entries
         WHERE task_uuid = ?1 ORDER BY start DESC",
    )?;

    let entries = stmt
        .query_map(params![task_uuid], row_to_entry)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(entries)
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
    })
}
