//! 任务计时记录数据结构

use serde::{Deserialize, Serialize};

/// 一段计时记录：start 到 end 之间花费在某个任务上的时间
/// end 为 None 表示这段计时仍在进行中
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: i64,
    pub task_uuid: String,
    pub start: String,
    pub end: Option<String>,
}
