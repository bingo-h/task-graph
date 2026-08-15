/**
 * @file 时长格式化
 * @description
 *  按用户在设置里配置的格式字符串格式化秒数。
 *  记号沿用 strftime 的 % 前缀写法：只有 "%X" 才会被替换，
 *  普通字母原样保留，因此可以自由拼出 "%Dd %Hh%Mm%Ss" → "1d 20h05m30s"
 *  这种带字母后缀的格式，不用担心字母本身和占位符打架。
 *    %D / %DD   天（不补零 / 补零两位）
 *    %H / %h    时，补零两位 / 不补零（格式里没有 %D、%DD 时代表总小时数，可超过 24）
 *    %M / %m    分，补零两位 / 不补零
 *    %S / %s    秒，补零两位 / 不补零
 */

import { ref } from "vue";

// 当前生效的格式，由 App.vue 在加载/保存设置后同步进来；
// 放在模块级别而不是逐层传参，方便所有用到 formatDuration 的组件直接复用。
export const DEFAULT_DURATION_FORMAT = "%H:%M:%S";
const durationFormat = ref(DEFAULT_DURATION_FORMAT);

/** 更新全局生效的时长格式（应用设置里保存后调用） */
export function setDurationFormat(format) {
  durationFormat.value = format && format.trim() ? format : DEFAULT_DURATION_FORMAT;
}

/**
 * 格式化秒数为字符串。省略 format 时使用当前设置里的格式。
 *
 * @param {number} totalSeconds
 * @param {string} [format]
 */
export function formatDuration(totalSeconds, format = durationFormat.value) {
  const s = Math.max(0, Math.floor(totalSeconds));
  const hasDay = /%DD|%D/.test(format);

  const days = hasDay ? Math.floor(s / 86400) : 0;
  const rest = hasDay ? s % 86400 : s;
  const hours = Math.floor(rest / 3600);
  const minutes = Math.floor((rest % 3600) / 60);
  const seconds = rest % 60;

  const pad = (n) => String(n).padStart(2, "0");

  return format
    .replace(/%DD/g, pad(days))
    .replace(/%D/g, String(days))
    .replace(/%H/g, pad(hours))
    .replace(/%h/g, String(hours))
    .replace(/%M/g, pad(minutes))
    .replace(/%m/g, String(minutes))
    .replace(/%S/g, pad(seconds))
    .replace(/%s/g, String(seconds));
}
