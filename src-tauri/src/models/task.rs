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

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::High => "H",
            Priority::Medium => "M",
            Priority::Low => "L",
        }
    }
}

/// 任务备注
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub created_at: Option<String>,
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
    pub scheduled: Option<String>,

    /// 创建日期
    pub created_at: String,

    /// 截止日期
    pub end: Option<String>,

    pub tags: Vec<String>,
    pub depends: Vec<String>,
    pub annotations: Vec<Annotation>,

    /// 派生字段 (不入库，由 API 层计算后得出)
    pub blocking: Vec<String>,
    pub is_overdue: bool,
    pub is_due_today: bool,
    pub is_locked: bool,

    /// 派生字段：累计花费在该任务上的秒数（含正在进行的计时段）
    #[serde(default)]
    pub total_seconds: i64,

    /// 派生字段：该任务当前是否正在计时
    #[serde(default)]
    pub is_timing: bool,

    /// 派生字段：若正在计时，这一段计时的开始时间
    #[serde(default)]
    pub active_since: Option<String>,
}

impl Task {
    /// 计算任务是否逾期
    pub fn compute_overdue(&self) -> bool {
        if self.status != TaskStatus::Pending {
            return false;
        }

        self.due.as_deref().map_or(false, |d: &str| {
            chrono::DateTime::parse_from_rfc3339(d)
                .map(|dt| dt < chrono::Utc::now())
                .unwrap_or(false)
        })
    }

    /// 计算任务是否今天到期
    pub fn compute_due_today(&self) -> bool {
        if self.status != TaskStatus::Pending {
            return false;
        }

        self.due.as_deref().map_or(false, |d| {
            chrono::DateTime::parse_from_rfc3339(d)
                .map(|dt| {
                    let delta = dt.signed_duration_since(chrono::Utc::now());
                    delta.num_seconds() >= 0 && delta.num_hours() < 24
                })
                .unwrap_or(false)
        })
    }
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
