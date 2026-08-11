//! 项目树节点

use serde::Serialize;

/// 序列化后的项目树节点
#[derive(Debug, Clone, Serialize)]
pub struct ProjectNode {
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub children: Vec<String>,

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
            pending_count: 0,
            completed_count: 0,
            waiting_count: 0,
            overdue_count: 0,
            locked_count: 0,
        }
    }
}
