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

/// 周期性任务的重复规则，存成 JSON 落进 tasks.recur_rule
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RecurRule {
    /// 每 interval 天重复一次
    Daily { interval: u32 },
    /// 每周固定星期几重复，weekdays: 0=周一..6=周日
    Weekly { weekdays: Vec<u8> },
    /// 每月固定第几天重复，超出当月天数时按月末截断（如 31 号在二月按 28/29 号算）
    Monthly { day: u8 },
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

    /// 标记为"今日任务"时的日期（UTC，YYYY-MM-DD）；过了这一天会在下次读取时自动清除
    #[serde(skip_serializing)]
    pub today_marked_date: Option<String>,

    /// 派生字段 (不入库，由 API 层计算后得出)
    pub blocking: Vec<String>,
    pub is_overdue: bool,
    pub is_due_today: bool,
    pub is_locked: bool,

    /// 派生字段：是否被标记为"今日任务"（由 today_marked_date 是否等于今天算出）
    #[serde(default)]
    pub is_today: bool,

    /// 派生字段：累计花费在该任务上的秒数（含正在进行的计时段）
    #[serde(default)]
    pub total_seconds: i64,

    /// 派生字段：该任务当前是否正在计时
    #[serde(default)]
    pub is_timing: bool,

    /// 派生字段：若正在计时，这一段计时的开始时间
    #[serde(default)]
    pub active_since: Option<String>,

    /// 单个 emoji，用于日历页打卡展示；未设置为 None
    pub icon: Option<String>,
    /// 十六进制颜色（如 #8250df），用于日历页打卡点/图标着色；未设置为 None
    pub color: Option<String>,
    /// 周期性规则；None 表示不是周期性任务
    pub recur_rule: Option<RecurRule>,

    /// 派生字段：recur_rule.is_some() 的语法糖，方便前端判断
    #[serde(default)]
    pub is_recurring: bool,
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
