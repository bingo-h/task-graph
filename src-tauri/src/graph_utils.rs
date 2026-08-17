//! 纯图算法工具，不碰数据库/Tauri 类型
//!
//! 供"今日任务"手动排序校验用：判断一条新边是否会跟已有的边集合产生矛盾/成环。

use crate::models::task::Task;
use std::collections::{HashMap, HashSet};

/// 从任务列表的 depends 字段构建正向邻接表：prerequisite(source) -> [dependent(target)]
pub fn forward_adjacency(tasks: &[Task]) -> HashMap<String, Vec<String>> {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for task in tasks {
        for dep_uuid in &task.depends {
            adj.entry(dep_uuid.clone()).or_default().push(task.uuid.clone());
        }
    }
    adj
}

/// 从 (from, to) 边对列表构建正向邻接表
pub fn adjacency_from_pairs(pairs: &[(String, String)]) -> HashMap<String, Vec<String>> {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for (from, to) in pairs {
        adj.entry(from.clone()).or_default().push(to.clone());
    }
    adj
}

/// BFS 判断从 `from` 沿邻接表能否到达 `to`
pub fn reachable(adj: &HashMap<String, Vec<String>>, from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }

    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: Vec<&str> = vec![from];

    while let Some(node) = queue.pop() {
        if !visited.insert(node) {
            continue;
        }
        if node == to {
            return true;
        }
        if let Some(next) = adj.get(node) {
            queue.extend(next.iter().map(String::as_str));
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reachable_finds_transitive_path() {
        let mut adj = HashMap::new();
        adj.insert("a".to_string(), vec!["b".to_string()]);
        adj.insert("b".to_string(), vec!["c".to_string()]);

        assert!(reachable(&adj, "a", "c"));
        assert!(!reachable(&adj, "c", "a"));
    }

    #[test]
    fn reachable_self_is_true() {
        let adj = HashMap::new();
        assert!(reachable(&adj, "a", "a"));
    }
}
