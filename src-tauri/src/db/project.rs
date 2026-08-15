//! 项目的持久化存储与项目树构建

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};

use crate::models::project::{ProjectNode, STAGE_ACTIVE, STAGE_PLANNED};
use crate::models::task::Task;

/// 无项目归属任务的虚拟项目路径标识符。
const INBOX_PROJECT: &str = "无项目";

/// 显式创建的项目记录
pub struct ProjectRecord {
    pub path: String,
    pub archived: bool,
    pub stage: String,
    pub trashed: bool,
    pub trashed_at: Option<String>,
}

/// 显式创建一个项目，路径已存在时忽略
pub fn create(conn: &Connection, path: &str, stage: &str) -> Result<()> {
    let created_at = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR IGNORE INTO projects (path, created_at, stage) VALUES (?1, ?2, ?3)",
        params![path, created_at, stage],
    )?;

    Ok(())
}

/// 查询所有显式创建的项目记录
pub fn list_all(conn: &Connection) -> Result<Vec<ProjectRecord>> {
    let mut stmt =
        conn.prepare("SELECT path, archived, stage, trashed, trashed_at FROM projects")?;

    let records = stmt
        .query_map([], |row| {
            Ok(ProjectRecord {
                path: row.get(0)?,
                archived: row.get::<_, i64>(1)? != 0,
                stage: row.get(2)?,
                trashed: row.get::<_, i64>(3)? != 0,
                trashed_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(records)
}

/// 设置项目的归档状态。项目此前未显式创建过（仅由任务隐式产生）时自动补建记录。
pub fn set_archived(conn: &Connection, path: &str, archived: bool) -> Result<()> {
    let created_at = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO projects (path, created_at, archived, stage) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET archived = excluded.archived",
        params![path, created_at, archived as i64, STAGE_ACTIVE],
    )?;

    Ok(())
}

/// 设置项目所属阶段（计划中 / 进行中）。项目此前未显式创建过时自动补建记录。
pub fn set_stage(conn: &Connection, path: &str, stage: &str) -> Result<()> {
    let created_at = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO projects (path, created_at, archived, stage) VALUES (?1, ?2, 0, ?3)
             ON CONFLICT(path) DO UPDATE SET stage = excluded.stage",
        params![path, created_at, stage],
    )?;

    Ok(())
}

/// 将项目移入废纸篓（软删除，可恢复）。任务和归档/阶段状态都原样保留，
/// 只是在树中被划到"废纸篓"分组；级联到所有子项目。
pub fn trash(conn: &Connection, path: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO projects (path, created_at, stage, trashed, trashed_at)
             VALUES (?1, ?2, ?3, 1, ?2)
             ON CONFLICT(path) DO UPDATE SET trashed = 1, trashed_at = excluded.trashed_at",
        params![path, now, STAGE_ACTIVE],
    )?;

    Ok(())
}

/// 从废纸篓恢复项目（仅对自身被显式移入废纸篓的路径有效）
pub fn restore(conn: &Connection, path: &str) -> Result<()> {
    conn.execute(
        "UPDATE projects SET trashed = 0, trashed_at = NULL WHERE path = ?1",
        params![path],
    )?;

    Ok(())
}

/// 彻底删除一个项目：级联删除该项目及其所有子项目下的任务（软删除任务本身），
/// 并移除项目记录。用于废纸篓中的"彻底删除"，以及到期自动清理。
pub fn purge(conn: &Connection, path: &str) -> Result<()> {
    let end = chrono::Utc::now().to_rfc3339();
    let prefix = format!("{path}.%");

    conn.execute(
        "UPDATE tasks SET status='deleted', end=?3, urgency=0
             WHERE status != 'deleted' AND (project = ?1 OR project LIKE ?2)",
        params![path, prefix, end],
    )?;

    conn.execute(
        "DELETE FROM projects WHERE path = ?1 OR path LIKE ?2",
        params![path, prefix],
    )?;

    Ok(())
}

/// 清理废纸篓中已超过保留期限的项目，返回被彻底删除的项目路径列表。
/// `retention_days` 为 0 表示关闭自动清理。
pub fn purge_expired(conn: &Connection, retention_days: u32) -> Result<Vec<String>> {
    if retention_days == 0 {
        return Ok(Vec::new());
    }

    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);

    let mut stmt =
        conn.prepare("SELECT path, trashed_at FROM projects WHERE trashed = 1 AND trashed_at IS NOT NULL")?;
    let expired: Vec<String> = stmt
        .query_map([], |row| {
            let path: String = row.get(0)?;
            let trashed_at: String = row.get(1)?;
            Ok((path, trashed_at))
        })?
        .filter_map(|r| r.ok())
        .filter_map(|(path, trashed_at)| {
            let ts = chrono::DateTime::parse_from_rfc3339(&trashed_at).ok()?;
            (ts < cutoff).then_some(path)
        })
        .collect();

    for path in &expired {
        purge(conn, path)?;
    }

    Ok(expired)
}

/// 将项目（及其所有子项目、其下任务）移动到新的父项目下；`new_parent` 为
/// `None` 时移动到顶层。通过整体重写路径前缀实现，同时更新任务的 project
/// 字段和 projects 表中的路径记录。
pub fn move_project(conn: &Connection, path: &str, new_parent: Option<&str>) -> Result<()> {
    let leaf = path
        .rsplit('.')
        .next()
        .ok_or_else(|| anyhow!("无效的项目路径"))?;

    let new_path = match new_parent {
        Some(p) if !p.is_empty() => format!("{p}.{leaf}"),
        _ => leaf.to_string(),
    };

    if new_path == path {
        return Ok(());
    }

    if let Some(p) = new_parent {
        if p == path || p.starts_with(&format!("{path}.")) {
            return Err(anyhow!("不能把项目移动到自己或自己的子项目下"));
        }
    }

    // 冲突检测：目标路径不能与既有项目（排除正在移动的这一整棵子树）重名
    let existing = list_existing_project_paths(conn)?;
    let moving_prefix = format!("{path}.");
    let conflict = existing
        .iter()
        .any(|p| p != path && !p.starts_with(&moving_prefix) && p == &new_path);
    if conflict {
        return Err(anyhow!("目标位置已存在同名项目：{new_path}"));
    }

    let old_prefix = format!("{path}.");
    let new_prefix = format!("{new_path}.");
    let old_prefix_len = (old_prefix.len() + 1) as i64; // SQLite substr 从 1 开始

    conn.execute(
        "UPDATE tasks SET project = ?2 WHERE project = ?1",
        params![path, new_path],
    )?;
    conn.execute(
        "UPDATE tasks SET project = ?3 || substr(project, ?4)
             WHERE project LIKE ?2",
        params![path, format!("{old_prefix}%"), new_prefix, old_prefix_len],
    )?;

    conn.execute(
        "UPDATE projects SET path = ?2 WHERE path = ?1",
        params![path, new_path],
    )?;
    conn.execute(
        "UPDATE projects SET path = ?3 || substr(path, ?4)
             WHERE path LIKE ?2",
        params![path, format!("{old_prefix}%"), new_prefix, old_prefix_len],
    )?;

    Ok(())
}

/// 收集当前所有出现过的项目路径（来自任务的 project 字段与 projects 表）
fn list_existing_project_paths(conn: &Connection) -> Result<HashSet<String>> {
    let mut set = HashSet::new();

    let mut stmt = conn.prepare("SELECT DISTINCT project FROM tasks WHERE project IS NOT NULL")?;
    for row in stmt.query_map([], |row| row.get::<_, String>(0))? {
        set.insert(row?);
    }

    let mut stmt = conn.prepare("SELECT path FROM projects")?;
    for row in stmt.query_map([], |row| row.get::<_, String>(0))? {
        set.insert(row?);
    }

    Ok(set)
}

/// 判断路径自身或其任意祖先路径是否在给定集合中
fn matches_self_or_ancestor(path: &str, set: &HashSet<String>) -> bool {
    let parts: Vec<&str> = path.split('.').collect();

    (1..=parts.len()).any(|i| set.contains(&parts[..i].join(".")))
}

/// 项目所属的显示分组：废纸篓优先级最高（级联），其次是归档（级联）；
/// 都不是的话按阶段（计划中/进行中）分组，阶段固定继承顶层项目，同样是级联的。
fn group_of(node: &ProjectNode) -> &str {
    if node.trashed {
        "trash"
    } else if node.archived {
        "archived"
    } else {
        node.stage.as_str()
    }
}

/// 从任务列表和显式创建的项目记录构建项目树
///
/// 返回 (节点字典, 计划中根节点列表, 进行中根节点列表, 已归档根节点列表, 废纸篓根节点列表)。
/// 每个项目都必须落在四组之一；当某节点与其父节点分组不同（例如子项目自身被
/// 单独移入废纸篓/归档）时，该节点会从父节点的 children 中摘出，作为所属分组
/// 自己的根节点。阶段（计划中/进行中）只能在顶层项目上设置且固定继承给全部
/// 子项目，因此阶段本身不会产生这种分组边界。
pub fn build(
    tasks: &[Task],
    project_records: &[ProjectRecord],
) -> (
    HashMap<String, ProjectNode>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
) {
    let mut nodes: HashMap<String, ProjectNode> = HashMap::new();
    let archived_set: HashSet<String> = project_records
        .iter()
        .filter(|r| r.archived)
        .map(|r| r.path.clone())
        .collect();
    let trashed_map: HashMap<&str, &str> = project_records
        .iter()
        .filter(|r| r.trashed)
        .filter_map(|r| r.trashed_at.as_deref().map(|t| (r.path.as_str(), t)))
        .collect();
    let trashed_set: HashSet<String> = trashed_map.keys().map(|k| k.to_string()).collect();
    let stage_map: HashMap<&str, &str> = project_records
        .iter()
        .map(|r| (r.path.as_str(), r.stage.as_str()))
        .collect();

    // 为一个项目路径的每个前缀段创建节点
    let insert_path = |nodes: &mut HashMap<String, ProjectNode>, project: &str| {
        let parts: Vec<&str> = project.split('.').collect();

        for i in 1..=parts.len() {
            let path = parts[..i].join(".");

            nodes
                .entry(path.clone())
                .or_insert_with(|| ProjectNode::new(path.clone(), parts[i - 1].to_string(), i - 1));
        }
    };

    // Step 1: 为每个任务所属的路径段创建节点
    for task in tasks {
        let Some(project) = &task.project else {
            continue;
        };
        insert_path(&mut nodes, project);
    }

    // Step 1.5: 为显式创建（可能尚无任务）的项目路径创建节点
    for record in project_records {
        insert_path(&mut nodes, &record.path);
    }

    // Step 1.6: 标记每个节点的归档/废纸篓状态和阶段（未显式设置过的默认视为"进行中"）
    // 阶段（计划中/进行中）只在顶层项目上设置，子项目一律继承顶层项目的阶段，
    // 不能单独移动，因此这里只查顶层路径段，忽略子项目自身可能存在的旧记录。
    let paths: Vec<String> = nodes.keys().cloned().collect();
    for path in &paths {
        if let Some(node) = nodes.get_mut(path) {
            node.self_archived = archived_set.contains(path);
            node.archived = matches_self_or_ancestor(path, &archived_set);
            node.self_trashed = trashed_set.contains(path);
            node.trashed = matches_self_or_ancestor(path, &trashed_set);
            node.trashed_at = trashed_map.get(path.as_str()).map(|s| s.to_string());

            let root = path.split('.').next().unwrap_or(path.as_str());
            node.stage = stage_map
                .get(root)
                .copied()
                .unwrap_or(STAGE_ACTIVE)
                .to_string();
        }
    }

    // Step 2: 连接父子关系（跨越分组边界的节点不挂到父节点下，
    // 而是作为所属分组自己的根节点）
    for path in &paths {
        let parts: Vec<&str> = path.split('.').collect();

        if parts.len() < 2 {
            continue;
        }

        let parent = parts[..parts.len() - 1].join(".");
        let is_boundary = group_of(&nodes[path]) != group_of(&nodes[&parent]);
        if is_boundary {
            continue;
        }

        if let Some(parent_node) = nodes.get_mut(&parent) {
            if !parent_node.children.contains(path) {
                parent_node.children.push(path.clone());
            }
        }
    }

    // Step 3: 聚合任务计数
    for task in tasks {
        let Some(project) = &task.project else {
            continue;
        };

        if let Some(node) = nodes.get_mut(project) {
            match task.status.as_str() {
                "pending" => {
                    node.pending_count += 1;

                    if task.is_overdue {
                        node.overdue_count += 1;
                    }
                    if task.is_locked {
                        node.locked_count += 1;
                    }
                }

                "completed" => node.completed_count += 1,

                "waiting" => node.waiting_count += 1,

                _ => {}
            }
        }
    }

    // Step 4: 从深到浅向上传播计数，但不跨越分组边界
    // （不同分组子树的任务计数不应汇总进另一分组的父项目）
    let mut sorted: Vec<String> = nodes.keys().cloned().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.chars().filter(|&c| c == '.').count()));
    for path in &sorted {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() < 2 {
            continue;
        }

        let parent = parts[..parts.len() - 1].join(".");
        if group_of(&nodes[path]) != group_of(&nodes[&parent]) {
            continue;
        }

        let (p, c, w, o, l) = {
            let n = &nodes[path];
            (
                n.pending_count,
                n.completed_count,
                n.waiting_count,
                n.overdue_count,
                n.locked_count,
            )
        };

        if let Some(pn) = nodes.get_mut(&parent) {
            pn.pending_count += p;
            pn.completed_count += c;
            pn.waiting_count += w;
            pn.overdue_count += o;
            pn.locked_count += l;
        }
    }

    // Step 5: 收集根节点，按所属分组分为四组
    // 某分组的根节点 = 自身属于该分组，且没有父节点或父节点属于另一分组
    let mut planned_roots: Vec<String> = Vec::new();
    let mut active_roots: Vec<String> = Vec::new();
    let mut archived_roots: Vec<String> = Vec::new();
    let mut trash_roots: Vec<String> = Vec::new();
    for path in &paths {
        let node = &nodes[path];
        let parts: Vec<&str> = path.split('.').collect();
        let parent_group: Option<String> = if parts.len() < 2 {
            None
        } else {
            let parent = parts[..parts.len() - 1].join(".");
            Some(group_of(&nodes[&parent]).to_string())
        };

        let own_group = group_of(node);
        let is_root = parent_group.as_deref() != Some(own_group);
        if !is_root {
            continue;
        }

        match own_group {
            "trash" => trash_roots.push(path.clone()),
            "archived" => archived_roots.push(path.clone()),
            STAGE_PLANNED => planned_roots.push(path.clone()),
            _ => active_roots.push(path.clone()),
        }
    }
    planned_roots.sort();
    active_roots.sort();
    archived_roots.sort();
    trash_roots.sort();

    // Step 6: 无项目归属任务的虚拟节点（始终归入"进行中"分组）
    let inbox: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.project.is_none() && t.status.as_str() != "deleted")
        .collect();

    if !inbox.is_empty() {
        let mut inbox_node = ProjectNode::new(INBOX_PROJECT.to_string(), "无项目".to_string(), 0);
        inbox_node.stage = STAGE_ACTIVE.to_string();

        for t in &inbox {
            match t.status.as_str() {
                "pending" => {
                    inbox_node.pending_count += 1;
                    if t.is_overdue {
                        inbox_node.overdue_count += 1;
                    }
                    if t.is_locked {
                        inbox_node.locked_count += 1;
                    }
                }

                "completed" => inbox_node.completed_count += 1,

                "waiting" => inbox_node.waiting_count += 1,

                _ => {}
            }
        }

        nodes.insert(INBOX_PROJECT.to_string(), inbox_node);
        active_roots.insert(0, INBOX_PROJECT.to_string());
    }

    (nodes, planned_roots, active_roots, archived_roots, trash_roots)
}
