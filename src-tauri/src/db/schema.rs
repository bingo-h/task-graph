//! 数据库表结构定义
//!
//! 使用简单的版本号机制管理 schema 迁移
//! 每次升级只需在 MIGRATIONS 数组里追加新的 SQL。

use anyhow::Result;
use rusqlite::Connection;

/// 所有迁移 SQL ，按版本号顺序排列
/// 版本号从 1 开始，数组下标 0 对应版本 1
/// 只追加，不修改已有条目
const MIGRATIONS: &[&str] = &[
    // 版本 1: 初始化表结构
    r#"
    CREATE TABLE IF NOT EXISTS tasks (
        uuid        TEXT PRIMARY KEY,
        description TEXT NOT NULL,
        status      TEXT NOT NULL DEFAULT 'pending',
        project     TEXT,
        priority    TEXT,
        urgency     REAL NOT NULL DEFAULT 0,
        due         TEXT,
        scheduled   TEXT,
        created_at  TEXT NOT NULL,
        end         TEXT,
        tags        TEXT NOT NULL DEFAULT '[]',
        depends     TEXT NOT NULL DEFAULT '[]',
        annotations TEXT NOT NULL DEFAULT '[]'
    );
    
    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER NOT NULL
    );
    "#,
    // 版本 2: 任务计时记录表
    r#"
    CREATE TABLE IF NOT EXISTS time_entries (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        task_uuid   TEXT NOT NULL,
        start       TEXT NOT NULL,
        end         TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_time_entries_task_uuid
        ON time_entries (task_uuid);
    "#,
    // 版本 3: 项目表，允许项目独立于任务存在
    r#"
    CREATE TABLE IF NOT EXISTS projects (
        path       TEXT PRIMARY KEY,
        created_at TEXT NOT NULL
    );
    "#,
    // 版本 4: 项目归档状态
    r#"
    ALTER TABLE projects ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
    "#,
    // 版本 5: 项目阶段（计划中 / 进行中），未设置时默认视为进行中
    r#"
    ALTER TABLE projects ADD COLUMN stage TEXT NOT NULL DEFAULT 'active';
    "#,
    // 版本 6: 项目废纸篓（软删除），trashed_at 用于计算自动彻底删除的到期时间
    r#"
    ALTER TABLE projects ADD COLUMN trashed INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE projects ADD COLUMN trashed_at TEXT;
    "#,
    // 版本 7: 计时记录的回忆总结（每段专注结束时可填写标题+正文，事后也能修改）
    r#"
    ALTER TABLE time_entries ADD COLUMN note_title TEXT;
    ALTER TABLE time_entries ADD COLUMN note_body TEXT;
    "#,
    // 版本 8: "今日任务"标记，记录标记时的日期（UTC），过了这一天会在下次读取时自动清除
    r#"
    ALTER TABLE tasks ADD COLUMN today_marked_date TEXT;
    "#,
    // 版本 9: 标签改为独立的 tags 表（名字 + 颜色）+ task_tags 多对多关联表，
    // 取代原来存在 tasks.tags 里的 JSON 数组，这样改名/合并标签只需改一行，
    // 也能给标签加颜色、按标签索引任务。
    // 旧数据从 tasks.tags 搬过来的那一步在 init() 里紧跟着这条迁移执行
    // （必须赶在版本 10 把 tasks.tags 列删掉之前完成）。
    r#"
    CREATE TABLE IF NOT EXISTS tags (
        name  TEXT PRIMARY KEY,
        color TEXT
    );

    CREATE TABLE IF NOT EXISTS task_tags (
        task_uuid TEXT NOT NULL,
        tag_name  TEXT NOT NULL,
        PRIMARY KEY (task_uuid, tag_name)
    );

    CREATE INDEX IF NOT EXISTS idx_task_tags_tag_name ON task_tags (tag_name);
    "#,
    // 版本 10: 标签数据已经搬到 tags/task_tags 表，删掉 tasks 表里废弃的旧 JSON 列
    r#"
    ALTER TABLE tasks DROP COLUMN tags;
    "#,
    // 版本 11: 任务图标/颜色（日历页打卡展示用）+ 周期性任务规则（JSON，见 models::task::RecurRule）
    r#"
    ALTER TABLE tasks ADD COLUMN icon TEXT;
    ALTER TABLE tasks ADD COLUMN color TEXT;
    ALTER TABLE tasks ADD COLUMN recur_rule TEXT;
    "#,
    // 版本 12: 周期性任务每个周期的完成结果记录，用于日历页计算"连续完成天数"。
    // 逾期后补做也算维持连续（outcome=completed_late），完全错过才算断掉（outcome=missed）。
    r#"
    CREATE TABLE IF NOT EXISTS recur_log (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        task_uuid    TEXT NOT NULL,
        cycle_due    TEXT NOT NULL,
        outcome      TEXT NOT NULL CHECK (outcome IN ('completed_on_time','completed_late','missed')),
        completed_at TEXT,
        recorded_at  TEXT NOT NULL,
        UNIQUE (task_uuid, cycle_due)
    );

    CREATE INDEX IF NOT EXISTS idx_recur_log_task_cycle
        ON recur_log (task_uuid, cycle_due DESC);
    "#,
    // 版本 13:"今日任务"视图下用户手动排的工作顺序，独立于真实的 depends 依赖图，
    // 只在按"今日任务"分类筛选时生效。from_uuid 应先于 to_uuid 完成。
    r#"
    CREATE TABLE IF NOT EXISTS today_order_edges (
        from_uuid  TEXT NOT NULL,
        to_uuid    TEXT NOT NULL,
        created_at TEXT NOT NULL,
        PRIMARY KEY (from_uuid, to_uuid)
    );

    CREATE INDEX IF NOT EXISTS idx_today_order_to ON today_order_edges (to_uuid);
    "#,
    // 版本 14: DAG 视图里、同一层级（dagre 同一 rank 列）任务之间的手动纵向顺序，
    // 独立于真实的 depends 依赖图，也独立于 today_order_edges。语义同样是链式的
    // "from 排在 to 上面"，一次拖拽重排会把涉及的整列节点重新写成一条新链
    // （见 db::sibling_order::replace_chain）。
    r#"
    CREATE TABLE IF NOT EXISTS sibling_order_edges (
        from_uuid  TEXT NOT NULL,
        to_uuid    TEXT NOT NULL,
        created_at TEXT NOT NULL,
        PRIMARY KEY (from_uuid, to_uuid)
    );

    CREATE INDEX IF NOT EXISTS idx_sibling_order_to ON sibling_order_edges (to_uuid);
    "#,
    // 版本 15: 无 DDL 变更，纯粹标记一次数据修复——重复任务的周期截止时间(due)
    // 早前一律按 UTC 当天 23:59:59 算，现在改成按本地日历天算。已经写进数据库的
    // 历史 due 是按旧逻辑算的，不会因为代码修好了就自动变对，这里借用 schema
    // 版本号机制保证只补算一次（见 init() 里版本 15 的特殊分支）。
    r#"
    "#,
];

/// 初始化数据库 schema ，自动执行尚未应用的迁移
pub fn init(conn: &Connection) -> Result<()> {
    // 获取当前 schema 的版本
    let current_version: i64 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    // 执行所有未应用的迁移
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version > current_version {
            conn.execute_batch(sql)?;

            // 版本 9 刚创建 tags/task_tags 表：趁 tasks.tags 列还没被
            // 版本 10 的 DROP COLUMN 删掉，立刻把旧数据搬过去
            if version == 9 {
                crate::db::tag::backfill_from_json(conn)?;
            }

            // 版本 15：把已有重复任务的 due 从旧的 UTC 日边界重新算成本地日边界，
            // 让时区修复立刻生效，不用再等一个用旧逻辑算出来的周期过去
            if version == 15 {
                crate::db::recur::backfill_local_due(conn)?;
            }

            // 更新版本号
            conn.execute("DELETE FROM schema_version", [])?;
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [version],
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    /// 模拟一个还停在版本 8（标签仍是 tasks.tags 里的 JSON 数组）的旧数据库，
    /// 验证升级到最新版本后：标签正确搬进 tags/task_tags 表、
    /// 有重复标签的任务不会产生重复关联、tasks.tags 列被删除、
    /// 且这个过程是幂等的（多跑几次 init 不会报错或产生副作用）。
    #[test]
    fn migrates_legacy_json_tags_into_relational_tables() {
        let conn = Connection::open_in_memory().unwrap();

        // 手搭一个版本 8 的旧 schema：tasks.tags 是 JSON 数组
        conn.execute_batch(
            r#"
            CREATE TABLE tasks (
                uuid        TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                status      TEXT NOT NULL DEFAULT 'pending',
                project     TEXT,
                priority    TEXT,
                urgency     REAL NOT NULL DEFAULT 0,
                due         TEXT,
                scheduled   TEXT,
                created_at  TEXT NOT NULL,
                end         TEXT,
                tags        TEXT NOT NULL DEFAULT '[]',
                depends     TEXT NOT NULL DEFAULT '[]',
                annotations TEXT NOT NULL DEFAULT '[]',
                today_marked_date TEXT
            );
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (8);
            "#,
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tasks (uuid, description, created_at, tags) VALUES (?1, ?2, ?3, ?4)",
            params!["t1", "买菜", "2026-01-01T00:00:00Z", r#"["life","urgent"]"#],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (uuid, description, created_at, tags) VALUES (?1, ?2, ?3, ?4)",
            params!["t2", "写周报", "2026-01-02T00:00:00Z", r#"["work","urgent"]"#],
        )
        .unwrap();
        // 没有标签的任务：不应该在 tags 表里留下任何痕迹
        conn.execute(
            "INSERT INTO tasks (uuid, description, created_at, tags) VALUES (?1, ?2, ?3, ?4)",
            params!["t3", "没有标签", "2026-01-03T00:00:00Z", "[]"],
        )
        .unwrap();

        init(&conn).unwrap();

        // schema_version 升到了最新
        let version: i64 =
            conn.query_row("SELECT version FROM schema_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);

        // tasks.tags 列已经被删除
        let has_tags_column: bool = conn
            .prepare("SELECT * FROM tasks LIMIT 0")
            .unwrap()
            .column_names()
            .iter()
            .any(|c| *c == "tags");
        assert!(!has_tags_column, "tasks.tags 列应该已经被 DROP COLUMN 删除");

        // tags 表：三个不重复的标签名，颜色初始为空
        let mut stmt = conn.prepare("SELECT name, color FROM tags ORDER BY name").unwrap();
        let tags: Vec<(String, Option<String>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            tags,
            vec![
                ("life".to_string(), None),
                ("urgent".to_string(), None),
                ("work".to_string(), None),
            ]
        );

        // task_tags：每个任务对应正确的标签集合
        let mut stmt = conn
            .prepare("SELECT tag_name FROM task_tags WHERE task_uuid = ?1 ORDER BY tag_name")
            .unwrap();
        let t1_tags: Vec<String> =
            stmt.query_map(["t1"], |r| r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(t1_tags, vec!["life", "urgent"]);

        let t3_tags: Vec<String> =
            stmt.query_map(["t3"], |r| r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert!(t3_tags.is_empty());

        let total_task_tags: i64 =
            conn.query_row("SELECT COUNT(*) FROM task_tags", [], |r| r.get(0)).unwrap();
        assert_eq!(total_task_tags, 4); // t1: 2 + t2: 2

        // 再跑一次 init 应该是纯粹的空操作，不报错、不产生重复数据
        init(&conn).unwrap();
        let total_task_tags_again: i64 =
            conn.query_row("SELECT COUNT(*) FROM task_tags", [], |r| r.get(0)).unwrap();
        assert_eq!(total_task_tags_again, 4);
    }

    /// 全新数据库（从版本 0 直接建到最新）不应该报错，且不会触发标签回填
    #[test]
    fn fresh_database_migrates_cleanly() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();

        let version: i64 =
            conn.query_row("SELECT version FROM schema_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);

        let tag_count: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0)).unwrap();
        assert_eq!(tag_count, 0);
    }
}
