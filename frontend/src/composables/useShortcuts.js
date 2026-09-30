/**
 * @file 全局快捷键：动作注册表（唯一数据源）+ 按键归一化/匹配/冲突检测 + 全局监听。
 * @module useShortcuts
 * @description
 *  归一化规则：固定 "mod+shift+alt+主键" 顺序、全部小写，"+" 连接。
 *  mod 是平台无关占位符，macOS 上对应 Cmd（metaKey），其它平台对应 Ctrl（ctrlKey）。
 *  主键是字母/数字时，大小写信息已经丢在 toLowerCase 里，所以额外用 shiftKey 补一个
 *  "shift" 修饰段；主键本身就是符号（如 "?"，由 Shift+/ 敲出但 e.key 已经是 "?"）时，
 *  不再叠加 "shift"——那样会导致这个符号键永远匹配不上不带修饰键的默认值。
 * @author Bin.H
 */

import { onMounted, onUnmounted } from "vue";

export function isMac() {
  const platform =
    navigator.userAgentData?.platform || navigator.platform || "";
  return platform.toLowerCase().includes("mac");
}

/** 把一次 keydown 事件归一化成绑定字符串；修饰键单独按下（还没配合主键）时返回 null */
export function normalizeKeyEvent(e) {
  if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return null;

  const modPressed = isMac() ? e.metaKey : e.ctrlKey;
  const isLetterOrDigit = /^[a-z0-9]$/i.test(e.key);

  const parts = [];
  if (modPressed) parts.push("mod");
  if (isLetterOrDigit && e.shiftKey) parts.push("shift");
  if (e.altKey) parts.push("alt");

  const baseKey = e.key.toLowerCase();
  parts.push(baseKey);
  return parts.join("+");
}

/** 当前焦点是否在文本输入类元素上（此时全局快捷键应该完全让路，不拦截任何按键） */
export function isTypingTarget(el) {
  const tag = el?.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || !!el?.isContentEditable;
}

/** 把归一化绑定字符串渲染成用户可读的展示文本，如 "mod+shift+z" -> "Ctrl+Shift+Z" */
export function formatBindingForDisplay(binding) {
  if (!binding) return "未绑定";
  const modLabel = isMac() ? "⌘" : "Ctrl";
  return binding
    .split("+")
    .map((part) => {
      if (part === "mod") return modLabel;
      if (part === "shift") return "Shift";
      if (part === "alt") return isMac() ? "Option" : "Alt";
      return part.toUpperCase();
    })
    .join("+");
}

/**
 * 动作注册表。`refs` 由调用方（App.vue）以依赖注入的方式传入，
 * 保持本文件不直接 import/操作任何组件——跟 useLayout.js 的既有约定一致。
 * @param {object} refs
 * @param {Function} refs.openAdd - 打开新建任务弹窗
 * @param {Function} refs.focusSearch - 聚焦全局搜索框
 * @param {import('vue').Ref<string>} refs.currentPage
 * @param {import('vue').Ref<boolean>} refs.showSettings
 * @param {import('vue').Ref<boolean>} refs.paletteOpen
 * @param {import('vue').Ref<boolean>} refs.helpOpen
 */
export function buildActions(refs) {
  const { openAdd, focusSearch, currentPage, showSettings, paletteOpen, helpOpen } = refs;

  return [
    { id: "task.new", label: "新建任务", category: "任务", run: () => openAdd(), defaultKeys: "mod+n" },
    { id: "search.focus", label: "聚焦搜索框", category: "导航", run: () => focusSearch(), defaultKeys: "mod+f" },
    { id: "palette.open", label: "命令面板", category: "导航", run: () => { paletteOpen.value = true; }, defaultKeys: "mod+k" },
    { id: "settings.open", label: "设置", category: "导航", run: () => { showSettings.value = true; }, defaultKeys: "mod+," },
    { id: "view.home", label: "切换到首页", category: "视图", run: () => { currentPage.value = "home"; }, defaultKeys: "1" },
    { id: "view.charts", label: "切换到分析", category: "视图", run: () => { currentPage.value = "charts"; }, defaultKeys: "2" },
    { id: "view.calendar", label: "切换到日历", category: "视图", run: () => { currentPage.value = "calendar"; }, defaultKeys: "3" },
    { id: "view.board", label: "切换到任务看板", category: "视图", run: () => { currentPage.value = "board"; }, defaultKeys: "4" },
    { id: "help.shortcuts", label: "快捷键帮助", category: "帮助", run: () => { helpOpen.value = true; }, defaultKeys: "?" },
  ];
}

/** 某个 action 当前实际生效的绑定：显式设为空字符串表示"无绑定"，不在 overrides 里则用默认值 */
export function effectiveBinding(action, overrides) {
  if (Object.prototype.hasOwnProperty.call(overrides, action.id)) {
    return overrides[action.id];
  }
  return action.defaultKeys;
}

/** 候选绑定是否已被别的 action 占用，返回占用者 action，没冲突返回 null */
export function findConflict(actions, overrides, actionId, candidateBinding) {
  if (!candidateBinding) return null;
  for (const action of actions) {
    if (action.id === actionId) continue;
    if (effectiveBinding(action, overrides) === candidateBinding) return action;
  }
  return null;
}

/**
 * 挂载全局快捷键监听（capture 阶段），组件卸载时自动移除。
 * @param {object} refs - 除 buildActions 用到的几项外，额外需要：
 * @param {import('vue').Ref<Record<string,string>>} refs.shortcuts - 当前生效的用户覆盖表（settings.shortcuts）
 * @param {import('vue').ComputedRef<boolean>} refs.anyModalOpen - 是否有任意弹窗/面板打开
 * @param {import('vue').Ref<string|null>} refs.selectedUUID - 当前选中的任务，Esc 兜底清除用
 */
export function useShortcuts(refs) {
  const actions = buildActions(refs);
  const { shortcuts, anyModalOpen, paletteOpen, helpOpen, selectedUUID } = refs;

  function handleGlobalShortcut(e) {
    // Escape 优先判断：必须在"输入框豁免"之前，否则在搜索框/命令面板输入框里
    // 按 Esc 会被直接放行、永远关不掉面板。
    if (e.key === "Escape") {
      if (paletteOpen.value) {
        paletteOpen.value = false;
        return;
      }
      if (helpOpen.value) {
        helpOpen.value = false;
        return;
      }
      if (!anyModalOpen.value) {
        selectedUUID.value = null;
      }
      return;
    }

    // 正在文本输入，或者有任意弹窗打开着：全局快捷键整体让路，
    // 避免打字误触发，也避免在弹窗背后悄悄执行了切视图这类操作。
    if (isTypingTarget(e.target) || anyModalOpen.value) return;

    if (e.key === "F1") {
      e.preventDefault();
      helpOpen.value = true;
      return;
    }

    const combo = normalizeKeyEvent(e);
    if (!combo) return;

    const overrides = shortcuts.value || {};
    for (const action of actions) {
      if (effectiveBinding(action, overrides) === combo) {
        e.preventDefault();
        action.run();
        return;
      }
    }
  }

  onMounted(() => {
    document.addEventListener("keydown", handleGlobalShortcut, true);
  });
  onUnmounted(() => {
    document.removeEventListener("keydown", handleGlobalShortcut, true);
  });

  return { actions };
}
