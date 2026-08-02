//! 任务核心数据结构

use serde::{Deserialize, Serialize};

/// 任务状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Completed,
    Deleted,
    Waiting,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Completed => "completed",
            TaskStatus::Deleted => "deleted",
            TaskStatus::Waiting => "waiting",
        }
    }
}

/// 任务优先级
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    #[serde(rename = "H")]
    High,
    #[serde(rename = "M")]
    Medium,
    #[serde(rename = "L")]
    Low,
}

/// 任务备注
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub entry: Option<String>,
    pub description: String,
}

/// 任务数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub uuid: String,
    pub description: String,

    #[serde(serialize_with = "serialize_status")]
    pub status: TaskStatus,

    pub project: Option<String>,

    #[serde(serialize_with = "serialize_priority")]
    pub priority: Option<Priority>,

    pub urgency: f64,

    pub due: Option<String>,
}

fn serialize_status<S: serde::Serializer>(status: &TaskStatus, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(status.as_str())
}

fn serialize_priority<S: serde::Serializer>(
    priority: &Option<Priority>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match priority {
        None => s.serialize_none(),
        Some(Priority::High) => s.serialize_some("H"),
        Some(Priority::Medium) => s.serialize_some("M"),
        Some(Priority::Low) => s.serialize_some("L"),
    }
}
