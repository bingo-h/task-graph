//! 紧迫度计算
//!
//! urgency = priority_score + due_score + age_score + tags_score + depends_score
//!
//! priority: H=6.0  M=3.9  L=1.8  无=0
//! due: 逾期=12  今日=8  7天内/14天内线性插值  无=0
//! age: min(age_days/365, 1) x 2.0
//! tags: 有标签 +1.0
//! depends: 有前置任务 -1.0

use chrono::{DateTime, Utc};

/// 计算任务的 Urgency 分数（保留两位小数）
pub fn compute_urgency(
    priority: Option<&str>,
    due: Option<&str>,
    created_at: &str,
    tags: &[String],
    depends: &[String],
) -> f64 {
    let mut urgency = 0.0_f64;

    urgency += match priority {
        Some("H") => 6.0,
        Some("M") => 3.9,
        Some("L") => 1.8,
        _ => 0.0,
    };

    // 这句代码的意思是：判断due是否为空
    // 不为空则赋值给due_str
    if let Some(due_str) = due {
        if let Ok(due_dt) = DateTime::parse_from_rfc3339(due_str) {
            let now = Utc::now();
            let due_dt = due_dt.with_timezone(&Utc);
            let delta = due_dt.signed_duration_since(now);

            // 这里不能用 num_days() （会四舍五入）
            // 因为要保留天的小数用于 7/14 天内线性插值
            let days = delta.num_seconds() as f64 / 86400.0;

            // due_score 采用分段线性衰减：
            // 1. 已逾期：直接给最高分 12，表示紧迫度最高。
            // 2. 今天内：固定 8 分，表示当天到期但尚未过期。
            // 3. 1-7 天内：从 8 分线性降到 5 分，越接近截止时间分越高。
            // 4. 7-14 天内：从 5 分线性降到 0 分，越远分越低。
            // 5. 14 天后：认为短期内不紧迫，得 0 分。
            urgency += if days < 0.0 {
                12.0
            } else if days < 1.0 {
                8.0
            } else if days < 7.0 {
                8.0 - (days / 7.0) * 3.0
            } else if days < 14.0 {
                5.0 - ((days - 7.0) / 7.0) * 3.0
            } else {
                0.0
            }
        }
    }

    if let Ok(created_dt) = DateTime::parse_from_rfc3339(created_at) {
        let age_days = Utc::now()
            .signed_duration_since(created_dt.with_timezone(&Utc))
            .num_days() as f64;
        urgency += (age_days / 365.0).min(1.0) * 2.0;
    }

    if !tags.is_empty() {
        urgency += 1.0;
    }
    if !depends.is_empty() {
        urgency -= 1.0;
    }

    (urgency * 100.0).round() / 100.0
}
