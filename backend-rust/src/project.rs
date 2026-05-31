//! 项目树构建与统计聚合。
//!
//! 从任务列表构建层级项目树，并将任务计数从叶节点向上聚合到根节点。
//! 逻辑与 task-tui 的 project.rs 相同，针对 Web API 做了序列化适配。

use std::collections::HashMap;
use serde::Serialize;
use crate::task::TaskView;
use crate::constants::INBOX_PROJECT;

/// 项目树节点，序列化后直接作为 API 响应的一部分。
#[derive(Debug, Clone, Serialize)]
pub struct ProjectNode {
    /// 完整的点分隔路径，例如 `"work.backend"`
    pub path: String,
    /// 显示名称（最后一段），例如 `"backend"`
    pub name: String,
    /// 在树中的深度（根节点为 0）
    pub depth: usize,
    /// 直接子节点的完整路径列表
    pub children: Vec<String>,

    // ── 任务计数（含所有后代节点）────────────────────────────────────────────
    pub pending_count:   usize,
    pub completed_count: usize,
    pub waiting_count:   usize,
    /// 逾期任务数量
    pub overdue_count:   usize,
    /// 被锁定任务数量（有未完成前置任务）
    pub locked_count:    usize,
}

impl ProjectNode {
    /// 创建一个空节点（计数均为 0）。
    fn new(path: &str) -> Self {
        let name  = path.split('.').last().unwrap_or(path).to_string();
        let depth = path.chars().filter(|&c| c == '.').count();
        ProjectNode {
            path:            path.to_string(),
            name,
            depth,
            children:        Vec::new(),
            pending_count:   0,
            completed_count: 0,
            waiting_count:   0,
            overdue_count:   0,
            locked_count:    0,
        }
    }
}

/// 完整的项目树，作为 API 响应的一部分返回给前端。
#[derive(Debug, Serialize)]
pub struct ProjectTree {
    /// 所有节点，key 为完整路径
    pub nodes: HashMap<String, ProjectNode>,
    /// 根级项目路径列表（排序后，收件箱排在最前）
    pub roots: Vec<String>,
}

impl ProjectTree {
    /// 从任务视图列表构建项目树。
    ///
    /// 构建步骤：
    /// 1. 为每个路径段创建节点
    /// 2. 连接父子关系
    /// 3. 将任务计数聚合到精确匹配的项目节点
    /// 4. 从叶节点向上传播计数
    /// 5. 收集根节点，处理"无项目"虚拟节点
    pub fn build(tasks: &[TaskView]) -> Self {
        let mut nodes: HashMap<String, ProjectNode> = HashMap::new();

        // ── 步骤 1：为每个路径段创建节点 ─────────────────────────────────────
        for task in tasks {
            let Some(project) = &task.project else { continue };
            if task.status == "deleted" { continue; }
            let parts: Vec<&str> = project.split('.').collect();
            for i in 1..=parts.len() {
                let path = parts[..i].join(".");
                nodes.entry(path.clone()).or_insert_with(|| ProjectNode::new(&path));
            }
        }

        // ── 步骤 2：连接父子关系 ──────────────────────────────────────────────
        let paths: Vec<String> = nodes.keys().cloned().collect();
        for path in &paths {
            let parts: Vec<&str> = path.split('.').collect();
            if parts.len() < 2 { continue; }
            let parent = parts[..parts.len() - 1].join(".");
            if let Some(parent_node) = nodes.get_mut(&parent) {
                if !parent_node.children.contains(path) {
                    parent_node.children.push(path.clone());
                }
            }
        }
        for node in nodes.values_mut() {
            node.children.sort();
        }

        // ── 步骤 3：聚合任务计数到精确匹配的项目节点 ─────────────────────────
        for task in tasks {
            let Some(project) = &task.project else { continue };
            if task.status == "deleted" { continue; }
            if let Some(node) = nodes.get_mut(project) {
                match task.status.as_str() {
                    "pending" => {
                        node.pending_count += 1;
                        if task.is_overdue { node.overdue_count += 1; }
                        if task.is_locked  { node.locked_count  += 1; }
                    }
                    "completed" => node.completed_count += 1,
                    "waiting"   => node.waiting_count   += 1,
                    _ => {}
                }
            }
        }

        // ── 步骤 4：从深到浅向上传播计数 ─────────────────────────────────────
        let mut sorted: Vec<String> = nodes.keys().cloned().collect();
        sorted.sort_by_key(|p| std::cmp::Reverse(p.chars().filter(|&c| c == '.').count()));
        for path in &sorted {
            let parts: Vec<&str> = path.split('.').collect();
            if parts.len() < 2 { continue; }
            let parent = parts[..parts.len() - 1].join(".");
            let (pending, completed, waiting, overdue, locked) = {
                let n = &nodes[path];
                (n.pending_count, n.completed_count, n.waiting_count,
                 n.overdue_count, n.locked_count)
            };
            if let Some(p) = nodes.get_mut(&parent) {
                p.pending_count   += pending;
                p.completed_count += completed;
                p.waiting_count   += waiting;
                p.overdue_count   += overdue;
                p.locked_count    += locked;
            }
        }

        // ── 步骤 5：收集根节点 ────────────────────────────────────────────────
        let mut roots: Vec<String> = nodes
            .keys()
            .filter(|p| !p.contains('.'))
            .cloned()
            .collect();
        roots.sort();

        // ── 步骤 6：处理无项目归属任务（虚拟"无项目"节点）───────────────────
        let inbox_tasks: Vec<&TaskView> = tasks
            .iter()
            .filter(|t| t.project.is_none() && t.status != "deleted")
            .collect();

        if !inbox_tasks.is_empty() {
            let mut inbox = ProjectNode::new(INBOX_PROJECT);
            for t in &inbox_tasks {
                match t.status.as_str() {
                    "pending" => {
                        inbox.pending_count += 1;
                        if t.is_overdue { inbox.overdue_count += 1; }
                        if t.is_locked  { inbox.locked_count  += 1; }
                    }
                    "completed" => inbox.completed_count += 1,
                    "waiting"   => inbox.waiting_count   += 1,
                    _ => {}
                }
            }
            nodes.insert(INBOX_PROJECT.to_string(), inbox);
            // 收件箱始终排在最前面
            roots.insert(0, INBOX_PROJECT.to_string());
        }

        ProjectTree { nodes, roots }
    }
}
