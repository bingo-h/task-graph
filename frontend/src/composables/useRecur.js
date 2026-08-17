/**
 * @file 周期性任务重复规则的展示格式化
 * @module useRecur
 */

const RECUR_WEEKDAY_LABELS = ["一", "二", "三", "四", "五", "六", "日"];

/** 把后端 RecurRule 拼成人类可读的一句话摘要，没有规则时返回空字符串 */
export function formatRecurSummary(rule) {
  if (!rule) return "";
  if (rule.kind === "daily") {
    return rule.interval === 1 ? "每天" : `每 ${rule.interval} 天`;
  }
  if (rule.kind === "weekly") {
    return rule.weekdays.length
      ? `每周${rule.weekdays.map((d) => RECUR_WEEKDAY_LABELS[d]).join("、")}`
      : "每周";
  }
  return `每月第 ${rule.day} 天`;
}
