//! 项目树节点

use serde::Serialize;

/// 项目阶段：计划中
pub const STAGE_PLANNED: &str = "planned";
/// 项目阶段：进行中。未显式设置过阶段的项目默认视为"进行中"。
pub const STAGE_ACTIVE: &str = "active";

/// 序列化后的项目树节点
#[derive(Debug, Clone, Serialize)]
pub struct ProjectNode {
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub children: Vec<String>,

    /// 自身或祖先被归档时为 true（级联归档的有效状态）
    pub archived: bool,
    /// 该路径自身是否被显式归档（用于判断能否在此节点直接切换归档状态）
    pub self_archived: bool,
    /// 项目所属阶段："planned"（计划中）或 "active"（进行中），不级联，默认 "active"
    pub stage: String,

    /// 自身或祖先在废纸篓中时为 true（级联，优先级高于归档/阶段分组）
    pub trashed: bool,
    /// 该路径自身是否被显式移入废纸篓（用于判断能否在此节点直接恢复）
    pub self_trashed: bool,
    /// 移入废纸篓的时间（仅 self_trashed 时有值），用于计算自动彻底删除的剩余时间
    pub trashed_at: Option<String>,

    pub pending_count: usize,
    pub completed_count: usize,
    pub waiting_count: usize,
    pub overdue_count: usize,
    pub locked_count: usize,
}

impl ProjectNode {
    pub fn new(path: String, name: String, depth: usize) -> Self {
        ProjectNode {
            path,
            name,
            depth,
            children: Vec::new(),
            archived: false,
            self_archived: false,
            stage: STAGE_ACTIVE.to_string(),
            trashed: false,
            self_trashed: false,
            trashed_at: None,
            pending_count: 0,
            completed_count: 0,
            waiting_count: 0,
            overdue_count: 0,
            locked_count: 0,
        }
    }
}
