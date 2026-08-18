/**
 * @file 本地时区时间换算
 * @module useLocalTime
 * @description
 *  后端存储/传输的所有时间戳都是 UTC（数据库里统一用 UTC，方便不做时区处理地
 *  比较排序）。但凡是要给人看、或者是人在表单里直接输入的时间，都必须换算成
 *  本地时区——不能直接截字符串当成本地时间用，那样在非 UTC+0 的时区下会算错
 *  （比如"今日任务"、重复任务周期过去踩过这个坑）。
 *  这里统一提供 UTC ISO 字符串 <-> 本地日期/时间字符串的双向换算。
 */

function pad(n) {
    return String(n).padStart(2, "0");
}

/** UTC ISO 时间戳 -> 本地日期 "YYYY-MM-DD"；解析失败/空值返回空字符串 */
export function isoToLocalDate(iso) {
    if (!iso) return "";
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return "";
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** UTC ISO 时间戳 -> 本地时刻 "HH:MM"；解析失败/空值返回空字符串 */
export function isoToLocalTime(iso) {
    if (!iso) return "";
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return "";
    return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** UTC ISO 时间戳 -> 本地"YYYY-MM-DD HH:MM"；解析失败/空值返回空字符串 */
export function isoToLocalDateTime(iso) {
    if (!iso) return "";
    const date = isoToLocalDate(iso);
    const time = isoToLocalTime(iso);
    return date && time ? `${date} ${time}` : date;
}

/**
 * 本地日期(+可选时刻) -> UTC ISO 时间戳。
 * dateStr 形如 "YYYY-MM-DD"，timeStr 形如 "HH:MM"（留空则用 00:00）。
 * 按浏览器所在的本地时区解释这组日期时间，再转换成正确的 UTC 时刻。
 * dateStr 为空时返回 null。
 */
export function localDateTimeToIso(dateStr, timeStr) {
    if (!dateStr) return null;

    const [year, month, day] = dateStr.split("-").map(Number);
    const [hour, minute] = (timeStr || "00:00").split(":").map(Number);

    // 用分解开的年月日时分构造 Date：JS 按本地时区解释这几个字段，
    // 跟直接 new Date("YYYY-MM-DDTHH:MM")（会被当成 UTC 或本地不确定）不一样
    const d = new Date(year, month - 1, day, hour, minute, 0, 0);
    return d.toISOString();
}
