//! 周期性任务：回滚/催更算法 + 完成记录（recur_log）读写
//!
//! 这是个没有后台常驻进程的桌面应用，任务的"周期是否已经翻篇"只能在每次
//! 打开/刷新时惰性补算（跟 task::reset_stale_today_marks 是同一种"懒补课"模式）。

use crate::models::recur;
use crate::models::task::{RecurRule, TaskStatus};
use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;

/// 单条周期完成记录，供日历页展示
#[derive(Debug, Clone, Serialize)]
pub struct RecurLogEntry {
    pub id: i64,
    pub task_uuid: String,
    pub cycle_due: String,
    pub outcome: String,
    pub completed_at: Option<String>,
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<RecurLogEntry> {
    Ok(RecurLogEntry {
        id: row.get(0)?,
        task_uuid: row.get(1)?,
        cycle_due: row.get(2)?,
        outcome: row.get(3)?,
        completed_at: row.get(4)?,
    })
}

/// 一次性数据修复（schema 版本 15）：早前周期截止时间一律按 UTC 当天 23:59:59 算，
/// 现在改成了按本地日历天算（见 models::recur::end_of_day）。已经写进数据库的 due
/// 是用旧逻辑算出来的，光改代码不会让这些历史数据自动变对——不修的话，这些任务
/// 还要再等一次用旧逻辑算出来的、偏移了时区差的截止时间过去，才能真正用上新逻辑。
///
/// 这里把已有 due 的日期部分取出来（旧逻辑里这就是"这个截止时间打算表示哪一天"），
/// 按新逻辑重新算一遍 23:59:59 再写回去，让修复立刻生效，不用等一个"旧的"周期。
pub fn backfill_local_due(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT uuid, due FROM tasks WHERE recur_rule IS NOT NULL AND due IS NOT NULL",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    for (uuid, due_str) in rows {
        let Ok(due) = chrono::DateTime::parse_from_rfc3339(&due_str) else {
            continue;
        };
        let date = due.with_timezone(&chrono::Utc).date_naive();
        let corrected = recur::end_of_day(date).to_rfc3339();

        conn.execute(
            "UPDATE tasks SET due = ?2 WHERE uuid = ?1",
            params![uuid, corrected],
        )?;
    }

    Ok(())
}

/// 某任务的全部周期完成记录，按 cycle_due 倒序
pub fn list_log(conn: &Connection, task_uuid: &str) -> Result<Vec<RecurLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_uuid, cycle_due, outcome, completed_at
         FROM recur_log WHERE task_uuid = ?1 ORDER BY cycle_due DESC",
    )?;
    let rows = stmt
        .query_map(params![task_uuid], row_to_entry)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 当前连续完成天数：从最近一个周期往前数，遇到第一条 missed 就停
pub fn current_streak(conn: &Connection, task_uuid: &str) -> Result<i64> {
    let mut stmt = conn.prepare(
        "SELECT outcome FROM recur_log WHERE task_uuid = ?1 ORDER BY cycle_due DESC",
    )?;
    let outcomes = stmt
        .query_map(params![task_uuid], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut streak = 0;
    for outcome in outcomes {
        if outcome == "missed" {
            break;
        }
        streak += 1;
    }
    Ok(streak)
}

/// 开启/关闭任务的周期性规则。开启时立即算出第一个周期的截止日期，
/// 状态重置为待办，并标记为"今日任务"；关闭时只清空规则，历史 recur_log 保留不动。
pub fn set_recur_rule(conn: &Connection, uuid: &str, rule: Option<RecurRule>) -> Result<()> {
    match &rule {
        Some(r) => {
            let now = chrono::Utc::now();
            let due = recur::first_cycle_due(r, now).to_rfc3339();
            let rule_json = serde_json::to_string(r)?;
            // "今日任务"标记要跟本地日历天走，不能用 now 的 UTC 日期
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();

            conn.execute(
                "UPDATE tasks SET recur_rule=?2, status='pending', end=NULL,
                     due=?3, today_marked_date=?4
                 WHERE uuid=?1",
                params![uuid, rule_json, due, today],
            )?;
        }
        None => {
            conn.execute(
                "UPDATE tasks SET recur_rule=NULL WHERE uuid=?1",
                params![uuid],
            )?;
        }
    }
    Ok(())
}

/// 对所有周期性任务做一次"惰性补课"：
/// - 若当前周期（due）已经过去，逐个周期往前滚，把每个滚过去的周期结算进 recur_log
///   （完成过的记 completed_on_time/completed_late，没完成过的记 missed；
///   连续多个错过的周期只折叠成一条 missed 记录，不逐个补历史行——
///   对"连续天数"计算无损，因为算法从最近周期往前扫，遇到第一条 missed 就停止）
/// - 滚完之后 due 落在未来某个周期，status 重置为 pending，today_marked_date 设为今天
/// - 若本次没有翻篇但任务仍是 pending（还在等这一周期完成），只要 today_marked_date
///   不是今天也要重新盖成今天，这样"重复任务只要没完成就一直留在今日任务里"
pub fn process_rollovers(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT uuid, status, due, end, recur_rule FROM tasks
         WHERE recur_rule IS NOT NULL AND status != 'deleted'",
    )?;

    struct Row {
        uuid: String,
        status: String,
        due: Option<String>,
        end: Option<String>,
        rule: RecurRule,
    }

    let rows: Vec<Row> = stmt
        .query_map([], |r| {
            let rule_json: String = r.get(4)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                rule_json,
            ))
        })?
        .filter_map(|res| res.ok())
        .filter_map(|(uuid, status, due, end, rule_json)| {
            serde_json::from_str::<RecurRule>(&rule_json)
                .ok()
                .map(|rule| Row { uuid, status, due, end, rule })
        })
        .collect();

    let now = chrono::Utc::now();
    // "今日任务"标记要跟本地日历天走，不能用 now 的 UTC 日期
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    for row in rows {
        let Some(due_str) = row.due.as_deref() else {
            continue; // 理论不会发生：set_recur_rule 开启时一定会写 due
        };
        let Ok(mut due) = chrono::DateTime::parse_from_rfc3339(due_str).map(|d| d.with_timezone(&chrono::Utc)) else {
            continue;
        };

        // 单份"这一次完成"的凭证，最多用于结算一个周期（.take() 语义）
        let mut completion = if row.status == TaskStatus::Completed.as_str() {
            row.end
                .as_deref()
                .and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok())
                .map(|d| d.with_timezone(&chrono::Utc))
        } else {
            None
        };

        let mut cycles = 0;
        let mut pending_missed_due: Option<chrono::DateTime<chrono::Utc>> = None;

        while due < now {
            cycles += 1;
            let next_due = recur::next_cycle_due(&row.rule, due);

            match completion.take() {
                Some(end) if end <= next_due => {
                    let outcome = if end <= due { "completed_on_time" } else { "completed_late" };
                    conn.execute(
                        "INSERT OR IGNORE INTO recur_log
                             (task_uuid, cycle_due, outcome, completed_at, recorded_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![row.uuid, due.to_rfc3339(), outcome, end.to_rfc3339(), now.to_rfc3339()],
                    )?;
                    pending_missed_due = None;
                }
                other => {
                    // 完成时间比 next_due 还晚（极端情况：应用长期没打开），
                    // 或者这一周期压根没完成过：都记为这一轮"待补的 missed"，
                    // 继续往下滚，多个连续 missed 只在最后落一条记录
                    completion = other; // 放回去，也许能匹配到更晚的周期
                    pending_missed_due = Some(due);
                }
            }

            due = next_due;
        }

        if let Some(missed_due) = pending_missed_due {
            conn.execute(
                "INSERT OR IGNORE INTO recur_log
                     (task_uuid, cycle_due, outcome, completed_at, recorded_at)
                 VALUES (?1, ?2, 'missed', NULL, ?3)",
                params![row.uuid, missed_due.to_rfc3339(), now.to_rfc3339()],
            )?;
        }

        if cycles > 0 {
            conn.execute(
                "UPDATE tasks SET status='pending', end=NULL, due=?2, today_marked_date=?3
                 WHERE uuid=?1",
                params![row.uuid, due.to_rfc3339(), today],
            )?;
        } else if row.status == TaskStatus::Pending.as_str() {
            conn.execute(
                "UPDATE tasks SET today_marked_date=?2
                 WHERE uuid=?1 AND (today_marked_date IS NULL OR today_marked_date != ?2)",
                params![row.uuid, today],
            )?;
        }
    }

    Ok(())
}
