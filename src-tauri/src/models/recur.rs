//! 周期性任务的日期计算（纯函数，不碰数据库/Tauri 类型）
//!
//! 所有周期的截止时间统一落在当天 23:59:59Z，跟应用里其它日期字段一样
//! 只按 UTC 计算，不做用户时区换算（沿用 today_marked_date/created_at 已有的约定）。

use super::task::RecurRule;
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};

/// 算出严格晚于 `from` 的下一个周期截止时间
pub fn next_cycle_due(rule: &RecurRule, from: DateTime<Utc>) -> DateTime<Utc> {
    match rule {
        RecurRule::Daily { interval } => {
            let days = (*interval).max(1) as i64;
            end_of_day(from.date_naive() + Duration::days(days))
        }

        RecurRule::Weekly { weekdays } => {
            let wanted = normalized_weekdays(weekdays, from);
            let mut d = from.date_naive() + Duration::days(1);
            for _ in 0..7 {
                if wanted.contains(&d.weekday().num_days_from_monday()) {
                    return end_of_day(d);
                }
                d += Duration::days(1);
            }
            // 理论不会走到这里（7 天内必有一个命中的星期几）
            end_of_day(from.date_naive() + Duration::days(7))
        }

        RecurRule::Monthly { day } => {
            let (y, m) = (from.year(), from.month());
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            end_of_day(NaiveDate::from_ymd_opt(ny, nm, clamp_day(ny, nm, *day)).unwrap())
        }
    }
}

/// 首次开启周期性时算第一个周期的截止时间：规则当天就命中则用今天，否则找下一个命中日
pub fn first_cycle_due(rule: &RecurRule, now: DateTime<Utc>) -> DateTime<Utc> {
    if matches_today(rule, now) {
        end_of_day(now.date_naive())
    } else {
        next_cycle_due(rule, now)
    }
}

fn matches_today(rule: &RecurRule, now: DateTime<Utc>) -> bool {
    match rule {
        RecurRule::Daily { .. } => true,
        RecurRule::Weekly { weekdays } => {
            normalized_weekdays(weekdays, now).contains(&now.weekday().num_days_from_monday())
        }
        RecurRule::Monthly { day } => {
            now.day() == clamp_day(now.year(), now.month(), *day)
        }
    }
}

/// 周几列表为空时的兜底：按基准时间当天的星期几算（相当于"每周同一天"）
fn normalized_weekdays(weekdays: &[u8], fallback_from: DateTime<Utc>) -> Vec<u32> {
    if weekdays.is_empty() {
        vec![fallback_from.weekday().num_days_from_monday()]
    } else {
        weekdays.iter().map(|&d| d as u32).collect()
    }
}

/// 超出当月天数时截断到月末（如 31 号在二月按 28/29 号算）
fn clamp_day(year: i32, month: u32, day: u8) -> u32 {
    (day as u32).max(1).min(days_in_month(year, month))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .unwrap()
        .pred_opt()
        .unwrap()
        .day()
}

fn end_of_day(d: NaiveDate) -> DateTime<Utc> {
    Utc.from_utc_datetime(&d.and_hms_opt(23, 59, 59).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, 0, 0).unwrap())
    }

    #[test]
    fn daily_advances_by_interval() {
        let rule = RecurRule::Daily { interval: 3 };
        let from = dt(2026, 1, 1, 12);
        assert_eq!(next_cycle_due(&rule, from).date_naive(), NaiveDate::from_ymd_opt(2026, 1, 4).unwrap());
    }

    #[test]
    fn weekly_finds_next_matching_weekday() {
        // 2026-01-01 是周四 (num_days_from_monday=3)；规则选周一(0)和周三(2)
        let rule = RecurRule::Weekly { weekdays: vec![0, 2] };
        let from = dt(2026, 1, 1, 12);
        // 下一个命中应该是周一 2026-01-05
        assert_eq!(next_cycle_due(&rule, from).date_naive(), NaiveDate::from_ymd_opt(2026, 1, 5).unwrap());
    }

    #[test]
    fn monthly_clamps_to_month_end() {
        let rule = RecurRule::Monthly { day: 31 };
        let from = dt(2026, 1, 31, 12);
        // 下个月是二月，2026 非闰年只有 28 天
        assert_eq!(next_cycle_due(&rule, from).date_naive(), NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
    }

    #[test]
    fn first_cycle_uses_today_when_rule_matches() {
        let rule = RecurRule::Daily { interval: 1 };
        let now = dt(2026, 3, 10, 9);
        assert_eq!(first_cycle_due(&rule, now).date_naive(), NaiveDate::from_ymd_opt(2026, 3, 10).unwrap());
    }
}
