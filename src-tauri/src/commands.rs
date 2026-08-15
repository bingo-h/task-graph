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
use crate::models::project::{ProjectNode, STAGE_ACTIVE, STAGE_PLANNED};
use crate::models::task::Task;
use crate::settings::Settings;

/// 完整图数据响应，与前端 `useApi.js` 约定的结构保持一致
#[derive(Serialize)]
pub struct GraphResponse {
    pub nodes: Vec<Task>,
    pub edges: Vec<Edge>,
    pub projects: HashMap<String, ProjectNode>,
    pub planned_project_roots: Vec<String>,
    pub active_project_roots: Vec<String>,
    pub archived_project_roots: Vec<String>,
    pub trash_project_roots: Vec<String>,
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

/// 无项目归属任务的虚拟项目路径标识符，与 `db::project` 内的常量保持一致
const INBOX_PROJECT: &str = "无项目";

/// 新建项目参数
#[derive(Deserialize)]
pub struct CreateProjectArgs {
    pub path: String,
    /// "planned"（计划中）或 "active"（进行中），不传时默认 "active"
    pub stage: Option<String>,
}

/// 设置项目归档状态参数
#[derive(Deserialize)]
pub struct ArchiveProjectArgs {
    pub path: String,
    pub archived: bool,
}

/// 设置项目阶段参数
#[derive(Deserialize)]
pub struct SetProjectStageArgs {
    pub path: String,
    pub stage: String,
}

/// 校验阶段取值是否合法
fn validate_stage(stage: &str) -> Result<(), String> {
    if stage == STAGE_PLANNED || stage == STAGE_ACTIVE {
        Result::Ok(())
    } else {
        Err("无效的项目阶段".to_string())
    }
}

/// 从数据库加载所有任务，计算派生字段，构建完整图数据。
fn build_graph() -> anyhow::Result<GraphResponse> {
    let conn = db::open()?;

    let retention_days = crate::settings::load()?.trash_retention_days;
    db::project::purge_expired(&conn, retention_days)?;

    let mut tasks = db::task::list_all(&conn)?;
    apply_derived_fields(&conn, &mut tasks)?;

    let edges: Vec<Edge> = tasks
        .iter()
        .flat_map(|t| {
            t.depends.iter().map(|dep| Edge {
                source: dep.clone(),
                target: t.uuid.clone(),
            })
        })
        .collect();

    let project_records = db::project::list_all(&conn)?;
    let (
        projects,
        planned_project_roots,
        active_project_roots,
        archived_project_roots,
        trash_project_roots,
    ) = db::project::build(&tasks, &project_records);

    Ok(GraphResponse {
        nodes: tasks,
        edges,
        projects,
        planned_project_roots,
        active_project_roots,
        archived_project_roots,
        trash_project_roots,
    })
}

/// 计算并填充派生字段：is_overdue / is_due_today / blocking / is_locked / total_seconds / is_timing / active_since
fn apply_derived_fields(conn: &rusqlite::Connection, tasks: &mut Vec<Task>) -> anyhow::Result<()> {
    let (totals, active_task, active_since) = db::time_entry::totals_by_task(conn)?;

    for task in tasks.iter_mut() {
        task.is_overdue = task.compute_overdue();
        task.is_due_today = task.compute_due_today();
        task.total_seconds = totals.get(&task.uuid).copied().unwrap_or(0);
        task.is_timing = active_task.as_deref() == Some(task.uuid.as_str());
        task.active_since = if task.is_timing {
            active_since.clone()
        } else {
            None
        };
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

    Ok(())
}

// Tauri 命令

/// 获取所有任务和项目树数据
#[tauri::command]
pub fn get_tasks() -> Result<GraphResponse, String> {
    build_graph().map_err(|e| e.to_string())
}

/// 新建项目，允许在没有任何任务的情况下独立创建
#[tauri::command]
pub fn create_project(args: CreateProjectArgs) -> Result<GraphResponse, String> {
    let path = args.path.trim();

    if path.is_empty() {
        return Err("项目路径不能为空".to_string());
    }

    if path == INBOX_PROJECT {
        return Err("该项目名称是保留名称".to_string());
    }

    let segments: Vec<&str> = path.split('.').collect();
    if segments.iter().any(|s| s.trim().is_empty()) {
        return Err("项目路径的每一段都不能为空，如 personal.reading".to_string());
    }

    let stage = args.stage.as_deref().unwrap_or(STAGE_ACTIVE);
    validate_stage(stage)?;

    let conn = db::open().map_err(|e| e.to_string())?;

    db::project::create(&conn, path, stage).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 设置项目所属阶段（计划中 / 进行中）
#[tauri::command]
pub fn set_project_stage(args: SetProjectStageArgs) -> Result<GraphResponse, String> {
    let path = args.path.trim();

    if path.is_empty() || path == INBOX_PROJECT {
        return Err("无效的项目路径".to_string());
    }

    if path.contains('.') {
        return Err("子项目不能单独设置阶段，请在顶层项目上操作".to_string());
    }

    validate_stage(&args.stage)?;

    let conn = db::open().map_err(|e| e.to_string())?;

    db::project::set_stage(&conn, path, &args.stage).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 设置项目归档状态（归档会级联到所有子项目）
#[tauri::command]
pub fn set_project_archived(args: ArchiveProjectArgs) -> Result<GraphResponse, String> {
    let path = args.path.trim();

    if path.is_empty() || path == INBOX_PROJECT {
        return Err("无效的项目路径".to_string());
    }

    if path.contains('.') {
        return Err("子项目不能单独归档，请在顶层项目上操作".to_string());
    }

    let conn = db::open().map_err(|e| e.to_string())?;

    db::project::set_archived(&conn, path, args.archived).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 将项目移入废纸篓（软删除，级联到所有子项目，可恢复）
#[tauri::command]
pub fn trash_project(path: String) -> Result<GraphResponse, String> {
    let path = path.trim();

    if path.is_empty() || path == INBOX_PROJECT {
        return Err("无效的项目路径".to_string());
    }

    let conn = db::open().map_err(|e| e.to_string())?;

    db::project::trash(&conn, path).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 从废纸篓恢复项目
#[tauri::command]
pub fn restore_project(path: String) -> Result<GraphResponse, String> {
    let path = path.trim();

    if path.is_empty() || path == INBOX_PROJECT {
        return Err("无效的项目路径".to_string());
    }

    let conn = db::open().map_err(|e| e.to_string())?;

    db::project::restore(&conn, path).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 彻底删除项目：级联删除该项目及所有子项目下的任务，不可恢复
#[tauri::command]
pub fn purge_project(path: String) -> Result<GraphResponse, String> {
    let path = path.trim();

    if path.is_empty() || path == INBOX_PROJECT {
        return Err("无效的项目路径".to_string());
    }

    let conn = db::open().map_err(|e| e.to_string())?;

    db::project::purge(&conn, path).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 移动项目参数
#[derive(Deserialize)]
pub struct MoveProjectArgs {
    pub path: String,
    /// 新的父项目路径；不传或为空表示移动到顶层
    pub new_parent: Option<String>,
}

/// 移动项目（及其所有子项目、任务）到新的父项目下，或移动到顶层
#[tauri::command]
pub fn move_project(args: MoveProjectArgs) -> Result<GraphResponse, String> {
    let path = args.path.trim();

    if path.is_empty() || path == INBOX_PROJECT {
        return Err("无效的项目路径".to_string());
    }

    let new_parent = args
        .new_parent
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty());

    if new_parent == Some(INBOX_PROJECT) {
        return Err("无效的目标项目".to_string());
    }

    let conn = db::open().map_err(|e| e.to_string())?;

    db::project::move_project(&conn, path, new_parent).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 获取应用设置
#[tauri::command]
pub fn get_settings() -> Result<Settings, String> {
    crate::settings::load().map_err(|e| e.to_string())
}

/// 保存应用设置
#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<Settings, String> {
    if settings.trash_retention_days > 3650 {
        return Err("保留天数过长".to_string());
    }
    if !(8..=32).contains(&settings.font_size) {
        return Err("字体大小需在 8-32 之间".to_string());
    }

    crate::settings::save(&settings).map_err(|e| e.to_string())?;

    Result::Ok(settings)
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

/// 开始为指定任务计时，若有其他任务正在计时则自动先结束
#[tauri::command]
pub fn start_timer(uuid: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::time_entry::start(&conn, &uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 停止计时后的响应：附带被结束的那条计时记录 id，
/// 供前端弹窗询问这段专注的回忆总结（没有正在计时的段时为 None）
#[derive(Serialize)]
pub struct StopTimerResult {
    #[serde(flatten)]
    pub graph: GraphResponse,
    pub stopped_entry_id: Option<i64>,
}

/// 停止当前正在进行的计时
#[tauri::command]
pub fn stop_timer() -> Result<StopTimerResult, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    let stopped_entry_id = db::time_entry::stop_active(&conn).map_err(|e| e.to_string())?;

    let graph = build_graph().map_err(|e| e.to_string())?;

    Result::Ok(StopTimerResult {
        graph,
        stopped_entry_id,
    })
}

/// 获取某任务的全部计时记录，按开始时间倒序
#[tauri::command]
pub fn list_time_entries(
    uuid: String,
) -> Result<Vec<crate::models::time_entry::TimeEntry>, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::time_entry::list_by_task(&conn, &uuid).map_err(|e| e.to_string())
}

/// 获取所有任务的全部计时记录（用于首页仪表盘按天聚合统计）
#[tauri::command]
pub fn list_all_time_entries() -> Result<Vec<crate::models::time_entry::TimeEntry>, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::time_entry::list_all(&conn).map_err(|e| e.to_string())
}

/// 保存/修改某条计时记录的回忆总结参数
#[derive(Deserialize)]
pub struct SaveTimeEntryNoteArgs {
    pub id: i64,
    pub title: Option<String>,
    pub body: Option<String>,
}

/// 保存某段专注的回忆总结（标题 + 正文），停止计时后弹窗填写，也可事后修改
#[tauri::command]
pub fn save_time_entry_note(args: SaveTimeEntryNoteArgs) -> Result<(), String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::time_entry::save_note(&conn, args.id, args.title.as_deref(), args.body.as_deref())
        .map_err(|e| e.to_string())
}

/// 删除某条计时记录（不可恢复），删除后耗时统计需要重新计算，因此返回最新的完整图数据
#[tauri::command]
pub fn delete_time_entry(id: i64) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::time_entry::delete(&conn, id).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 删除任务
#[tauri::command]
pub fn delete_task(uuid: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::mark_deleted(&conn, &uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}
