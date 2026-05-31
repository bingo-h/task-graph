//! 数据加载层。
//!
//! 调用 `task export` 获取任务数据，构建任务视图和项目树。
//! 与 Python 版本逻辑相同，用 Rust 重写。

use anyhow::{Context, Result};
use serde::Serialize;
use std::process::Command;

use crate::task::{RawTask, TaskView, build_task_views};
use crate::project::ProjectTree;
#[allow(unused_imports)]
use crate::constants::INBOX_PROJECT;

/// API 响应的完整图数据结构，直接序列化后返回给前端。
#[derive(Debug, Serialize)]
pub struct GraphData {
    /// 任务节点列表（含派生字段）
    pub nodes: Vec<TaskView>,
    /// 依赖关系边列表，格式 `{source: uuid, target: uuid}`
    /// 含义：source 是 target 的前置任务
    pub edges: Vec<Edge>,
    /// 项目树节点字典，key 为完整路径
    pub projects: std::collections::HashMap<String, crate::project::ProjectNode>,
    /// 根项目路径列表（含"无项目"虚拟节点）
    pub project_roots: Vec<String>,
}

/// 任务依赖关系边。
#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    /// 前置任务 UUID
    pub source: String,
    /// 后续任务 UUID
    pub target: String,
}

/// 调用 `task export` 并返回完整的图数据。
///
/// 流程：
/// 1. 执行 `task export`，获取 JSON 数组
/// 2. 解析为 `RawTask` 列表，过滤 deleted 任务
/// 3. 构建 `TaskView` 列表（含 blocking、is_locked 等派生字段）
/// 4. 构建项目树
/// 5. 提取依赖边
pub fn load_graph_data() -> Result<GraphData> {
    // ── 步骤 1：执行 task export ──────────────────────────────────────────────
    let output = Command::new("task")
        .arg("export")
        .output()
        .context("无法执行 task export，请确认 Taskwarrior 已安装且在 PATH 中")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("task export 失败：{}", stderr);
    }

    // ── 步骤 2：解析 JSON，过滤 deleted ──────────────────────────────────────
    let raw_tasks: Vec<RawTask> = serde_json::from_slice(&output.stdout)
        .context("解析 task export JSON 失败")?;

    let raw_tasks: Vec<RawTask> = raw_tasks
        .into_iter()
        .filter(|t| t.status != crate::task::TaskStatus::Deleted)
        .collect();

    // ── 步骤 3：构建任务视图（含派生字段）────────────────────────────────────
    let nodes = build_task_views(&raw_tasks);

    // ── 步骤 4：构建项目树 ────────────────────────────────────────────────────
    let tree = ProjectTree::build(&nodes);

    // ── 步骤 5：提取依赖边 ────────────────────────────────────────────────────
    let edges: Vec<Edge> = nodes
        .iter()
        .flat_map(|task| {
            task.depends.iter().map(|dep_uuid| Edge {
                source: dep_uuid.clone(),
                target: task.uuid.clone(),
            })
        })
        .collect();

    Ok(GraphData {
        nodes,
        edges,
        projects:      tree.nodes,
        project_roots: tree.roots,
    })
}

/// 执行一条 `task` 子命令。
///
/// 统一添加 `rc.confirmation=off rc.bulk=0` 跳过交互确认。
/// 成功返回 stdout，失败返回带错误信息的 `Err`。
pub fn run_task_command(args: &[&str]) -> Result<String> {
    let output = Command::new("task")
        .args(["rc.confirmation=off", "rc.bulk=0"])
        .args(args)
        .output()
        .context("无法执行 task 命令")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
