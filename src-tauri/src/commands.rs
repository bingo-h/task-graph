//! Tauri 命令层
//!
//! 每个 `#[tauri::command]` 对应前端 `invoke('函数名', ...)` 的一次调用。
//! 本文件负责参数接受和错误类型转换
//!

use std::collections::HashMap;

use anyhow::Ok;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::db::task::{CreateTaskRequest, UpdateTaskRequest};
use crate::models::project::ProjectNode;
use crate::models::task::Task;

/// 完整图数据响应，与前端 `useApi.js` 约定的结构保持一致
#[derive(Serialize)]
pub struct GraphResponse {
    pub nodes: Vec<Task>,
    pub edges: Vec<Edge>,
    pub projects: HashMap<String, ProjectNode>,
    pub project_roots: Vec<String>,
}

/// 依赖关系边：source 是 target 的前置任务
#[derive(Serialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
}

/// 新建任务参数
#[derive(Deserialize)]
pub struct AddTaskArgs {
    pub description: String,
    pub project: Option<String>,
    pub priority: Option<String>,
    pub due: Option<String>,
    pub scheduled: Option<String>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub depends: Vec<String>,
}

/// 修改任务参数
#[derive(Serialize, Deserialize)]
pub struct ModifyTaskArgs {
    pub uuid: String,
    pub description: Option<String>,
    pub project: Option<String>,
    pub priority: Option<String>,
    pub due: Option<String>,
    pub scheduled: Option<String>,
    pub tags: Option<Vec<String>>,
    pub depends: Option<Vec<String>>,

    #[serde(default)]
    pub clear_project: bool,

    #[serde(default)]
    pub clear_priority: bool,

    #[serde(default)]
    pub clear_due: bool,

    #[serde(default)]
    pub clear_scheduled: bool,
}

/// 从数据库加载所有任务，计算派生字段，构建完整图数据。
fn build_graph() -> anyhow::Result<GraphResponse> {
    let conn = db::open()?;

    let mut tasks = db::task::list_all(&conn)?;
    apply_derived_fields(&mut tasks);

    let edges: Vec<Edge> = tasks
        .iter()
        .flat_map(|t| {
            t.depends.iter().map(|dep| Edge {
                source: dep.clone(),
                target: t.uuid.clone(),
            })
        })
        .collect();

    let (projects, project_roots) = db::project::build(&tasks);

    Ok(GraphResponse {
        nodes: tasks,
        edges,
        projects,
        project_roots,
    })
}

/// 计算并填充派生字段：is_overdue / is_due_today / blocking / is_locked
fn apply_derived_fields(tasks: &mut Vec<Task>) {
    for task in tasks.iter_mut() {
        task.is_overdue = task.compute_overdue();
        task.is_due_today = task.compute_due_today();
    }

    let uuid_to_idx: HashMap<String, usize> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.uuid.clone(), i))
        .collect();

    // 记录父任务阻塞的子任务：[(父任务索引, 子任务 UUID)]
    let mut blocking_updates: Vec<(usize, String)> = Vec::new();
    // 记录需要标记为锁定的子任务索引
    let mut lock_updates: Vec<usize> = Vec::new();

    // 遍历所有任务，检查其依赖关系（depends）
    // idx: 当前任务在 tasks 向量中的索引；task: 当前任务引用
    for (idx, task) in tasks.iter().enumerate() {
        // 对于当前任务的每个依赖 UUID
        for dep_uuid in &task.depends {
            // 将依赖的 UUID 映射到 tasks 中对应的父任务索引（如果存在）
            if let Some(&parent_idx) = uuid_to_idx.get(dep_uuid) {
                // 记录哪些父任务阻塞当前任务
                blocking_updates.push((parent_idx, task.uuid.clone()));

                // 如果父任务尚未完成，则子任务应被标记为锁定（blocked）
                if tasks[parent_idx].status.as_str() != "completed" {
                    lock_updates.push(idx);
                }
            }
        }
    }

    // 记录任务阻塞了哪些子任务
    for (idx, children_uuid) in blocking_updates {
        tasks[idx].blocking.push(children_uuid);
    }

    // 标记 lock_updates 队列中的任务为锁定
    for idx in lock_updates {
        tasks[idx].is_locked = true;
    }
}

// Tauri 命令

/// 获取所有任务和项目树数据
#[tauri::command]
pub fn get_tasks() -> Result<GraphResponse, String> {
    build_graph().map_err(|e| e.to_string())
}

/// 新建任务
#[tauri::command]
pub fn add_task(args: AddTaskArgs) -> Result<GraphResponse, String> {
    if args.description.trim().is_empty() {
        return Err("任务描述不能为空".to_string());
    }

    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::create(
        &conn,
        &CreateTaskRequest {
            description: args.description,
            project: args.project,
            priority: args.priority,
            due: args.due,
            scheduled: args.scheduled,
            tags: args.tags,
            depends: args.depends,
        },
    )
    .map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 修改任务
#[tauri::command]
pub fn modify_task(args: ModifyTaskArgs) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::update(
        &conn,
        &args.uuid,
        &UpdateTaskRequest {
            description: args.description,
            project: args.project,
            priority: args.priority,
            due: args.due,
            scheduled: args.scheduled,
            tags: args.tags,
            depends: args.depends,
            clear_project: args.clear_project,
            clear_priority: args.clear_priority,
            clear_due: args.clear_due,
            clear_scheduled: args.clear_scheduled,
        },
    )
    .map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 标记任务完成
#[tauri::command]
pub fn done_task(uuid: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::mark_done(&conn, &uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 取消任务完成，恢复为待办
#[tauri::command]
pub fn undone_task(uuid: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::mark_pending(&conn, &uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 删除任务
#[tauri::command]
pub fn delete_task(uuid: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::mark_deleted(&conn, &uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}
