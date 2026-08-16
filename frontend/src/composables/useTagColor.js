/**
 * @file 标签颜色小工具
 * @module useTagColor
 * @description 标签本身只存一个 hex 颜色（或不设置，回退到默认洋红），
 *  这里统一算出标签徽章用的浅色背景 + 主色文字，供各处标签展示复用。
 */

const DEFAULT_TAG_COLOR = "#8250df"; // 默认洋红，和标签下拉框/多选高亮同一套配色

/** "#rrggbb" -> "rgba(r, g, b, alpha)"，非法输入原样返回浅色兜底 */
function hexToRgba(hex, alpha) {
  const h = (hex || "").replace("#", "");
  const normalized = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  const int = parseInt(normalized, 16);

  if (normalized.length !== 6 || Number.isNaN(int)) {
    return `rgba(130, 80, 223, ${alpha})`;
  }

  const r = (int >> 16) & 255;
  const g = (int >> 8) & 255;
  const b = int & 255;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * 标签徽章的行内样式：浅色背景 + 标签自己的颜色作为文字色
 * @param {string} color - 标签的 color 字段（hex），没设置时用默认色
 */
export function tagChipStyle(color) {
  const c = color || DEFAULT_TAG_COLOR;
  return {
    background: hexToRgba(c, 0.15),
    color: c,
  };
}

export { DEFAULT_TAG_COLOR };
