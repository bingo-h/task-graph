//! DAG 视图里、同一 dagre rank 列内任务节点的手动纵向排序（独立于 depends 依赖图和 today_order_edges）

use anyhow::Result;
use rusqlite::{params, Connection};

/// 全部手动排序边，(from_uuid, to_uuid)：from 排在 to 上面
pub fn list_all(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT from_uuid, to_uuid FROM sibling_order_edges")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 把 `uuids` 这一整列节点的顺序整体替换成给定顺序：先删掉所有牵涉到这些节点的旧边，
/// 再按给定顺序插入一条新链（相邻两两连接）。前端每次拖拽落定后传来的都是这一列
/// 当前可见节点的完整新顺序，所以用"整体替换"而不是增删单条边，逻辑更简单也不会留下
/// 跟新顺序矛盾的旧边。
pub fn replace_chain(conn: &Connection, uuids: &[String]) -> Result<()> {
    if uuids.len() < 2 {
        return Ok(());
    }

    let placeholders = uuids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let del_sql = format!(
        "DELETE FROM sibling_order_edges WHERE from_uuid IN ({placeholders}) OR to_uuid IN ({placeholders})"
    );
    let params: Vec<&dyn rusqlite::ToSql> =
        uuids.iter().chain(uuids.iter()).map(|u| u as &dyn rusqlite::ToSql).collect();
    conn.execute(&del_sql, params.as_slice())?;

    let now = chrono::Utc::now().to_rfc3339();
    for pair in uuids.windows(2) {
        conn.execute(
            "INSERT OR IGNORE INTO sibling_order_edges (from_uuid, to_uuid, created_at)
             VALUES (?1, ?2, ?3)",
            params![pair[0], pair[1], now],
        )?;
    }

    Ok(())
}

/// 任务被删除时清理跟它相关的手动排序边，避免留下指向已删除任务的孤儿行
pub fn prune_for_task(conn: &Connection, uuid: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM sibling_order_edges WHERE from_uuid=?1 OR to_uuid=?1",
        params![uuid],
    )?;
    Ok(())
}
