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

watch(
    () => props.visible,
    (visible) => {
        if (!visible) return;
        activeSection.value = "general";
        trashRetentionDays.value = props.settings.trash_retention_days ?? 30;
        fontSize.value = props.settings.font_size ?? 14;
        durationFormat.value =
            props.settings.duration_format || DEFAULT_DURATION_FORMAT;
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
        duration_format: durationFormat.value.trim() || DEFAULT_DURATION_FORMAT,
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
