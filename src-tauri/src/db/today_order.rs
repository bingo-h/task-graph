//! "今日任务"视图下的手动排序边（独立于真实的 depends 依赖图）

use anyhow::Result;
use rusqlite::{params, Connection};

/// 全部手动排序边，(from_uuid, to_uuid)：from 应先于 to 完成
pub fn list_all(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT from_uuid, to_uuid FROM today_order_edges")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn add_edge(conn: &Connection, from_uuid: &str, to_uuid: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO today_order_edges (from_uuid, to_uuid, created_at)
         VALUES (?1, ?2, ?3)",
        params![from_uuid, to_uuid, now],
    )?;
    Ok(())
}

pub fn remove_edge(conn: &Connection, from_uuid: &str, to_uuid: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM today_order_edges WHERE from_uuid=?1 AND to_uuid=?2",
        params![from_uuid, to_uuid],
    )?;
    Ok(())
}

/// 任务被删除时清理跟它相关的手动排序边，避免留下指向已删除任务的孤儿行
pub fn prune_for_task(conn: &Connection, uuid: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM today_order_edges WHERE from_uuid=?1 OR to_uuid=?1",
        params![uuid],
    )?;
    Ok(())
}
