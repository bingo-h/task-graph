//! 通用撤销/重做栈。
//!
//! 任务类操作（新建/编辑/删除/完成/依赖）走"行快照"：操作前后各拍一次
//! `tasks` 表整行快照，撤销=覆写回 before（before=None 表示这行原本不存在，
//! 撤销时直接删掉），重做=覆写回 after。动态读取全部列，不逐列硬编码结构体，
//! 这样以后 `tasks` 表加新列不需要同步改这里。
//!
//! 项目结构类操作（改名/移动/归档/阶段/废纸篓/新建）走"逆操作"：`rename_project`/
//! `move_project` 会级联影响整棵子树、且改名本身会改变 `projects.path` 这个主键，
//! 用行快照会导致"快照前的主键"和"快照后的主键"对不上；这类操作现有的
//! `db::project::*` 函数本身就有清晰的逆操作（改名的逆操作是改回原名，移动的
//! 逆操作是移回原父级……），记录"撤销时要调用哪个函数、传什么参数"比记录
//! 一整棵子树的 before/after 快照简单可靠得多。

use rusqlite::Connection;
use serde_json::Map;

/// 任务表某一行的完整快照（全部列，动态读取）
pub type TaskRowSnapshot = Map<String, serde_json::Value>;

/// 读取 `tasks` 表某一行的完整快照；这一行不存在时返回 `None`
pub fn capture_task_row(conn: &Connection, uuid: &str) -> anyhow::Result<Option<TaskRowSnapshot>> {
    let mut stmt = conn.prepare("SELECT * FROM tasks WHERE uuid = ?1")?;
    let column_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = stmt.query(rusqlite::params![uuid])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let mut map = Map::new();
    for (i, name) in column_names.iter().enumerate() {
        let value: rusqlite::types::Value = row.get(i)?;
        let json_value = match value {
            rusqlite::types::Value::Null => serde_json::Value::Null,
            rusqlite::types::Value::Integer(n) => serde_json::Value::from(n),
            rusqlite::types::Value::Real(f) => serde_json::Value::from(f),
            rusqlite::types::Value::Text(s) => serde_json::Value::from(s),
            // tasks 表没有 BLOB 列，不会真的走到这里，纯防御性兜底
            rusqlite::types::Value::Blob(_) => serde_json::Value::Null,
        };
        map.insert(name.clone(), json_value);
    }
    Ok(Some(map))
}

fn json_to_sql_value(v: &serde_json::Value) -> rusqlite::types::Value {
    match v {
        serde_json::Value::Null => rusqlite::types::Value::Null,
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else {
                rusqlite::types::Value::Real(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        // tasks 表列不会产生 Bool/Array/Object，纯防御性兜底
        _ => rusqlite::types::Value::Null,
    }
}

/// 把一份快照整行写回 `tasks` 表（`uuid` 列本身也在快照里，作为主键覆写）。
/// 用快照自带的列集合动态拼 INSERT OR REPLACE，不假设固定的列顺序/数量——
/// `urgency` 列虽然是 REAL，即使这里传 Integer 变体，SQLite 的列类型亲和性
/// 也会把它转换成 REAL 存储，不用在这里手动区分整数/浮点。
pub fn restore_task_row(conn: &Connection, snapshot: &TaskRowSnapshot) -> anyhow::Result<()> {
    let columns: Vec<&str> = snapshot.keys().map(|k| k.as_str()).collect();
    let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "INSERT OR REPLACE INTO tasks ({}) VALUES ({})",
        columns.join(", "),
        placeholders.join(", ")
    );
    let values: Vec<rusqlite::types::Value> =
        columns.iter().map(|k| json_to_sql_value(&snapshot[*k])).collect();
    conn.execute(&sql, rusqlite::params_from_iter(values))?;
    Ok(())
}

/// 一步可撤销/重做的操作
pub enum UndoAction {
    /// 任务字段类：一次操作影响的一组任务行（单个操作时这个 Vec 只有一项；
    /// 批量操作如 done_tasks/delete_tasks/set_tasks_project 会把整批放进同一条
    /// UndoAction，撤销时一次性全部还原，而不是要按 N 次 Ctrl+Z）。
    /// before=None 表示这个 uuid 在操作前不存在（新建任务）；
    /// 删除是软删除（mark_deleted 只改 status），所以删除操作的 before/after 都是 Some。
    TaskRow {
        label: String,
        rows: Vec<(String, Option<TaskRowSnapshot>, Option<TaskRowSnapshot>)>,
    },
    /// 项目结构类：记录撤销/重做时各自要调用哪个 db::project 函数、传什么参数
    ProjectInverse {
        label: String,
        undo: ProjectCall,
        redo: ProjectCall,
    },
}

/// 项目结构类操作的一次调用描述，映射回 `db::project::*` 对应的函数
pub enum ProjectCall {
    Create { path: String, stage: String },
    Purge { path: String },
    /// 撤销"新建项目"专用逆操作：只精确删这一条项目记录（见
    /// `db::project::delete_record` 的文档注释），不走 `Purge` 那种 LIKE
    /// 前缀级联匹配，避免误删无关项目的子树
    DeleteRecord { path: String },
    Rename { path: String, new_name: String },
    Move { path: String, new_parent: Option<String> },
    SetArchived { path: String, archived: bool },
    SetStage { path: String, stage: String },
    Trash { path: String },
    Restore { path: String },
}

impl ProjectCall {
    pub fn apply(&self, conn: &Connection) -> anyhow::Result<()> {
        match self {
            ProjectCall::Create { path, stage } => crate::db::project::create(conn, path, stage),
            ProjectCall::Purge { path } => crate::db::project::purge(conn, path),
            ProjectCall::DeleteRecord { path } => crate::db::project::delete_record(conn, path),
            ProjectCall::Rename { path, new_name } => {
                crate::db::project::rename_project(conn, path, new_name).map(|_| ())
            }
            ProjectCall::Move { path, new_parent } => {
                crate::db::project::move_project(conn, path, new_parent.as_deref()).map(|_| ())
            }
            ProjectCall::SetArchived { path, archived } => {
                crate::db::project::set_archived(conn, path, *archived)
            }
            ProjectCall::SetStage { path, stage } => crate::db::project::set_stage(conn, path, stage),
            ProjectCall::Trash { path } => crate::db::project::trash(conn, path),
            ProjectCall::Restore { path } => crate::db::project::restore(conn, path),
        }
    }
}

/// 只把 `from` 和 `to` 两份快照之间真正不同的列，覆写成 `to` 的值——不是
/// 无差别整行 REPLACE。这样撤销/重做一步操作，不会连带冲掉同一行上由
/// 其它不在撤销范围内的操作（今日标记、开始计时、重复规则……）造成的改动。
/// `to` 为 None 删除这一行（撤销"新建"）；这一行当前不存在时（重做"新建"）
/// 退化成整行 INSERT（此时没有旧值可比较，也没必要比较）。
fn apply_task_row_diff(
    conn: &Connection,
    uuid: &str,
    from: &Option<TaskRowSnapshot>,
    to: &Option<TaskRowSnapshot>,
) -> anyhow::Result<()> {
    match to {
        None => {
            conn.execute("DELETE FROM tasks WHERE uuid = ?1", rusqlite::params![uuid])?;
        }
        Some(to_snapshot) => match from {
            None => {
                restore_task_row(conn, to_snapshot)?;
            }
            Some(from_snapshot) => {
                let changed: Vec<&str> = to_snapshot
                    .keys()
                    .filter(|k| from_snapshot.get(*k) != to_snapshot.get(*k))
                    .map(|k| k.as_str())
                    .collect();
                if changed.is_empty() {
                    return Ok(());
                }
                let set_clause: Vec<String> = changed
                    .iter()
                    .enumerate()
                    .map(|(i, col)| format!("{col} = ?{}", i + 1))
                    .collect();
                let sql = format!(
                    "UPDATE tasks SET {} WHERE uuid = ?{}",
                    set_clause.join(", "),
                    changed.len() + 1
                );
                let mut values: Vec<rusqlite::types::Value> =
                    changed.iter().map(|k| json_to_sql_value(&to_snapshot[*k])).collect();
                values.push(rusqlite::types::Value::Text(uuid.to_string()));
                conn.execute(&sql, rusqlite::params_from_iter(values))?;
            }
        },
    }
    Ok(())
}

fn apply_task_rows(
    conn: &Connection,
    rows: &[(String, Option<TaskRowSnapshot>, Option<TaskRowSnapshot>)],
    reverse: bool,
) -> anyhow::Result<()> {
    for (uuid, before, after) in rows {
        let (from, to) = if reverse { (after, before) } else { (before, after) };
        apply_task_row_diff(conn, uuid, from, to)?;
    }
    Ok(())
}

/// 撤销一步操作：任务类覆写回 before（None 表示删行）；项目类调用 undo 字段
pub fn apply_reverse(conn: &Connection, action: &UndoAction) -> anyhow::Result<()> {
    match action {
        UndoAction::TaskRow { rows, .. } => apply_task_rows(conn, rows, true),
        UndoAction::ProjectInverse { undo, .. } => undo.apply(conn),
    }
}

/// 重做一步操作：任务类覆写回 after（None 表示删行，理论上不会命中，因为本计划
/// 范围内的删除都是软删除，after 永远是 Some；保留这个分支只是为了跟 apply_reverse
/// 对称、逻辑自洽）；项目类调用 redo 字段
pub fn apply_forward(conn: &Connection, action: &UndoAction) -> anyhow::Result<()> {
    match action {
        UndoAction::TaskRow { rows, .. } => apply_task_rows(conn, rows, false),
        UndoAction::ProjectInverse { redo, .. } => redo.apply(conn),
    }
}

/// 撤销/重做栈：只在内存里，应用重启即清空，不持久化到磁盘
#[derive(Default)]
pub struct UndoStack {
    undo: Vec<UndoAction>,
    redo: Vec<UndoAction>,
}

pub const MAX_DEPTH: usize = 50;

impl UndoStack {
    /// 用户做了一步会被记录的新操作：压入 undo 栈，清空 redo 栈——
    /// 标准桌面应用行为，做了新操作后，之前撤销出来的"重做"分支全部作废
    pub fn push(&mut self, action: UndoAction) {
        self.undo.push(action);
        if self.undo.len() > MAX_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// 弹出最近一步待撤销的操作（undo 命令用）
    pub fn pop_undo(&mut self) -> Option<UndoAction> {
        self.undo.pop()
    }

    /// undo 命令执行完撤销后，把这个动作放进 redo 栈顶——不清空 undo 栈
    pub fn push_redo(&mut self, action: UndoAction) {
        self.redo.push(action);
    }

    /// 弹出最近一步待重做的操作（redo 命令用）
    pub fn pop_redo(&mut self) -> Option<UndoAction> {
        self.redo.pop()
    }

    /// redo 命令执行完重做后，把这个动作放回 undo 栈顶——**不能调用 push()**，
    /// 那个方法会清空 redo 栈，而这里 redo 栈里可能还有更早的、用户还没重做完
    /// 的步骤，不应该被这一次 redo 顺带清掉（见 Global Constraints 的说明）
    pub fn push_undo(&mut self, action: UndoAction) {
        self.undo.push(action);
        if self.undo.len() > MAX_DEPTH {
            self.undo.remove(0);
        }
    }

    /// 清空撤销/重做栈——彻底删除（purge_project）之后调用，避免后续 Ctrl+Z
    /// 用旧的快照/逆操作把已经被用户明确确认永久删除的数据复活回来
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::task::CreateTaskRequest;
    use rusqlite::{params, Connection};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::schema::init(&conn).unwrap();
        conn
    }

    fn make_task(conn: &Connection, description: &str) -> crate::models::task::Task {
        db::task::create(
            conn,
            &CreateTaskRequest {
                description: description.to_string(),
                project: Some("工作".into()),
                priority: Some("H".into()),
                due: None,
                scheduled: None,
                started_at: None,
                tags: vec![],
                depends: vec![],
                annotation: None,
                icon: None,
                color: None,
                recur_rule: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn capture_task_row_returns_none_for_missing_uuid() {
        let conn = test_conn();
        assert!(capture_task_row(&conn, "does-not-exist").unwrap().is_none());
    }

    #[test]
    fn capture_and_restore_round_trips_a_task_row() {
        let conn = test_conn();
        let task = make_task(&conn, "写报告");

        let before = capture_task_row(&conn, &task.uuid).unwrap().unwrap();

        conn.execute(
            "UPDATE tasks SET description = ?2 WHERE uuid = ?1",
            params![task.uuid, "改过的描述"],
        )
        .unwrap();
        let changed = db::task::get_by_uuid(&conn, &task.uuid).unwrap().unwrap();
        assert_eq!(changed.description, "改过的描述");

        restore_task_row(&conn, &before).unwrap();
        let restored = db::task::get_by_uuid(&conn, &task.uuid).unwrap().unwrap();
        assert_eq!(restored.description, "写报告");
        assert_eq!(restored.priority.map(|p| p.as_str().to_string()), Some("H".to_string()));
    }

    #[test]
    fn apply_reverse_of_task_row_with_before_none_deletes_the_row() {
        let conn = test_conn();
        let task = make_task(&conn, "临时任务");
        let after = capture_task_row(&conn, &task.uuid).unwrap();

        let action = UndoAction::TaskRow {
            label: "新建任务".into(),
            rows: vec![(task.uuid.clone(), None, after)],
        };
        apply_reverse(&conn, &action).unwrap();

        assert!(db::task::get_by_uuid(&conn, &task.uuid).unwrap().is_none());
    }

    #[test]
    fn apply_forward_of_task_row_recreates_the_same_uuid() {
        let conn = test_conn();
        let task = make_task(&conn, "临时任务");
        let after = capture_task_row(&conn, &task.uuid).unwrap();
        let uuid = task.uuid.clone();

        let action = UndoAction::TaskRow {
            label: "新建任务".into(),
            rows: vec![(uuid.clone(), None, after)],
        };
        apply_reverse(&conn, &action).unwrap();
        assert!(db::task::get_by_uuid(&conn, &uuid).unwrap().is_none());

        apply_forward(&conn, &action).unwrap();
        let recreated = db::task::get_by_uuid(&conn, &uuid).unwrap().unwrap();
        assert_eq!(recreated.uuid, uuid);
        assert_eq!(recreated.description, "临时任务");
    }

    #[test]
    fn apply_reverse_of_task_row_batch_restores_every_row() {
        let conn = test_conn();
        let t1 = make_task(&conn, "任务一");
        let t2 = make_task(&conn, "任务二");
        let before1 = capture_task_row(&conn, &t1.uuid).unwrap();
        let before2 = capture_task_row(&conn, &t2.uuid).unwrap();

        db::task::mark_done(&conn, &t1.uuid).unwrap();
        db::task::mark_done(&conn, &t2.uuid).unwrap();
        let after1 = capture_task_row(&conn, &t1.uuid).unwrap();
        let after2 = capture_task_row(&conn, &t2.uuid).unwrap();

        let action = UndoAction::TaskRow {
            label: "批量完成".into(),
            rows: vec![(t1.uuid.clone(), before1, after1), (t2.uuid.clone(), before2, after2)],
        };
        apply_reverse(&conn, &action).unwrap();

        assert_eq!(
            db::task::get_by_uuid(&conn, &t1.uuid).unwrap().unwrap().status.as_str(),
            "pending"
        );
        assert_eq!(
            db::task::get_by_uuid(&conn, &t2.uuid).unwrap().unwrap().status.as_str(),
            "pending"
        );
    }

    #[test]
    fn project_inverse_rename_round_trips() {
        let conn = test_conn();
        db::project::create(&conn, "工作", "active").unwrap();

        let action = UndoAction::ProjectInverse {
            label: "重命名项目".into(),
            undo: ProjectCall::Rename { path: "副业".into(), new_name: "工作".into() },
            redo: ProjectCall::Rename { path: "工作".into(), new_name: "副业".into() },
        };

        apply_forward(&conn, &action).unwrap(); // 模拟"重做"：工作 -> 副业
        let paths: Vec<String> =
            db::project::list_all(&conn).unwrap().into_iter().map(|p| p.path).collect();
        assert!(paths.contains(&"副业".to_string()));
        assert!(!paths.contains(&"工作".to_string()));

        apply_reverse(&conn, &action).unwrap(); // 撤销回工作
        let paths: Vec<String> =
            db::project::list_all(&conn).unwrap().into_iter().map(|p| p.path).collect();
        assert!(paths.contains(&"工作".to_string()));
    }

    #[test]
    fn project_rename_undo_also_restores_child_tasks_project_field() {
        let conn = test_conn();
        db::project::create(&conn, "工作", "active").unwrap();
        let task = db::task::create(
            &conn,
            &CreateTaskRequest {
                description: "任务".into(),
                project: Some("工作".into()),
                priority: None,
                due: None,
                scheduled: None,
                started_at: None,
                tags: vec![],
                depends: vec![],
                annotation: None,
                icon: None,
                color: None,
                recur_rule: None,
            },
        )
        .unwrap();

        let new_path = db::project::rename_project(&conn, "工作", "副业").unwrap();
        assert_eq!(new_path, "副业");
        assert_eq!(
            db::task::get_by_uuid(&conn, &task.uuid).unwrap().unwrap().project,
            Some("副业".to_string())
        );

        let action = UndoAction::ProjectInverse {
            label: "重命名项目".into(),
            undo: ProjectCall::Rename { path: "副业".into(), new_name: "工作".into() },
            redo: ProjectCall::Rename { path: "工作".into(), new_name: "副业".into() },
        };
        apply_reverse(&conn, &action).unwrap();

        assert_eq!(
            db::task::get_by_uuid(&conn, &task.uuid).unwrap().unwrap().project,
            Some("工作".to_string())
        );
    }

    #[test]
    fn create_project_undo_does_not_purge_case_colliding_sibling() {
        let conn = test_conn();
        // 既有项目 "work.meetings"，其下挂着一个任务——路径和后面要撤销新建的
        // "Work" 只有大小写不同，SQLite 的 LIKE 默认对 ASCII 大小写不敏感，
        // "Work.%" 会误匹配到 "work.meetings"
        db::project::create(&conn, "work.meetings", "active").unwrap();
        let existing_task = db::task::create(
            &conn,
            &CreateTaskRequest {
                description: "既有任务".into(),
                project: Some("work.meetings".into()),
                priority: None,
                due: None,
                scheduled: None,
                started_at: None,
                tags: vec![],
                depends: vec![],
                annotation: None,
                icon: None,
                color: None,
                recur_rule: None,
            },
        )
        .unwrap();

        // 模拟 create_project("Work") 之后紧接着撤销：undo 字段现在是
        // DeleteRecord，不再是 Purge
        db::project::create(&conn, "Work", "active").unwrap();
        let action = UndoAction::ProjectInverse {
            label: "新建项目".into(),
            undo: ProjectCall::DeleteRecord { path: "Work".into() },
            redo: ProjectCall::Create { path: "Work".into(), stage: "active".into() },
        };
        apply_reverse(&conn, &action).unwrap();

        let paths: Vec<String> =
            db::project::list_all(&conn).unwrap().into_iter().map(|p| p.path).collect();
        // "Work" 这条记录被精确删除
        assert!(!paths.contains(&"Work".to_string()));
        // "work.meetings" 项目记录和它下面的任务完全不受影响
        assert!(paths.contains(&"work.meetings".to_string()));
        let reloaded = db::task::get_by_uuid(&conn, &existing_task.uuid).unwrap().unwrap();
        assert_eq!(reloaded.status.as_str(), "pending");
        assert_eq!(reloaded.project, Some("work.meetings".to_string()));
    }

    #[test]
    fn apply_reverse_preserves_untracked_column_changes_made_in_between() {
        let conn = test_conn();
        let task = make_task(&conn, "原始描述");

        // 拍下"编辑描述"这一步操作的 before 快照
        let before = capture_task_row(&conn, &task.uuid).unwrap();
        conn.execute(
            "UPDATE tasks SET description = ?2 WHERE uuid = ?1",
            params![task.uuid, "改过的描述"],
        )
        .unwrap();
        let after = capture_task_row(&conn, &task.uuid).unwrap();

        // 编辑之后、撤销之前，发生一次不在撤销范围内的操作：标记为"今日任务"
        conn.execute(
            "UPDATE tasks SET today_marked_date = ?2 WHERE uuid = ?1",
            params![task.uuid, "2026-01-01"],
        )
        .unwrap();

        let action = UndoAction::TaskRow {
            label: "编辑任务".into(),
            rows: vec![(task.uuid.clone(), before, after)],
        };
        apply_reverse(&conn, &action).unwrap();

        let restored = db::task::get_by_uuid(&conn, &task.uuid).unwrap().unwrap();
        // 描述正确回退
        assert_eq!(restored.description, "原始描述");
        // 但"今日标记"这个未被追踪的改动不应该被整行覆写冲掉
        assert_eq!(restored.today_marked_date, Some("2026-01-01".to_string()));
    }

    #[test]
    fn undo_stack_push_clears_redo_but_push_undo_does_not() {
        let mut stack = UndoStack::default();
        let a = UndoAction::TaskRow { label: "a".into(), rows: vec![] };
        let b = UndoAction::TaskRow { label: "b".into(), rows: vec![] };
        let c = UndoAction::TaskRow { label: "c".into(), rows: vec![] };

        stack.push(a);
        stack.push(b);
        // 撤销两步：undo 栈空，redo 栈有 2 项
        let undone_b = stack.pop_undo().unwrap();
        stack.push_redo(undone_b);
        let undone_a = stack.pop_undo().unwrap();
        stack.push_redo(undone_a);
        assert!(stack.pop_undo().is_none());

        // 重做一步：从 redo 弹一个、push_undo 放回 undo 栈——redo 栈里应该还剩一个
        let redone = stack.pop_redo().unwrap();
        stack.push_undo(redone);
        assert!(stack.pop_redo().is_some(), "push_undo 不应该清空 redo 栈");

        // 而 push()（模拟用户做了全新操作）必须清空 redo
        stack.push(c);
        assert!(stack.pop_redo().is_none(), "push() 必须清空 redo 栈");
    }

    #[test]
    fn undo_stack_respects_max_depth() {
        let mut stack = UndoStack::default();
        for i in 0..(MAX_DEPTH + 5) {
            stack.push(UndoAction::TaskRow { label: i.to_string(), rows: vec![] });
        }
        let mut count = 0;
        while stack.pop_undo().is_some() {
            count += 1;
        }
        assert_eq!(count, MAX_DEPTH);
    }

    #[test]
    fn undo_stack_clear_empties_both_undo_and_redo() {
        let mut stack = UndoStack::default();
        stack.push(UndoAction::TaskRow { label: "a".into(), rows: vec![] });
        stack.push_redo(UndoAction::TaskRow { label: "b".into(), rows: vec![] });

        stack.clear();

        assert!(stack.pop_undo().is_none());
        assert!(stack.pop_redo().is_none());
    }

    #[test]
    fn simulated_add_then_modify_then_undo_both_restores_original_state() {
        let conn = test_conn();
        let mut stack = UndoStack::default();

        // 模拟 add_task：新建后 push 一条 before=None 的 UndoAction
        let task = make_task(&conn, "原始描述");
        let after_add = capture_task_row(&conn, &task.uuid).unwrap();
        stack.push(UndoAction::TaskRow {
            label: "新建任务".into(),
            rows: vec![(task.uuid.clone(), None, after_add)],
        });

        // 模拟 modify_task：改描述前后各拍一次
        let before_modify = capture_task_row(&conn, &task.uuid).unwrap();
        conn.execute(
            "UPDATE tasks SET description = ?2 WHERE uuid = ?1",
            params![task.uuid, "改过的描述"],
        )
        .unwrap();
        let after_modify = capture_task_row(&conn, &task.uuid).unwrap();
        stack.push(UndoAction::TaskRow {
            label: "编辑任务".into(),
            rows: vec![(task.uuid.clone(), before_modify, after_modify)],
        });

        // 撤销一次：应该回到"改过的描述"之前，即"原始描述"
        let action = stack.pop_undo().unwrap();
        apply_reverse(&conn, &action).unwrap();
        assert_eq!(
            db::task::get_by_uuid(&conn, &task.uuid).unwrap().unwrap().description,
            "原始描述"
        );
        stack.push_redo(action);

        // 再撤销一次：应该整行消失（回到"新建"之前）
        let action = stack.pop_undo().unwrap();
        apply_reverse(&conn, &action).unwrap();
        assert!(db::task::get_by_uuid(&conn, &task.uuid).unwrap().is_none());
        stack.push_redo(action);

        // 全部重做：应该回到"改过的描述"
        let action = stack.pop_redo().unwrap();
        apply_forward(&conn, &action).unwrap();
        stack.push_undo(action);
        let action = stack.pop_redo().unwrap();
        apply_forward(&conn, &action).unwrap();
        stack.push_undo(action);
        assert_eq!(
            db::task::get_by_uuid(&conn, &task.uuid).unwrap().unwrap().description,
            "改过的描述"
        );
    }
}
