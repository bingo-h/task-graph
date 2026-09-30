<!--
  @Component: 设置弹窗组件
  @Description:
    左侧菜单分区展示各类设置：通用（高亮模式/废纸篓天数/字体大小）、
    时长格式（计时时长的显示格式）。保存在独立 settings.json 中。
  @Author: Bin.H
  @Date: 2026-08-15
-->

<script setup>
import { ref, computed, watch, onBeforeUnmount, nextTick } from "vue";
import SegmentedControl from "./SegmentedControl.vue";
import { formatDuration, DEFAULT_DURATION_FORMAT } from "../composables/useDuration";
import { getVersion } from "@tauri-apps/api/app";
import { listSystemFonts, listColorSchemes } from "../composables/useApi";
import constants from "../config/constants";
import {
    updateStatus,
    updateError,
    latestVersion,
    updateNotes,
    checkForUpdate,
    downloadAndInstallUpdate,
    restartToApply,
} from "../composables/useUpdater";
import {
    buildActions,
    effectiveBinding,
    findConflict,
    formatBindingForDisplay,
    normalizeKeyEvent,
} from "../composables/useShortcuts";

const props = defineProps({
    visible: { type: Boolean, required: true },
    settings: { type: Object, required: true },
    highlightMode: { type: String, required: true },
});

const emit = defineEmits(["close", "save", "update:highlight-mode"]);

// ----------------------------------------
// 左侧菜单分区
// ----------------------------------------
const SECTIONS = [
    { key: "general", label: "通用" },
    { key: "appearance", label: "外观" },
    { key: "duration", label: "时长格式" },
    { key: "graph", label: "图谱显示" },
    { key: "shortcuts", label: "快捷键" },
    { key: "about", label: "关于" },
];
const activeSection = ref("general");

const currentVersion = ref("");
getVersion().then((v) => (currentVersion.value = v));

const highlightModeOptions = [
    { key: "ancestors", label: "祖先链路" },
    { key: "neighbors", label: "直接上下游" },
    { key: "full", label: "完整链路" },
];

const trashRetentionDays = ref(30);
const fontSize = ref(14);
const durationFormat = ref(DEFAULT_DURATION_FORMAT);
const defaultDueTime = ref("23:59");
const inboxLabel = ref(constants.INBOX_PROJECT);
const notificationDurationSeconds = ref(3);

// ----------------------------------------
// 外观：配色方案 / 深浅模式 / 圆角 / 界面风格
// ----------------------------------------
const colorScheme = ref("");
const themeMode = ref("light");
const cornerRadius = ref(10);
const uiStyle = ref("flat");

// Teleport 出去的遮罩层不是弹窗打开前焦点所在元素的祖先，@keydown.esc 挂在遮罩层上
// 只有遮罩层内部有元素持有焦点时才会收到事件——所以弹窗打开时要主动把焦点塞进来，
// 具体做法跟 ConfirmDialog.vue 一致。这个弹窗内容随分区切换、没有一个总是存在的
// 自然聚焦目标（分区不同、控件也不同），因此直接聚焦遮罩层自身（配合 tabindex="-1"）。
const overlayRef = ref(null);

// ----------------------------------------
// 快捷键：改绑 / 冲突检测 / 恢复默认
// ----------------------------------------
const shortcutOverrides = ref({}); // 本地编辑副本，保存前不影响 App.vue 的真实 settings
const recordingActionId = ref(null); // 当前正在"录制"按键的 action id，null 表示没有
const shortcutConflict = ref(null); // { actionId, conflictWith: action, candidate } 或 null

// 快捷键分区不需要真实的 App.vue 注入（openAdd/currentPage 等 run() 回调这里用不上，
// 只是借用同一份 buildActions 拿到 id/label/category/defaultKeys），传空函数占位即可
const shortcutActions = buildActions({
    openAdd: () => {},
    focusSearch: () => {},
    currentPage: { value: "" },
    showSettings: { value: false },
    paletteOpen: { value: false },
    helpOpen: { value: false },
});

function bindingLabel(action) {
    return formatBindingForDisplay(effectiveBinding(action, shortcutOverrides.value));
}

// 录制按键用 document 级监听而不是"聚焦按钮 + @keydown"：WebKitGTK（这个项目在
// Linux 上用的 webview 引擎）点击 <button> 不一定会给它键盘焦点（跟 Chromium 的默认
// 行为不一样），指着按钮本身的 @keydown 在这个引擎上不可靠。这里改成跟 IconPicker.vue
// 已有的"打开时挂 document 监听、关闭时摘掉"同一套模式，不依赖任何元素的焦点状态。
function startRecording(actionId) {
    recordingActionId.value = actionId;
    shortcutConflict.value = null;
    document.addEventListener("keydown", onRecordKeydown, true);
}

function stopRecording() {
    recordingActionId.value = null;
    document.removeEventListener("keydown", onRecordKeydown, true);
}

function onRecordKeydown(e) {
    e.preventDefault();
    e.stopPropagation();
    const actionId = recordingActionId.value;
    if (!actionId) return;

    if (e.key === "Escape") {
        stopRecording();
        return;
    }
    const candidate = normalizeKeyEvent(e);
    if (!candidate) return; // 单独按下修饰键，继续等主键

    const conflictWith = findConflict(
        shortcutActions,
        shortcutOverrides.value,
        actionId,
        candidate,
    );
    if (conflictWith) {
        shortcutConflict.value = { actionId, conflictWith, candidate };
        stopRecording();
        return;
    }

    shortcutOverrides.value = { ...shortcutOverrides.value, [actionId]: candidate };
    stopRecording();
}

onBeforeUnmount(() => {
    document.removeEventListener("keydown", onRecordKeydown, true);
});

/** 冲突提示里的"改绑给这个"：把原占用者清成显式"无绑定"，候选绑定给新 action */
function resolveConflictOverwrite() {
    if (!shortcutConflict.value) return;
    const { actionId, conflictWith, candidate } = shortcutConflict.value;
    shortcutOverrides.value = {
        ...shortcutOverrides.value,
        [actionId]: candidate,
        [conflictWith.id]: "",
    };
    shortcutConflict.value = null;
}

function cancelConflict() {
    shortcutConflict.value = null;
}

function resetOneShortcut(actionId) {
    const next = { ...shortcutOverrides.value };
    delete next[actionId];
    shortcutOverrides.value = next;
}

function resetAllShortcuts() {
    shortcutOverrides.value = {};
}

const colorSchemeOptions = ref([{ id: "", name: "默认（克制中性）" }]);
async function loadColorSchemesOnce() {
    try {
        const list = await listColorSchemes();
        colorSchemeOptions.value = [
            { id: "", name: "默认（克制中性）" },
            ...list,
        ];
    } catch {
        colorSchemeOptions.value = [{ id: "", name: "默认（克制中性）" }];
    }
}

const themeModeOptions = [
    { key: "system", label: "跟随系统" },
    { key: "light", label: "浅色" },
    { key: "dark", label: "深色" },
];

const uiStyleOptions = [
    { key: "flat", label: "扁平" },
    { key: "neumorphism", label: "新拟态" },
];

// ----------------------------------------
// 字体：从系统已安装字体里选，边输入边模糊搜索筛选（子串匹配，不区分大小写）
// ----------------------------------------
const fontFamily = ref("sans-serif");
const systemFonts = ref([]);
const systemFontsLoaded = ref(false); // 加载过一次就不用重复扫描，扫描系统字体这一下不算便宜
const showFontDropdown = ref(false);
// 候选字体可能有几百上千个，全部渲染进下拉框会很卡，只取前面这些，够用来定位到想要的字体了
const FONT_OPTIONS_LIMIT = 100;

async function loadSystemFontsOnce() {
    if (systemFontsLoaded.value) return;
    try {
        systemFonts.value = await listSystemFonts();
    } catch {
        systemFonts.value = [];
    } finally {
        systemFontsLoaded.value = true;
    }
}

const filteredFontOptions = computed(() => {
    const keyword = fontFamily.value.trim().toLowerCase();
    const list = keyword
        ? systemFonts.value.filter((f) => f.toLowerCase().includes(keyword))
        : systemFonts.value;
    return list.slice(0, FONT_OPTIONS_LIMIT);
});

function selectFont(name) {
    fontFamily.value = name;
    showFontDropdown.value = false;
}

/** 输入框失焦时延迟隐藏下拉框，使下拉项的点击事件能先触发 */
function hideFontDropdownDelayed() {
    setTimeout(() => {
        showFontDropdown.value = false;
    }, 150);
}

// ----------------------------------------
// 节点字体：图谱任务节点卡片单独的字体，留空表示跟随上面的全局字体；
// 候选列表复用同一份 systemFonts，交互跟全局字体输入框是同一套逻辑，只是各自独立的状态
// ----------------------------------------
const nodeFontFamily = ref("");
const showNodeFontDropdown = ref(false);

const filteredNodeFontOptions = computed(() => {
    const keyword = nodeFontFamily.value.trim().toLowerCase();
    const list = keyword
        ? systemFonts.value.filter((f) => f.toLowerCase().includes(keyword))
        : systemFonts.value;
    return list.slice(0, FONT_OPTIONS_LIMIT);
});

function selectNodeFont(name) {
    nodeFontFamily.value = name;
    showNodeFontDropdown.value = false;
}

function hideNodeFontDropdownDelayed() {
    setTimeout(() => {
        showNodeFontDropdown.value = false;
    }, 150);
}

// 图谱任务节点卡片上默认显示哪些信息（悬浮详情窗不受影响，总是显示全部）
const nodeShowProject = ref(true);
const nodeShowDue = ref(true);
const nodeShowPriority = ref(true);
const nodeShowRecur = ref(true);

// 对应信息在卡片上显示的标签文字，可自定义；DEFAULT_NODE_LABELS 是"重置"按钮恢复的目标值
const NODE_LABELS = constants.DEFAULT_NODE_LABELS;
// 卡片本身很窄，标签文字太长会被截断得很难看，限制一下输入长度
const NODE_LABEL_MAX_LENGTH = 8;
const nodeLabelProject = ref(NODE_LABELS.project);
const nodeLabelDue = ref(NODE_LABELS.due);
const nodeLabelPriority = ref(NODE_LABELS.priority);
const nodeLabelRecur = ref(NODE_LABELS.recur);

watch(
    () => props.visible,
    async (visible) => {
        if (!visible) {
            stopRecording();
            return;
        }
        activeSection.value = "general";
        trashRetentionDays.value = props.settings.trash_retention_days ?? 30;
        fontSize.value = props.settings.font_size ?? 14;
        fontFamily.value = props.settings.font_family || "sans-serif";
        nodeFontFamily.value = props.settings.node_font_family || "";
        loadSystemFontsOnce();
        durationFormat.value =
            props.settings.duration_format || DEFAULT_DURATION_FORMAT;
        defaultDueTime.value = props.settings.default_due_time || "23:59";
        inboxLabel.value = props.settings.inbox_label || constants.INBOX_PROJECT;
        notificationDurationSeconds.value = props.settings.notification_duration_seconds ?? 3;
        nodeShowProject.value = props.settings.node_show_project ?? true;
        nodeShowDue.value = props.settings.node_show_due ?? true;
        nodeShowPriority.value = props.settings.node_show_priority ?? true;
        nodeShowRecur.value = props.settings.node_show_recur ?? true;
        nodeLabelProject.value = props.settings.node_label_project || NODE_LABELS.project;
        nodeLabelDue.value = props.settings.node_label_due || NODE_LABELS.due;
        nodeLabelPriority.value = props.settings.node_label_priority || NODE_LABELS.priority;
        nodeLabelRecur.value = props.settings.node_label_recur || NODE_LABELS.recur;
        colorScheme.value = props.settings.color_scheme || "";
        themeMode.value = props.settings.theme_mode || "light";
        cornerRadius.value = props.settings.corner_radius ?? 10;
        // 校验而非单纯 || 兜底：合法取值集合会随版本变化（如液态玻璃被移除），
        // 旧 settings.json 里存的值可能已不在当前选项里，不归一化的话分段控件
        // 不会高亮任何一项，且原样提交会被后端 validate_ui_style 拒绝、连累整个
        // 表单保存不了。
        uiStyle.value = uiStyleOptions.some((o) => o.key === props.settings.ui_style)
            ? props.settings.ui_style
            : "flat";
        loadColorSchemesOnce();
        shortcutOverrides.value = { ...(props.settings.shortcuts || {}) };
        recordingActionId.value = null;
        shortcutConflict.value = null;

        await nextTick();
        overlayRef.value?.focus();
    },
);

// 记号沿用 strftime 的 % 前缀写法，只有 "%X" 才会被替换，普通字母原样保留，
// 因此可以直接把单位字母写进格式里（如 %Dd %Hh%Mm%Ss），不用担心跟占位符冲突。
// 用一段跨天的样例时长（1天20小时5分30秒）预览效果，
// 同时能看出"没写 %D/%DD 时 %H 显示总小时数"这条规则。
const DURATION_TOKENS = [
    { token: "%D / %DD", desc: "天，不补零 / 补零两位" },
    { token: "%H / %h", desc: "时，补零两位 / 不补零（没写 %D、%DD 时为总小时数，可超过 24）" },
    { token: "%M / %m", desc: "分，补零两位 / 不补零" },
    { token: "%S / %s", desc: "秒，补零两位 / 不补零" },
];
const DURATION_PRESETS = [
    "%H:%M:%S",
    "%h:%M:%S",
    "%h小时%m分钟",
    "%Dd %Hh%Mm%Ss",
];
const PREVIEW_SECONDS = 86400 + 20 * 3600 + 5 * 60 + 30; // 1天20小时5分30秒

const durationPreview = computed(() => {
    try {
        return formatDuration(PREVIEW_SECONDS, durationFormat.value || DEFAULT_DURATION_FORMAT);
    } catch {
        return "";
    }
});

/**
 * stepper 输入框失焦时的兜底：v-model.number 遇到非数字文本（如手动输入 "abc"）
 * 会退化成字符串，导致 +/− 按钮算出 NaN 后永久卡死。这里在失焦时把当前值
 * 夹到跟 +/− 按钮相同的上下界内，非法/空值则回退到 fallback。
 */
function clampStepperInput(value, min, max, fallback) {
    const n = Number(value);
    return Number.isFinite(n) ? Math.min(max, Math.max(min, Math.round(n))) : fallback;
}

function submit() {
    emit("save", {
        trash_retention_days: Math.max(
            0,
            Math.round(Number(trashRetentionDays.value) || 0),
        ),
        font_size: Math.min(
            32,
            Math.max(8, Math.round(Number(fontSize.value) || 14)),
        ),
        font_family: fontFamily.value.trim() || "sans-serif",
        // 节点字体允许留空（表示跟随全局字体），不像上面的全局字体那样兜底成 sans-serif
        node_font_family: nodeFontFamily.value.trim(),
        duration_format: durationFormat.value.trim() || DEFAULT_DURATION_FORMAT,
        default_due_time: defaultDueTime.value || "23:59",
        inbox_label: inboxLabel.value.trim() || constants.INBOX_PROJECT,
        notification_duration_seconds: Math.min(
            30,
            Math.max(1, Math.round(Number(notificationDurationSeconds.value) || 3)),
        ),
        node_show_project: nodeShowProject.value,
        node_show_due: nodeShowDue.value,
        node_show_priority: nodeShowPriority.value,
        node_show_recur: nodeShowRecur.value,
        node_label_project: nodeLabelProject.value.trim() || NODE_LABELS.project,
        node_label_due: nodeLabelDue.value.trim() || NODE_LABELS.due,
        node_label_priority: nodeLabelPriority.value.trim() || NODE_LABELS.priority,
        node_label_recur: nodeLabelRecur.value.trim() || NODE_LABELS.recur,
        color_scheme: colorScheme.value,
        theme_mode: themeMode.value,
        corner_radius: clampStepperInput(cornerRadius.value, 0, 24, 10),
        ui_style: uiStyle.value,
        shortcuts: shortcutOverrides.value,
    });
}
</script>

<template>
    <Teleport to="body">
        <div
            v-if="visible"
            ref="overlayRef"
            class="modal-overlay"
            tabindex="-1"
            @click.self="emit('close')"
            @keydown.esc="emit('close')"
        >
            <div class="modal">
                <div class="modal-header">
                    <span class="modal-title">设置</span>
                    <button class="modal-close" @click="emit('close')">
                        ×
                    </button>
                </div>

                <div class="modal-layout">
                    <!-- 左侧分区菜单 -->
                    <nav class="settings-nav">
                        <button
                            v-for="s in SECTIONS"
                            :key="s.key"
                            class="nav-item"
                            :class="{ active: activeSection === s.key }"
                            @click="activeSection = s.key"
                        >
                            {{ s.label }}
                        </button>
                    </nav>

                    <div class="modal-body">
                        <!-- 通用 -->
                        <template v-if="activeSection === 'general'">
                            <div class="form-row">
                                <label class="form-label">
                                    高亮模式
                                    <span class="form-hint">
                                        选中任务时，图谱中链路高亮的范围
                                    </span>
                                </label>
                                <SegmentedControl
                                    :options="highlightModeOptions"
                                    :model-value="highlightMode"
                                    @update:model-value="
                                        emit('update:highlight-mode', $event)
                                    "
                                />
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    废纸篓保留天数
                                    <span class="form-hint">
                                        超过此天数的项目会在下次打开应用时自动彻底删除，0
                                        表示永不自动删除
                                    </span>
                                </label>
                                <div class="stepper">
                                    <button
                                        type="button"
                                        @click="trashRetentionDays = Math.max(0, trashRetentionDays - 1)"
                                    >
                                        −
                                    </button>
                                    <input
                                        v-model.number="trashRetentionDays"
                                        inputmode="numeric"
                                        @blur="trashRetentionDays = clampStepperInput(trashRetentionDays, 0, 3650, 0)"
                                    />
                                    <button
                                        type="button"
                                        @click="trashRetentionDays = Math.min(3650, trashRetentionDays + 1)"
                                    >
                                        +
                                    </button>
                                </div>
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    字体大小
                                    <span class="form-hint">单位像素，8-32</span>
                                </label>
                                <div class="stepper">
                                    <button
                                        type="button"
                                        @click="fontSize = Math.max(8, fontSize - 1)"
                                    >
                                        −
                                    </button>
                                    <input
                                        v-model.number="fontSize"
                                        inputmode="numeric"
                                        @blur="fontSize = clampStepperInput(fontSize, 8, 32, 14)"
                                    />
                                    <button
                                        type="button"
                                        @click="fontSize = Math.min(32, fontSize + 1)"
                                    >
                                        +
                                    </button>
                                </div>
                            </div>

                            <div class="form-row font-field-row">
                                <label class="form-label">
                                    字体
                                    <span class="form-hint">
                                        从系统已安装字体中选择，输入关键字即可模糊搜索
                                    </span>
                                </label>
                                <input
                                    v-model="fontFamily"
                                    class="form-input"
                                    placeholder="输入字体名称关键字…"
                                    @focus="showFontDropdown = true"
                                    @blur="hideFontDropdownDelayed"
                                />

                                <div
                                    class="font-preview"
                                    :style="{ fontFamily: fontFamily || undefined }"
                                >
                                    预览 Preview 任务管理 0123
                                </div>

                                <div
                                    v-if="showFontDropdown && filteredFontOptions.length > 0"
                                    class="suggest-dropdown"
                                >
                                    <button
                                        v-for="f in filteredFontOptions"
                                        :key="f"
                                        type="button"
                                        class="suggest-dropdown-item"
                                        :style="{ fontFamily: f }"
                                        @mousedown.prevent="selectFont(f)"
                                    >
                                        {{ f }}
                                    </button>
                                </div>
                                <div
                                    v-else-if="
                                        showFontDropdown &&
                                        systemFontsLoaded &&
                                        systemFonts.length === 0
                                    "
                                    class="suggest-dropdown"
                                >
                                    <span class="empty-hint">
                                        未检测到系统字体列表，可以直接手动输入字体名称
                                    </span>
                                </div>
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    任务默认到期时间
                                    <span class="form-hint">
                                        新建/修改任务只选日期、不选具体时间时，自动补上的到期时刻
                                    </span>
                                </label>
                                <input
                                    v-model="defaultDueTime"
                                    type="time"
                                    class="form-input"
                                />
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    "无项目"分类名称
                                    <span class="form-hint">
                                        没有归属到任何项目的任务，在项目树/项目筛选里显示的分类名字
                                    </span>
                                </label>
                                <input
                                    v-model="inboxLabel"
                                    class="form-input"
                                    maxlength="20"
                                    placeholder="无项目"
                                />
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    通知自动消失时间
                                    <span class="form-hint">
                                        错误提示悬浮通知出现后，多少秒自动收回，单位秒，1-30
                                    </span>
                                </label>
                                <input
                                    v-model.number="notificationDurationSeconds"
                                    type="number"
                                    min="1"
                                    max="30"
                                    class="form-input"
                                />
                            </div>
                        </template>

                        <!-- 外观 -->
                        <template v-else-if="activeSection === 'appearance'">
                            <div class="form-row">
                                <label class="form-label">
                                    配色方案
                                    <span class="form-hint">
                                        内置预设，或数据目录 themes/ 下你自己放的自定义文件
                                    </span>
                                </label>
                                <select v-model="colorScheme" class="form-input">
                                    <option
                                        v-for="opt in colorSchemeOptions"
                                        :key="opt.id"
                                        :value="opt.id"
                                    >
                                        {{ opt.name }}
                                    </option>
                                </select>
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    深浅模式
                                </label>
                                <SegmentedControl
                                    :options="themeModeOptions"
                                    v-model="themeMode"
                                />
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    圆角
                                    <span class="form-hint">
                                        影响按钮/输入框/卡片/图节点的圆角，0-24
                                    </span>
                                </label>
                                <input
                                    v-model.number="cornerRadius"
                                    type="range"
                                    min="0"
                                    max="24"
                                />
                                <span class="form-hint">{{ cornerRadius }}px</span>
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    界面风格
                                    <span class="form-hint">
                                        只影响顶栏/侧栏/右键菜单/弹窗/日期图标颜色选择器/统计卡片/按钮等控件，不影响图谱任务节点的状态色
                                    </span>
                                </label>
                                <SegmentedControl
                                    :options="uiStyleOptions"
                                    v-model="uiStyle"
                                />
                            </div>
                        </template>

                        <!-- 时长格式 -->
                        <template v-else-if="activeSection === 'duration'">
                            <div class="form-row">
                                <label class="form-label">
                                    计时时长格式
                                    <span class="form-hint">
                                        记号沿用 strftime 的 %
                                        前缀写法，自己拼写想要的格式
                                    </span>
                                </label>
                                <input
                                    v-model="durationFormat"
                                    class="form-input"
                                    placeholder="%H:%M:%S"
                                />

                                <div class="duration-preview">
                                    预览：<span class="duration-preview-value">{{
                                        durationPreview
                                    }}</span>
                                </div>

                                <div class="duration-presets">
                                    <button
                                        v-for="p in DURATION_PRESETS"
                                        :key="p"
                                        type="button"
                                        class="duration-preset-btn"
                                        :class="{
                                            active: durationFormat === p,
                                        }"
                                        @click="durationFormat = p"
                                    >
                                        {{ p }}
                                    </button>
                                </div>

                                <table class="duration-token-table">
                                    <tbody>
                                        <tr
                                            v-for="t in DURATION_TOKENS"
                                            :key="t.token"
                                        >
                                            <td class="duration-token">
                                                {{ t.token }}
                                            </td>
                                            <td class="duration-token-desc">
                                                {{ t.desc }}
                                            </td>
                                        </tr>
                                    </tbody>
                                </table>
                            </div>
                        </template>

                        <!-- 图谱显示 -->
                        <template v-else-if="activeSection === 'graph'">
                            <div class="form-row font-field-row">
                                <label class="form-label">
                                    节点字体
                                    <span class="form-hint">
                                        任务看板图谱里任务卡片文字单独使用的字体；留空则跟随"通用"里的全局字体
                                    </span>
                                </label>
                                <input
                                    v-model="nodeFontFamily"
                                    class="form-input"
                                    placeholder="留空跟随全局字体…"
                                    @focus="showNodeFontDropdown = true"
                                    @blur="hideNodeFontDropdownDelayed"
                                />

                                <div
                                    class="font-preview"
                                    :style="{ fontFamily: nodeFontFamily || fontFamily || undefined }"
                                >
                                    预览 Preview 任务管理 0123
                                </div>

                                <div
                                    v-if="showNodeFontDropdown && filteredNodeFontOptions.length > 0"
                                    class="suggest-dropdown"
                                >
                                    <button
                                        v-for="f in filteredNodeFontOptions"
                                        :key="f"
                                        type="button"
                                        class="suggest-dropdown-item"
                                        :style="{ fontFamily: f }"
                                        @mousedown.prevent="selectNodeFont(f)"
                                    >
                                        {{ f }}
                                    </button>
                                </div>
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    任务卡片显示信息
                                    <span class="form-hint">
                                        控制任务看板图谱里，任务卡片上显示哪些信息、以及每项的标签文字；
                                        开启的项即使任务没有对应的值也会显示（标为"无"），关闭的项鼠标悬浮在卡片上时仍会在详情窗里显示。
                                        节点大小会跟着开启的项数自动调整。
                                    </span>
                                </label>

                                <div class="node-display-row">
                                    <label class="checkbox-row">
                                        <input
                                            v-model="nodeShowProject"
                                            type="checkbox"
                                        />
                                        所属项目
                                    </label>
                                    <input
                                        v-model="nodeLabelProject"
                                        class="form-input node-label-input"
                                        placeholder="标签文字"
                                        :maxlength="NODE_LABEL_MAX_LENGTH"
                                    />
                                    <button
                                        type="button"
                                        class="label-reset-btn"
                                        title="恢复默认标签"
                                        :disabled="nodeLabelProject === NODE_LABELS.project"
                                        @click="nodeLabelProject = NODE_LABELS.project"
                                    >
                                        ↺
                                    </button>
                                </div>

                                <div class="node-display-row">
                                    <label class="checkbox-row">
                                        <input v-model="nodeShowDue" type="checkbox" />
                                        截止日期
                                    </label>
                                    <input
                                        v-model="nodeLabelDue"
                                        class="form-input node-label-input"
                                        placeholder="标签文字"
                                        :maxlength="NODE_LABEL_MAX_LENGTH"
                                    />
                                    <button
                                        type="button"
                                        class="label-reset-btn"
                                        title="恢复默认标签"
                                        :disabled="nodeLabelDue === NODE_LABELS.due"
                                        @click="nodeLabelDue = NODE_LABELS.due"
                                    >
                                        ↺
                                    </button>
                                </div>

                                <div class="node-display-row">
                                    <label class="checkbox-row">
                                        <input
                                            v-model="nodeShowPriority"
                                            type="checkbox"
                                        />
                                        优先级
                                    </label>
                                    <input
                                        v-model="nodeLabelPriority"
                                        class="form-input node-label-input"
                                        placeholder="标签文字"
                                        :maxlength="NODE_LABEL_MAX_LENGTH"
                                    />
                                    <button
                                        type="button"
                                        class="label-reset-btn"
                                        title="恢复默认标签"
                                        :disabled="nodeLabelPriority === NODE_LABELS.priority"
                                        @click="nodeLabelPriority = NODE_LABELS.priority"
                                    >
                                        ↺
                                    </button>
                                </div>

                                <div class="node-display-row">
                                    <label class="checkbox-row">
                                        <input
                                            v-model="nodeShowRecur"
                                            type="checkbox"
                                        />
                                        重复任务标记
                                    </label>
                                    <input
                                        v-model="nodeLabelRecur"
                                        class="form-input node-label-input"
                                        placeholder="标签文字"
                                        :maxlength="NODE_LABEL_MAX_LENGTH"
                                    />
                                    <button
                                        type="button"
                                        class="label-reset-btn"
                                        title="恢复默认标签"
                                        :disabled="nodeLabelRecur === NODE_LABELS.recur"
                                        @click="nodeLabelRecur = NODE_LABELS.recur"
                                    >
                                        ↺
                                    </button>
                                </div>
                            </div>
                        </template>

                        <!-- 快捷键 -->
                        <template v-else-if="activeSection === 'shortcuts'">
                            <div class="form-row">
                                <label class="form-label">
                                    自定义快捷键
                                    <span class="form-hint">
                                        点击按键框进入录制状态，按下新的组合键即可；Esc
                                        取消录制。改绑到已被占用的组合键会提示冲突。
                                    </span>
                                </label>
                                <button type="button" class="mode-btn" @click="resetAllShortcuts">
                                    全部恢复默认
                                </button>
                            </div>

                            <div
                                v-for="action in shortcutActions"
                                :key="action.id"
                                class="form-row shortcut-edit-row"
                            >
                                <label class="form-label">
                                    {{ action.label }}
                                    <span class="form-hint">{{ action.category }}</span>
                                </label>
                                <div class="shortcut-edit-controls">
                                    <button
                                        type="button"
                                        class="mode-btn shortcut-key-btn"
                                        :class="{ active: recordingActionId === action.id }"
                                        @click="startRecording(action.id)"
                                    >
                                        {{
                                            recordingActionId === action.id
                                                ? "按下新的组合键…"
                                                : bindingLabel(action)
                                        }}
                                    </button>
                                    <button
                                        type="button"
                                        class="mode-btn shortcut-reset-btn"
                                        @click="resetOneShortcut(action.id)"
                                    >
                                        恢复默认
                                    </button>
                                </div>

                                <div
                                    v-if="shortcutConflict && shortcutConflict.actionId === action.id"
                                    class="form-hint shortcut-conflict"
                                >
                                    当前绑定给『{{ shortcutConflict.conflictWith.label }}』，
                                    <button type="button" class="mode-btn" @click="resolveConflictOverwrite">
                                        改绑给这个
                                    </button>
                                    <button type="button" class="mode-btn" @click="cancelConflict">
                                        取消
                                    </button>
                                </div>
                            </div>
                        </template>

                        <!-- 关于 -->
                        <template v-else-if="activeSection === 'about'">
                            <div class="form-row">
                                <label class="form-label">
                                    当前版本
                                    <span class="form-hint">v{{ currentVersion }}</span>
                                </label>
                            </div>

                            <div class="form-row">
                                <label class="form-label">检查更新</label>

                                <button
                                    v-if="
                                        updateStatus === 'idle' ||
                                        updateStatus === 'up-to-date' ||
                                        updateStatus === 'error'
                                    "
                                    class="mode-btn"
                                    @click="checkForUpdate"
                                >
                                    检查更新
                                </button>
                                <span
                                    v-else-if="updateStatus === 'checking'"
                                    class="form-hint"
                                >
                                    正在检查…
                                </span>

                                <div
                                    v-if="updateStatus === 'up-to-date'"
                                    class="form-hint"
                                >
                                    已是最新版本
                                </div>

                                <div v-if="updateStatus === 'error'" class="form-hint update-error">
                                    检查失败：{{ updateError }}
                                </div>

                                <template v-if="updateStatus === 'available'">
                                    <div class="form-hint">
                                        发现新版本 v{{ latestVersion }}
                                    </div>
                                    <div v-if="updateNotes" class="update-notes">
                                        {{ updateNotes }}
                                    </div>
                                    <button
                                        class="mode-btn active"
                                        @click="downloadAndInstallUpdate"
                                    >
                                        下载并安装
                                    </button>
                                </template>

                                <div
                                    v-if="updateStatus === 'downloading'"
                                    class="form-hint"
                                >
                                    正在下载安装…
                                </div>

                                <template v-if="updateStatus === 'ready'">
                                    <div class="form-hint">
                                        已安装完成，重启后生效
                                    </div>
                                    <button
                                        class="mode-btn active"
                                        @click="restartToApply"
                                    >
                                        立即重启
                                    </button>
                                </template>
                            </div>
                        </template>
                    </div>
                </div>

                <div class="modal-footer">
                    <button class="btn-cancel" @click="emit('close')">
                        取消
                    </button>
                    <button class="btn-submit" @click="submit">保存</button>
                </div>
            </div>
        </div>
    </Teleport>
</template>

<style scoped>
.modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

.modal {
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 10px;
    width: 560px;
    max-width: 90vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
}

.modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
}
.modal-title {
    font-size: 1.1538rem;
    font-weight: 700;
    color: var(--cyan);
}
.modal-close {
    font-size: 1.5385rem;
    color: var(--fg-dim);
    line-height: 1;
    padding: 0 4px;
    border-radius: 4px;
    transition: color 0.15s;
}
.modal-close:hover {
    color: var(--fg);
}

/* 左侧菜单 + 右侧内容 */
.modal-layout {
    display: flex;
    flex: 1;
    min-height: 0;
}

.settings-nav {
    flex-shrink: 0;
    width: 130px;
    padding: 12px 8px;
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.nav-item {
    text-align: left;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 0.9231rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.nav-item:hover {
    color: var(--fg);
    background: rgba(0, 0, 0, 0.05);
}
.nav-item.active {
    color: var(--blue);
    background: rgba(122, 162, 247, 0.12);
    font-weight: 600;
}

.modal-body {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.form-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
}
.form-label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 0.9231rem;
    font-weight: 700;
    color: var(--fg);
}
.form-hint {
    font-size: 0.8462rem;
    font-weight: 400;
    color: var(--fg-dim);
}
.form-input {
    width: 100%;
}

/* 字体选择：自定义下拉框需要一个定位锚点 */
.font-field-row {
    position: relative;
}
.font-preview {
    padding: 10px 12px;
    border-radius: 6px;
    background: var(--bg-dark);
    border: 1px solid var(--border);
    color: var(--fg);
    font-size: 1rem;
}

/* 字体候选下拉框，样式对齐任务表单里的项目/标签下拉框 */
.suggest-dropdown {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 10;
    max-height: 220px;
    overflow-y: auto;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow:
        0 10px 28px rgba(0, 0, 0, 0.22),
        0 2px 6px rgba(0, 0, 0, 0.12);
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.suggest-dropdown::-webkit-scrollbar {
    width: 4px;
}
.suggest-dropdown::-webkit-scrollbar-thumb {
    background: var(--fg-dark);
    border-radius: 2px;
}
.suggest-dropdown-item {
    text-align: left;
    padding: 6px 10px;
    border-radius: 5px;
    font-size: 0.9231rem;
    color: var(--fg);
    transition:
        background 0.12s,
        color 0.12s;
}
.suggest-dropdown-item:hover {
    background: var(--bg-select);
    color: var(--magenta);
}
.empty-hint {
    display: block;
    padding: 8px;
    color: var(--fg-dim);
    font-size: 0.8462rem;
}

.checkbox-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.9231rem;
    font-weight: 400;
    color: var(--fg);
    cursor: pointer;
}
.checkbox-row input[type="checkbox"] {
    width: 15px;
    height: 15px;
    cursor: pointer;
}

.node-display-row {
    display: flex;
    align-items: center;
    gap: 10px;
}
.node-display-row .checkbox-row {
    flex: 0 0 130px;
    flex-shrink: 0;
}
.node-label-input {
    flex: 1;
    min-width: 0;
}
/* 固定宽高的图标按钮：一直占着这块地方，不会因为出现/消失导致左边输入框跟着变宽变窄 */
.label-reset-btn {
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 5px;
    border: 1px solid var(--border);
    font-size: 0.9231rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.label-reset-btn:hover:not(:disabled) {
    color: var(--blue);
    border-color: var(--blue);
}
.label-reset-btn:disabled {
    opacity: 0.3;
    cursor: default;
}

.duration-preview {
    font-size: 0.8462rem;
    color: var(--fg-dim);
}
.duration-preview-value {
    color: var(--blue);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
}

.duration-presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
}
.duration-preset-btn {
    padding: 3px 10px;
    border-radius: 5px;
    border: 1px solid var(--border);
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.duration-preset-btn:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}
.duration-preset-btn.active {
    color: var(--blue);
    border-color: var(--blue);
    background: rgba(122, 162, 247, 0.1);
}

.duration-token-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8462rem;
}
.duration-token-table td {
    padding: 3px 0;
    border-top: 1px solid var(--border);
}
.duration-token {
    width: 70px;
    color: var(--magenta);
    font-weight: 700;
    font-family: monospace;
}
.duration-token-desc {
    color: var(--fg-dim);
}

.mode-btn {
    padding: 5px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 0.9231rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.mode-btn:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}
.mode-btn.active {
    color: var(--blue);
    border-color: var(--blue);
    background: rgba(122, 162, 247, 0.1);
}

.update-notes {
    font-size: 0.8462rem;
    color: var(--fg-dim);
    white-space: pre-wrap;
    background: rgba(0, 0, 0, 0.15);
    border-radius: 6px;
    padding: 8px 10px;
}
.update-error {
    color: var(--red, #f7768e);
}

.shortcut-edit-row {
    flex-direction: column;
    align-items: flex-start;
}
.shortcut-edit-controls {
    display: flex;
    gap: 8px;
    margin-top: 4px;
}
.shortcut-key-btn {
    min-width: 140px;
}
.shortcut-key-btn.active {
    box-shadow: inset 0 0 0 1.5px var(--blue);
}
.shortcut-conflict {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--red);
    margin-top: 4px;
}

.modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 14px 20px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
}
.btn-submit {
    padding: 7px 20px;
    border-radius: 6px;
    background: var(--blue);
    color: var(--bg);
    font-weight: 700;
    font-size: 1rem;
    transition: opacity 0.15s;
}
.btn-submit:hover {
    opacity: 0.85;
}
.btn-cancel {
    padding: 7px 16px;
    border-radius: 6px;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    font-size: 1rem;
    transition: all 0.15s;
}
.btn-cancel:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}
</style>
