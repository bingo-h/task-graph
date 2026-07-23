//! 任务数据结构。
//!
//! Task 是内部表示，同时也是 API 序列化的视图
//! 日期统一使用 ISO 8601 字符串存储和传输

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 任务生命周期状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Completed,
    Deleted,
    Waiting,
}

/// 枚举定义
impl TaskStatus {
    /// 返回状态的英文小写标签，用于 API 响应。
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Completed => "completed",
            TaskStatus::Deleted => "deleted",
            TaskStatus::Waiting => "waiting",
        }
    }
}

/// 任务优先级。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    #[serde(rename = "H")]
    High,
    #[serde(rename = "M")]
    Medium,
    #[serde(rename = "L")]
    Low,
}

// ── 原始任务结构（从 task export JSON 反序列化）────────────────────────────────

/// 从 `task export` JSON 解析的原始任务结构。
///
/// 字段与 Taskwarrior JSON schema 一一对应，未知字段由 serde 忽略。
/// 此结构仅用于内部处理，不直接序列化为 API 响应。
#[derive(Debug, Clone, Deserialize)]
pub struct RawTask {
    pub uuid: String,
    pub description: String,
    pub status: TaskStatus,

    #[serde(default)]
    pub project: Option<String>,

    #[serde(default)]
    pub tags: Option<Vec<String>>,

    #[serde(default)]
    pub priority: Option<Priority>,

    #[serde(default)]
    pub urgency: f64,

    #[serde(default, deserialize_with = "deserialize_tw_datetime")]
    pub due: Option<DateTime<Utc>>,

    #[serde(default, deserialize_with = "deserialize_tw_datetime")]
    pub scheduled: Option<DateTime<Utc>>,

    #[serde(default, deserialize_with = "deserialize_tw_datetime")]
    pub entry: Option<DateTime<Utc>>,

    #[serde(default, deserialize_with = "deserialize_tw_datetime")]
    pub end: Option<DateTime<Utc>>,

    /// 此任务依赖的其他任务 UUID 列表（这些任务必须先完成）
    #[serde(default)]
    pub depends: Vec<String>,

    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

/// 任务备注条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    #[serde(default, deserialize_with = "deserialize_tw_datetime")]
    pub entry: Option<DateTime<Utc>>,
    pub description: String,
}

// ── API 视图结构（序列化为 API 响应）─────────────────────────────────────────

/// 序列化为 API 响应的任务视图。
///
/// 与 `RawTask` 的区别：
/// - 日期字段转为 ISO 8601 字符串（前端友好）
/// - 增加派生字段：`is_overdue`、`is_due_today`、`is_locked`
/// - 增加反向依赖字段：`blocking`
#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    pub uuid: String,
    pub description: String,
    pub status: String,

    pub project: Option<String>,
    pub tags: Vec<String>,
    pub priority: Option<String>,
    pub urgency: f64,

    /// ISO 8601 格式的日期字符串，例如 "2026-05-20T14:00:00Z"
    pub due: Option<String>,
    pub scheduled: Option<String>,
    pub entry: Option<String>,
    pub end: Option<String>,

    pub depends: Vec<String>,
    pub annotations: Vec<AnnotationView>,

    /// 依赖此任务的任务 UUID 列表（depends 的反向关系）
    pub blocking: Vec<String>,

    /// 此任务是否已逾期（截止日期早于当前时间且状态为 pending）
    pub is_overdue: bool,
    /// 此任务是否在今日 24 小时内到期
    pub is_due_today: bool,
    /// 此任务是否被锁定（有未完成的前置任务）
    pub is_locked: bool,
}

/// 备注的 API 视图（日期转为 ISO 字符串）。
#[derive(Debug, Clone, Serialize)]
pub struct AnnotationView {
    pub entry: Option<String>,
    pub description: String,
}

// ── 转换逻辑 ──────────────────────────────────────────────────────────────────

/// 将 `DateTime<Utc>` 格式化为前端友好的 ISO 字符串。
pub fn fmt_dt(dt: Option<DateTime<Utc>>) -> Option<String> {
    dt.map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

impl RawTask {
    /// 判断任务是否逾期。
    pub fn is_overdue(&self) -> bool {
        if self.status != TaskStatus::Pending {
            return false;
        }
        self.due.map_or(false, |d| d < Utc::now())
    }

    /// 判断任务是否在今日 24 小时内到期。
    pub fn is_due_today(&self) -> bool {
        if self.status != TaskStatus::Pending {
            return false;
        }
        self.due.map_or(false, |d| {
            let delta = d.signed_duration_since(Utc::now());
            delta.num_seconds() >= 0 && delta.num_hours() < 24
        })
    }

    /// 将 `RawTask` 转换为 `TaskView`，附加派生字段。
    ///
    /// `blocking` 和 `is_locked` 在批量处理时由 `build_task_views` 统一计算。
    pub fn to_view(&self) -> TaskView {
        TaskView {
            uuid: self.uuid.clone(),
            description: self.description.clone(),
            status: self.status.as_str().to_string(),
            project: self.project.clone(),
            tags: self.tags.clone().unwrap_or_default(),
            priority: self.priority.as_ref().map(|p| {
                match p {
                    Priority::High => "H",
                    Priority::Medium => "M",
                    Priority::Low => "L",
                }
                .to_string()
            }),
            urgency: self.urgency,
            due: fmt_dt(self.due),
            scheduled: fmt_dt(self.scheduled),
            entry: fmt_dt(self.entry),
            end: fmt_dt(self.end),
            depends: self.depends.clone(),
            annotations: self
                .annotations
                .iter()
                .map(|a| AnnotationView {
                    entry: fmt_dt(a.entry),
                    description: a.description.clone(),
                })
                .collect(),
            blocking: Vec::new(), // 由 build_task_views 填充
            is_overdue: self.is_overdue(),
            is_due_today: self.is_due_today(),
            is_locked: false, // 由 build_task_views 填充
        }
    }
}

/// 批量将 `RawTask` 列表转换为 `TaskView` 列表，同时计算 blocking 和 is_locked。
pub fn build_task_views(raw_tasks: &[RawTask]) -> Vec<TaskView> {
    let mut views: Vec<TaskView> = raw_tasks.iter().map(|t| t.to_view()).collect();

    // 构建 UUID → 索引映射，用于 O(1) 查找
    // 注意：这里用 String 而非 &str 作为 key，避免持有对 views 的借用
    let uuid_to_idx: std::collections::HashMap<String, usize> = views
        .iter()
        .enumerate()
        .map(|(i, v)| (v.uuid.clone(), i))
        .collect();

    // 收集所有依赖边（child_idx, dep_uuid）
    let edges: Vec<(usize, String)> = views
        .iter()
        .enumerate()
        .flat_map(|(child_idx, v)| v.depends.iter().map(move |dep| (child_idx, dep.clone())))
        .collect();

    // 先收集所有需要修改的操作，再统一应用，避免同时持有可变和不可变借用
    let mut blocking_updates: Vec<(usize, String)> = Vec::new(); // (parent_idx, child_uuid)
    let mut lock_updates: Vec<usize> = Vec::new(); // child_idx

    for (child_idx, dep_uuid) in &edges {
        if let Some(&parent_idx) = uuid_to_idx.get(dep_uuid.as_str()) {
            let child_uuid = views[*child_idx].uuid.clone();
            blocking_updates.push((parent_idx, child_uuid));
            if views[parent_idx].status != "completed" {
                lock_updates.push(*child_idx);
            }
        }
    }

    // 统一应用修改
    for (parent_idx, child_uuid) in blocking_updates {
        views[parent_idx].blocking.push(child_uuid);
    }
    for child_idx in lock_updates {
        views[child_idx].is_locked = true;
    }

    views
}
