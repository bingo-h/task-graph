//! 从数据库任务列表构建项目树

use std::collections::HashMap;

use crate::models::project::ProjectNode;
use crate::models::task::Task;

/// 无项目归属任务的虚拟项目路径标识符。
const INBOX_PROJECT: &str = "无项目";

/// 从任务列表构建项目树，返回节点字典和根节点列表
pub fn build(tasks: &[Task]) -> (HashMap<String, ProjectNode>, Vec<String>) {
    let mut nodes: HashMap<String, ProjectNode> = HashMap::new();

    // Step 1: 为每个路径段创建节点
    for task in tasks {
        let Some(project) = &task.project else {
            continue;
        };
        let parts: Vec<&str> = project.split('.').collect();

        for i in 1..=parts.len() {
            let path = parts[..i].join(".");

            nodes
                .entry(path.clone())
                .or_insert_with(|| ProjectNode::new(path.clone(), parts[i - 1].to_string(), i - 1));
        }
    }

    // Step 2: 连接父子关系
    let paths: Vec<String> = nodes.keys().cloned().collect();
    for path in &paths {
        let parts: Vec<&str> = path.split('.').collect();

        if parts.len() < 2 {
            continue;
        }

        let parent = parts[..parts.len() - 1].join(".");
        if let Some(parent_node) = nodes.get_mut(&parent) {
            if !parent_node.children.contains(path) {
                parent_node.children.push(path.clone());
            }
        }
    }

    // Step 3: 聚合任务计数
    for task in tasks {
        let Some(project) = &task.project else {
            continue;
        };

        if let Some(node) = nodes.get_mut(project) {
            match task.status.as_str() {
                "pending" => {
                    node.pending_count += 1;

                    if task.is_overdue {
                        node.overdue_count += 1;
                    }
                    if task.is_locked {
                        node.locked_count += 1;
                    }
                }

                "completed" => node.completed_count += 1,

                "waiting" => node.waiting_count += 1,

                _ => {}
            }
        }
    }

    // Step 4: 从深到浅向上传播计数
    let mut sorted: Vec<String> = nodes.keys().cloned().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.chars().filter(|&c| c == '.').count()));
    for path in &sorted {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() < 2 {
            continue;
        }

        let parent = parts[..parts.len() - 1].join(".");
        let (p, c, w, o, l) = {
            let n = &nodes[path];
            (
                n.pending_count,
                n.completed_count,
                n.waiting_count,
                n.overdue_count,
                n.locked_count,
            )
        };

        if let Some(pn) = nodes.get_mut(&parent) {
            pn.pending_count += p;
            pn.completed_count += c;
            pn.waiting_count += w;
            pn.overdue_count += o;
            pn.locked_count += l;
        }
    }

    // Step 5: 收集根节点
    let mut roots: Vec<String> = nodes.keys().filter(|p| !p.contains('.')).cloned().collect();
    roots.sort();

    // Step 6: 无项目归属任务的虚拟节点
    let inbox: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.project.is_none() && t.status.as_str() != "deleted")
        .collect();

    if !inbox.is_empty() {
        let mut inbox_node = ProjectNode::new(INBOX_PROJECT.to_string(), "无项目".to_string(), 0);

        for t in &inbox {
            match t.status.as_str() {
                "pending" => {
                    inbox_node.pending_count += 1;
                    if t.is_overdue {
                        inbox_node.overdue_count += 1;
                    }
                    if t.is_locked {
                        inbox_node.locked_count += 1;
                    }
                }

                "completed" => inbox_node.completed_count += 1,

                "waiting" => inbox_node.waiting_count += 1,

                _ => {}
            }
        }

        nodes.insert(INBOX_PROJECT.to_string(), inbox_node);
        roots.insert(0, INBOX_PROJECT.to_string());
    }

    (nodes, roots)
}
