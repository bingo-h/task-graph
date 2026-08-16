//! 标签数据库操作
//!
//! 标签本身（名字 + 颜色）存在 tags 表，标签与任务的关联存在 task_tags 表（多对多）。
//! 任务侧仍然只暴露一个 `Vec<String>` 标签名列表（Task.tags），颜色等元信息
//! 单独通过 tags 表提供，和 projects 表 / GraphResponse.projects 是同一种拆分方式。

use crate::models::urgency::compute_urgency;
use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;

/// 一个标签及其元信息，供前端展示颜色和使用计数
#[derive(Serialize)]
pub struct TagInfo {
    pub name: String,
    pub color: Option<String>,
    /// 使用这个标签的任务数（不含已删除任务）
    pub task_count: i64,
}

/// 查询所有标签及各自的颜色、使用计数，按名字排序
pub fn list_all(conn: &Connection) -> Result<Vec<TagInfo>> {
    let mut stmt = conn.prepare(
        "SELECT t.name, t.color,
                (SELECT COUNT(*) FROM task_tags tt
                 JOIN tasks ON tasks.uuid = tt.task_uuid
                 WHERE tt.tag_name = t.name AND tasks.status != 'deleted') AS task_count
         FROM tags t
         ORDER BY t.name",
    )?;

    let tags = stmt
        .query_map([], |row| {
            Ok(TagInfo {
                name: row.get(0)?,
                color: row.get(1)?,
                task_count: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(tags)
}

/// 查询所有任务当前的标签列表：task_uuid -> 标签名数组（按名字排序）
pub fn tags_by_task(conn: &Connection) -> Result<HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare("SELECT task_uuid, tag_name FROM task_tags ORDER BY tag_name")?;

    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let rows =
        stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;

    for row in rows {
        let (task_uuid, tag_name) = row?;
        map.entry(task_uuid).or_default().push(tag_name);
    }

    Ok(map)
}

/// 查询单个任务当前的标签列表
pub fn tags_for_task(conn: &Connection, task_uuid: &str) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT tag_name FROM task_tags WHERE task_uuid = ?1 ORDER BY tag_name")?;
    let tags = stmt
        .query_map(params![task_uuid], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(tags)
}

/// 设置某个任务的完整标签集合（替换式）：新出现的标签自动在 tags 表登记（颜色留空），
/// 不在新集合里的旧关联会被移除；不会影响标签本身（颜色、是否还被其他任务使用）
pub fn set_tags_for_task(conn: &Connection, task_uuid: &str, tags: &[String]) -> Result<()> {
    conn.execute("DELETE FROM task_tags WHERE task_uuid = ?1", params![task_uuid])?;

    for tag in tags {
        conn.execute(
            "INSERT OR IGNORE INTO tags (name, color) VALUES (?1, NULL)",
            params![tag],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO task_tags (task_uuid, tag_name) VALUES (?1, ?2)",
            params![task_uuid, tag],
        )?;
    }

    Ok(())
}

/// 设置标签颜色（color 为 None 表示清空，前端会回退到默认颜色）
pub fn set_color(conn: &Connection, name: &str, color: Option<&str>) -> Result<()> {
    conn.execute("UPDATE tags SET color = ?2 WHERE name = ?1", params![name, color])?;
    Ok(())
}

/// 重命名标签；若目标名字已存在则合并（任务关联去重后并入目标标签，旧标签整个删除）。
/// 返回受影响的任务数。改名/合并不会让任何任务的标签集合从"有"变"无"，
/// 而 urgency 只关心标签集合是否为空（见 models::urgency），所以不需要重新计算 urgency。
pub fn rename(conn: &Connection, old_name: &str, new_name: &str) -> Result<usize> {
    let old_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tags WHERE name = ?1)",
        params![old_name],
        |row| row.get(0),
    )?;
    if !old_exists {
        return Ok(0);
    }

    let affected: i64 = conn.query_row(
        "SELECT COUNT(*) FROM task_tags WHERE tag_name = ?1",
        params![old_name],
        |row| row.get(0),
    )?;

    let new_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tags WHERE name = ?1)",
        params![new_name],
        |row| row.get(0),
    )?;

    if new_exists {
        // 目标标签已存在：合并——把 old_name 下的任务关联挪到 new_name 下（去重），旧标签整个删掉
        conn.execute(
            "INSERT OR IGNORE INTO task_tags (task_uuid, tag_name)
             SELECT task_uuid, ?2 FROM task_tags WHERE tag_name = ?1",
            params![old_name, new_name],
        )?;
        conn.execute("DELETE FROM task_tags WHERE tag_name = ?1", params![old_name])?;
        conn.execute("DELETE FROM tags WHERE name = ?1", params![old_name])?;
    } else {
        conn.execute(
            "UPDATE task_tags SET tag_name = ?2 WHERE tag_name = ?1",
            params![old_name, new_name],
        )?;
        conn.execute("UPDATE tags SET name = ?2 WHERE name = ?1", params![old_name, new_name])?;
    }

    Ok(affected as usize)
}

/// 彻底删除一个标签：解除它和所有任务的关联，并从 tags 表移除。
/// 这会让部分任务的标签集合变空，需要重新计算这些任务的 urgency。
pub fn delete(conn: &Connection, name: &str) -> Result<()> {
    let mut stmt = conn.prepare("SELECT task_uuid FROM task_tags WHERE tag_name = ?1")?;
    let affected_uuids: Vec<String> =
        stmt.query_map(params![name], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;

    conn.execute("DELETE FROM task_tags WHERE tag_name = ?1", params![name])?;
    conn.execute("DELETE FROM tags WHERE name = ?1", params![name])?;

    for uuid in affected_uuids {
        recompute_urgency(conn, &uuid)?;
    }

    Ok(())
}

/// 重新计算并写回单个任务的 urgency（标签集合变化后调用）
fn recompute_urgency(conn: &Connection, task_uuid: &str) -> Result<()> {
    let (priority, due, created_at, depends_json): (
        Option<String>,
        Option<String>,
        String,
        String,
    ) = conn.query_row(
        "SELECT priority, due, created_at, depends FROM tasks WHERE uuid = ?1",
        params![task_uuid],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let depends: Vec<String> = serde_json::from_str(&depends_json).unwrap_or_default();
    let tags = tags_for_task(conn, task_uuid)?;

    let urgency =
        compute_urgency(priority.as_deref(), due.as_deref(), &created_at, &tags, &depends);

    conn.execute("UPDATE tasks SET urgency = ?2 WHERE uuid = ?1", params![task_uuid, urgency])?;

    Ok(())
}

/// 一次性从旧的 tasks.tags JSON 列迁移到 tags / task_tags 表（schema 版本 9 引入）
pub fn backfill_from_json(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT uuid, tags FROM tasks")?;
    let rows: Vec<(String, String)> =
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;

    for (uuid, tags_json) in rows {
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        for tag in tags {
            conn.execute(
                "INSERT OR IGNORE INTO tags (name, color) VALUES (?1, NULL)",
                params![tag],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO task_tags (task_uuid, tag_name) VALUES (?1, ?2)",
                params![uuid, tag],
            )?;
        }
    }

    Ok(())
}
