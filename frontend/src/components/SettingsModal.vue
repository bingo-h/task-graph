<!--
  @Component: 设置弹窗组件
  @Description:
    左侧菜单分区展示各类设置：通用（高亮模式/废纸篓天数/字体大小）、
    时长格式（计时时长的显示格式）。保存在独立 settings.json 中。
  @Author: Bin.H
  @Date: 2026-08-15
-->

<script setup>
import { ref, computed, watch } from "vue";
import { formatDuration, DEFAULT_DURATION_FORMAT } from "../composables/useDuration";
import { getVersion } from "@tauri-apps/api/app";
import { listSystemFonts } from "../composables/useApi";
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
    { key: "duration", label: "时长格式" },
    { key: "graph", label: "图谱显示" },
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
    (visible) => {
        if (!visible) return;
        activeSection.value = "general";
        trashRetentionDays.value = props.settings.trash_retention_days ?? 30;
        fontSize.value = props.settings.font_size ?? 14;
        fontFamily.value = props.settings.font_family || "sans-serif";
        nodeFontFamily.value = props.settings.node_font_family || "";
        loadSystemFontsOnce();
        durationFormat.value =
            props.settings.duration_format || DEFAULT_DURATION_FORMAT;
        defaultDueTime.value = props.settings.default_due_time || "23:59";
        nodeShowProject.value = props.settings.node_show_project ?? true;
        nodeShowDue.value = props.settings.node_show_due ?? true;
        nodeShowPriority.value = props.settings.node_show_priority ?? true;
        nodeShowRecur.value = props.settings.node_show_recur ?? true;
        nodeLabelProject.value = props.settings.node_label_project || NODE_LABELS.project;
        nodeLabelDue.value = props.settings.node_label_due || NODE_LABELS.due;
        nodeLabelPriority.value = props.settings.node_label_priority || NODE_LABELS.priority;
        nodeLabelRecur.value = props.settings.node_label_recur || NODE_LABELS.recur;
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
        node_show_project: nodeShowProject.value,
        node_show_due: nodeShowDue.value,
        node_show_priority: nodeShowPriority.value,
        node_show_recur: nodeShowRecur.value,
        node_label_project: nodeLabelProject.value.trim() || NODE_LABELS.project,
        node_label_due: nodeLabelDue.value.trim() || NODE_LABELS.due,
        node_label_priority: nodeLabelPriority.value.trim() || NODE_LABELS.priority,
        node_label_recur: nodeLabelRecur.value.trim() || NODE_LABELS.recur,
    });
}
</script>

<template>
    <Teleport to="body">
        <div v-if="visible" class="modal-overlay" @click.self="emit('close')">
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
                                <div class="mode-group">
                                    <button
                                        v-for="m in highlightModeOptions"
                                        :key="m.key"
                                        class="mode-btn"
                                        :class="{
                                            active: highlightMode === m.key,
                                        }"
                                        @click="
                                            emit(
                                                'update:highlight-mode',
                                                m.key,
                                            )
                                        "
                                    >
                                        {{ m.label }}
                                    </button>
                                </div>
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    废纸篓保留天数
                                    <span class="form-hint">
                                        超过此天数的项目会在下次打开应用时自动彻底删除，0
                                        表示永不自动删除
                                    </span>
                                </label>
                                <input
                                    v-model.number="trashRetentionDays"
                                    type="number"
                                    min="0"
                                    max="3650"
                                    class="form-input"
                                />
                            </div>

                            <div class="form-row">
                                <label class="form-label">
                                    字体大小
                                    <span class="form-hint">单位像素，8-32</span>
                                </label>
                                <input
                                    v-model.number="fontSize"
                                    type="number"
                                    min="8"
                                    max="32"
                                    class="form-input"
                                />
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

.mode-group {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
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
