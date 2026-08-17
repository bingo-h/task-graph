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
use crate::graph_utils;
use crate::models::project::{ProjectNode, STAGE_ACTIVE, STAGE_PLANNED};
use crate::models::task::Task;
use crate::settings::Settings;

/// 完整图数据响应，与前端 `useApi.js` 约定的结构保持一致
#[derive(Serialize)]
pub struct GraphResponse {
    pub nodes: Vec<Task>,
    pub edges: Vec<Edge>,
    /// "今日任务"视图下的手动排序边，独立于 edges（真实依赖），source 应先于 target 完成
    pub today_order_edges: Vec<Edge>,
    pub projects: HashMap<String, ProjectNode>,
    pub planned_project_roots: Vec<String>,
    pub active_project_roots: Vec<String>,
    pub archived_project_roots: Vec<String>,
    pub trash_project_roots: Vec<String>,
    /// 标签名 -> 颜色/使用计数，任务节点里 tags 仍然只是名字数组
    pub tags: HashMap<String, db::tag::TagInfo>,
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

    /// 备注：非空时作为任务唯一的一条 annotation
    #[serde(default)]
    pub annotation: Option<String>,

    #[serde(default)]
    pub icon: Option<String>,

    #[serde(default)]
    pub color: Option<String>,

    #[serde(default)]
    pub recur_rule: Option<crate::models::task::RecurRule>,
}

/// 修改任务参数
/// 所有字段都标了 default，未提供的键一律视为“不修改这个字段”，
/// 因此调用方可以只传想改的那一两个字段（比如只改备注），不用每次带上完整表单
#[derive(Serialize, Deserialize)]
pub struct ModifyTaskArgs {
    pub uuid: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub project: Option<String>,

    #[serde(default)]
    pub priority: Option<String>,

    #[serde(default)]
    pub due: Option<String>,

    #[serde(default)]
    pub scheduled: Option<String>,

    #[serde(default)]
    pub tags: Option<Vec<String>>,

    #[serde(default)]
    pub depends: Option<Vec<String>>,

    #[serde(default)]
    pub clear_project: bool,

    #[serde(default)]
    pub clear_priority: bool,

    #[serde(default)]
    pub clear_due: bool,

    #[serde(default)]
    pub clear_scheduled: bool,

    /// 备注：非空时整体替换原有的那一条 annotation；不提供则不改动
    #[serde(default)]
    pub annotation: Option<String>,

    /// 显式清空备注（annotation 为空时才有意义）
    #[serde(default)]
    pub clear_annotation: bool,

    #[serde(default)]
    pub icon: Option<String>,

    #[serde(default)]
    pub clear_icon: bool,

    #[serde(default)]
    pub color: Option<String>,

    #[serde(default)]
    pub clear_color: bool,
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

/// 校验任务图标恰好是一个 emoji（按 grapheme cluster 计数，不能用 chars().count()——
/// 旗帜、肤色变体、家庭组合 emoji 等本来就是多个 Unicode 码位拼成一个视觉字符）
fn validate_icon(icon: &str) -> Result<(), String> {
    use unicode_segmentation::UnicodeSegmentation;

    let icon = icon.trim();
    if icon.is_empty() || icon.graphemes(true).count() != 1 {
        return Err("图标必须是单个 emoji".to_string());
    }
    Result::Ok(())
}

/// 从数据库加载所有任务，计算派生字段，构建完整图数据。
fn build_graph() -> anyhow::Result<GraphResponse> {
    let conn = db::open()?;

    let retention_days = crate::settings::load()?.trash_retention_days;
    db::project::purge_expired(&conn, retention_days)?;
    db::task::reset_stale_today_marks(&conn)?;
    db::recur::process_rollovers(&conn)?;

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

    let today_order_edges: Vec<Edge> = db::today_order::list_all(&conn)?
        .into_iter()
        .map(|(source, target)| Edge { source, target })
        .collect();

    let project_records = db::project::list_all(&conn)?;
    let (
        projects,
        planned_project_roots,
        active_project_roots,
        archived_project_roots,
        trash_project_roots,
    ) = db::project::build(&tasks, &project_records);

    let tags = db::tag::list_all(&conn)?
        .into_iter()
        .map(|t| (t.name.clone(), t))
        .collect();

    Ok(GraphResponse {
        nodes: tasks,
        edges,
        today_order_edges,
        projects,
        planned_project_roots,
        active_project_roots,
        archived_project_roots,
        trash_project_roots,
        tags,
    })
}

/// 计算并填充派生字段：is_overdue / is_due_today / is_today / blocking / is_locked / total_seconds / is_timing / active_since
fn apply_derived_fields(conn: &rusqlite::Connection, tasks: &mut Vec<Task>) -> anyhow::Result<()> {
    let (totals, active) = db::time_entry::totals_by_task(conn)?;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    for task in tasks.iter_mut() {
        task.is_overdue = task.compute_overdue();
        task.is_due_today = task.compute_due_today();
        task.is_today = task.today_marked_date.as_deref() == Some(today.as_str());
        task.total_seconds = totals.get(&task.uuid).copied().unwrap_or(0);
        task.active_since = active.get(&task.uuid).cloned();
        task.is_timing = task.active_since.is_some();
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
    if !crate::settings::validate_due_time(&settings.default_due_time) {
        return Err("默认到期时间格式需为 HH:MM".to_string());
    }
    for label in [
        &settings.node_label_project,
        &settings.node_label_due,
        &settings.node_label_priority,
        &settings.node_label_recur,
    ] {
        if label.chars().count() > crate::settings::NODE_LABEL_MAX_LEN {
            return Err(format!(
                "节点信息标签文字最多 {} 个字符",
                crate::settings::NODE_LABEL_MAX_LEN
            ));
        }
    }

    crate::settings::save(&settings).map_err(|e| e.to_string())?;

    Result::Ok(settings)
}

/// 把前端传来的 `due` 补全成完整的 RFC3339 时间戳。
/// 前端日期选择器只给"YYYY-MM-DD"（不带时间），若不补时间，
/// `Task::compute_overdue()` 用的 `parse_from_rfc3339` 会直接解析失败、
/// 永远判定不出逾期。已经是完整时间戳（带 "T"）的值原样透传。
fn normalize_due(due: Option<String>) -> Result<Option<String>, String> {
    let Some(due) = due else { return Result::Ok(None) };

    if due.contains('T') {
        return Result::Ok(Some(due));
    }

    let default_time = crate::settings::load()
        .map_err(|e| e.to_string())?
        .default_due_time;

    Result::Ok(Some(format!("{due}T{default_time}:00Z")))
}

/// 新建任务
#[tauri::command]
pub fn add_task(args: AddTaskArgs) -> Result<GraphResponse, String> {
    if args.description.trim().is_empty() {
        return Err("任务描述不能为空".to_string());
    }
    if let Some(icon) = &args.icon {
        validate_icon(icon)?;
    }

    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::create(
        &conn,
        &CreateTaskRequest {
            description: args.description,
            project: args.project,
            priority: args.priority,
            due: normalize_due(args.due)?,
            scheduled: args.scheduled,
            tags: args.tags,
            depends: args.depends,
            annotation: args.annotation,
            icon: args.icon,
            color: args.color,
            recur_rule: args.recur_rule,
        },
    )
    .map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 修改任务
#[tauri::command]
pub fn modify_task(args: ModifyTaskArgs) -> Result<GraphResponse, String> {
    if let Some(icon) = &args.icon {
        validate_icon(icon)?;
    }

    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::update(
        &conn,
        &args.uuid,
        &UpdateTaskRequest {
            description: args.description,
            project: args.project,
            priority: args.priority,
            due: normalize_due(args.due)?,
            scheduled: args.scheduled,
            tags: args.tags,
            depends: args.depends,
            clear_project: args.clear_project,
            clear_priority: args.clear_priority,
            clear_due: args.clear_due,
            clear_scheduled: args.clear_scheduled,
            annotation: args.annotation,
            clear_annotation: args.clear_annotation,
            icon: args.icon,
            clear_icon: args.clear_icon,
            color: args.color,
            clear_color: args.clear_color,
        },
    )
    .map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 改变依赖关系终点的参数
#[derive(Deserialize)]
pub struct ReconnectDependencyArgs {
    pub source_uuid: String,
    pub old_target_uuid: String,
    /// None 表示拖到空白处：只删除旧的依赖关系，不建立新的
    pub new_target_uuid: Option<String>,
}

/// 把"source_uuid 是 old_target_uuid 的前置任务"这条依赖关系，改成指向 new_target_uuid
/// （new_target_uuid 为空表示直接删除这条依赖），对应拖拽图谱里已有连线终点的交互
#[tauri::command]
pub fn reconnect_dependency(args: ReconnectDependencyArgs) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    let old_target = db::task::get_by_uuid(&conn, &args.old_target_uuid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "旧的依赖任务不存在".to_string())?;

    let new_depends: Vec<String> =
        old_target.depends.into_iter().filter(|d| d != &args.source_uuid).collect();
    db::task::set_depends(&conn, &args.old_target_uuid, new_depends).map_err(|e| e.to_string())?;

    if let Some(new_target_uuid) = &args.new_target_uuid {
        let new_target = db::task::get_by_uuid(&conn, new_target_uuid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "新的目标任务不存在".to_string())?;

        if !new_target.depends.contains(&args.source_uuid) {
            let mut depends = new_target.depends;
            depends.push(args.source_uuid.clone());
            db::task::set_depends(&conn, new_target_uuid, depends).map_err(|e| e.to_string())?;
        }
    }

    build_graph().map_err(|e| e.to_string())
}

/// 为"今日任务"视图新增一条手动排序边：from_uuid 应先于 to_uuid 完成。
/// 校验：不能和真实的 depends 依赖图矛盾（不能要求后置任务先于前置任务完成），
/// 也不能在手动排序图自身内部形成环。
#[tauri::command]
pub fn add_today_order_edge(from_uuid: String, to_uuid: String) -> Result<GraphResponse, String> {
    if from_uuid == to_uuid {
        return Err("不能连接到自己".to_string());
    }

    let conn = db::open().map_err(|e| e.to_string())?;
    let tasks = db::task::list_all(&conn).map_err(|e| e.to_string())?;

    // 1. 不能与真实依赖图矛盾：若 to_uuid 在真实依赖图里能到达 from_uuid，
    //    说明原本的顺序是 to 必须先于 from，和这条新边要求的方向正好相反
    let depends_adj = graph_utils::forward_adjacency(&tasks);
    if graph_utils::reachable(&depends_adj, &to_uuid, &from_uuid) {
        return Err("与已有的依赖链矛盾：这两个任务原本的先后顺序不能颠倒".to_string());
    }

    // 2. 不能在手动排序图自身内部形成环
    let existing_edges = db::today_order::list_all(&conn).map_err(|e| e.to_string())?;
    let order_adj = graph_utils::adjacency_from_pairs(&existing_edges);
    if graph_utils::reachable(&order_adj, &to_uuid, &from_uuid) {
        return Err("这样连接会在今日任务排序里形成循环".to_string());
    }

    db::today_order::add_edge(&conn, &from_uuid, &to_uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 删除一条"今日任务"手动排序边
#[tauri::command]
pub fn remove_today_order_edge(from_uuid: String, to_uuid: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::today_order::remove_edge(&conn, &from_uuid, &to_uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 重命名标签（用到旧名字的任务一起改名）；若新名字已存在则合并为同一个标签
#[tauri::command]
pub fn rename_tag(old_tag: String, new_tag: String) -> Result<GraphResponse, String> {
    let old_tag = old_tag.trim();
    let new_tag = new_tag.trim();

    if old_tag.is_empty() || new_tag.is_empty() {
        return Err("标签不能为空".to_string());
    }

    if old_tag != new_tag {
        let conn = db::open().map_err(|e| e.to_string())?;
        db::tag::rename(&conn, old_tag, new_tag).map_err(|e| e.to_string())?;
    }

    build_graph().map_err(|e| e.to_string())
}

/// 设置标签颜色的参数
#[derive(Deserialize)]
pub struct SetTagColorArgs {
    pub name: String,
    /// 传 None / 空字符串表示清空颜色，前端会回退到默认颜色
    pub color: Option<String>,
}

/// 设置标签颜色
#[tauri::command]
pub fn set_tag_color(args: SetTagColorArgs) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    let color = args.color.as_deref().map(str::trim).filter(|c| !c.is_empty());
    db::tag::set_color(&conn, args.name.trim(), color).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 彻底删除一个标签，解除它和所有任务的关联
#[tauri::command]
pub fn delete_tag(name: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::tag::delete(&conn, name.trim()).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 标记任务完成
#[tauri::command]
pub fn done_task(uuid: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::mark_done(&conn, &uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 批量标记多个任务完成（框选/Ctrl 多选后的批量操作）
#[tauri::command]
pub fn done_tasks(uuids: Vec<String>) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    for uuid in &uuids {
        db::task::mark_done(&conn, uuid).map_err(|e| e.to_string())?;
    }

    build_graph().map_err(|e| e.to_string())
}

/// 取消任务完成，恢复为待办
#[tauri::command]
pub fn undone_task(uuid: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::mark_pending(&conn, &uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 设置/取消"今日任务"标记
#[tauri::command]
pub fn set_task_today(uuid: String, marked: bool) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::task::set_today(&conn, &uuid, marked).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 批量设置多个任务的"今日任务"参数
#[derive(Deserialize)]
pub struct SetTasksTodayArgs {
    pub uuids: Vec<String>,
    pub marked: bool,
}

/// 批量设置/取消多个任务的"今日任务"标记（框选/Ctrl 多选后的批量操作）
#[tauri::command]
pub fn set_tasks_today(args: SetTasksTodayArgs) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    for uuid in &args.uuids {
        db::task::set_today(&conn, uuid, args.marked).map_err(|e| e.to_string())?;
    }

    build_graph().map_err(|e| e.to_string())
}

/// 设置周期性任务规则的参数
#[derive(Deserialize)]
pub struct SetTaskRecurArgs {
    pub uuid: String,
    /// None 表示停止重复（历史 recur_log 保留，只是不再产生新记录）
    pub rule: Option<crate::models::task::RecurRule>,
}

/// 开启/关闭任务的周期性重复
#[tauri::command]
pub fn set_task_recur(args: SetTaskRecurArgs) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::recur::set_recur_rule(&conn, &args.uuid, args.rule).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 查询某个周期性任务当前的连续完成天数
#[tauri::command]
pub fn get_recur_streak(uuid: String) -> Result<i64, String> {
    let conn = db::open().map_err(|e| e.to_string())?;
    db::recur::current_streak(&conn, &uuid).map_err(|e| e.to_string())
}

/// 查询某个周期性任务的全部完成记录（日历页用）
#[tauri::command]
pub fn list_recur_log(uuid: String) -> Result<Vec<db::recur::RecurLogEntry>, String> {
    let conn = db::open().map_err(|e| e.to_string())?;
    db::recur::list_log(&conn, &uuid).map_err(|e| e.to_string())
}

/// 开始为指定任务计时，若有其他任务正在计时则自动先结束
#[tauri::command]
pub fn start_timer(uuid: String) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::time_entry::start(&conn, &uuid).map_err(|e| e.to_string())?;

    build_graph().map_err(|e| e.to_string())
}

/// 同时为多个任务开始计时（框选/Ctrl 多选后批量计时，共享同一段开始时间，
/// 各自单独记一条计时记录），若有其他任务正在计时则自动先结束。
/// 停止时直接复用单任务的 `stop_timer`：结束这段计时即可，不会连带标记任务完成。
#[tauri::command]
pub fn start_group_timer(uuids: Vec<String>) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    db::time_entry::start_many(&conn, &uuids).map_err(|e| e.to_string())?;

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

/// 批量删除多个任务（框选/Ctrl 多选后的批量操作）
#[tauri::command]
pub fn delete_tasks(uuids: Vec<String>) -> Result<GraphResponse, String> {
    let conn = db::open().map_err(|e| e.to_string())?;

    for uuid in &uuids {
        db::task::mark_deleted(&conn, uuid).map_err(|e| e.to_string())?;
    }

    build_graph().map_err(|e| e.to_string())
}
