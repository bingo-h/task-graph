<!--
  @Component: 根组件
  @Description: 负责从后端拉取组件，管理全局状态，协调三个面板，分发写操作到API
  @Author: Bin.H
  @Date: 2026-05-23
-->

<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

import TaskFormModal from "./components/TaskFormModal.vue";
import ProjectTree from "./components/ProjectTree.vue";
import TaskGraph from "./components/TaskGraph.vue";
import TaskDetail from "./components/TaskDetail.vue";
import SettingsModal from "./components/SettingsModal.vue";
import Dashboard from "./components/Dashboard.vue";
import TimeEntryNoteModal from "./components/TimeEntryNoteModal.vue";
import { computeHighlight } from "./composables/useLayout";
import {
    formatDuration,
    setDurationFormat,
    DEFAULT_DURATION_FORMAT,
} from "./composables/useDuration";
import {
    fetchTasks,
    createProject,
    setProjectArchived,
    setProjectStage,
    trashProject,
    restoreProject,
    purgeProject,
    moveProject,
    getSettings,
    saveSettings,
    addTask,
    modifyTask,
    doneTask,
    undoneTask,
    setTaskToday,
    deleteTask,
    startTimer,
    stopTimer,
    saveTimeEntryNote,
    deleteTimeEntry,
} from "./composables/useApi";

// 无边框窗口：自定义标题栏控制
const appWindow = getCurrentWindow();
function minimizeWindow() {
    appWindow.minimize();
}
function toggleMaximizeWindow() {
    appWindow.toggleMaximize();
}
function closeWindow() {
    appWindow.close();
}

// 全局状态
const nodes = ref([]); // 所有任务节点
const edges = ref([]); // 所有边
const projects = ref({});
const plannedProjectRoots = ref([]);
const activeProjectRoots = ref([]);
const archivedProjectRoots = ref([]);
const trashProjectRoots = ref([]);

// 应用设置（保存在独立的 settings.json 中，方便导出/同步）
const settings = ref({
    trash_retention_days: 30,
    font_size: 14,
    duration_format: DEFAULT_DURATION_FORMAT,
});
const showSettings = ref(false);

/** 把字体大小应用到全局 CSS 变量 */
function applyFontSize(size) {
    document.documentElement.style.setProperty("--app-font-size", `${size}px`);
}

// 当前页面："home" 首页仪表盘 / "board" 任务看板（原有的三栏视图）
const currentPage = ref("home");

/**
 * 首页"今日任务"里点击某个任务，跳转到任务看板并选中它
 *
 * @description 由 Dashboard 的 @jump-to-task 事件触发
 * @param {string} uuid - 任务 UUID
 */
function onJumpToTask(uuid) {
    currentPage.value = "board";
    selectedUUID.value = uuid;
}

// 当前状态
const selectedUUID = ref(null);
const selectedProject = ref(null);
const hlMode = ref("ancestors"); // 高亮模式
const loading = ref(false);
const error = ref("");

// 派生状态
const selectedTask = computed(
    () => nodes.value.find((n) => n.uuid === selectedUUID.value) || null,
); // 当前选中的任务对象，未选中时为null

const highlightSet = computed(() =>
    computeHighlight(selectedUUID.value, edges.value, hlMode.value),
);

// ----------------------------------------
// 全局计时状态（标题栏悬浮秒表）
// ----------------------------------------
/** 当前正在计时的任务（全局至多一个） */
const activeTimingTask = computed(
    () => nodes.value.find((n) => n.is_timing) || null,
);

const nowTick = ref(Date.now()); // 每秒刷新，驱动秒表实时跳动
const tickTimer = setInterval(() => {
    nowTick.value = Date.now();
}, 1000);
onUnmounted(() => clearInterval(tickTimer));

/** 本次专注时长（秒）：从这一段计时开始到现在，不含之前的历史累计 */
const activeSessionSeconds = computed(() => {
    const since = activeTimingTask.value?.active_since;
    if (!since) return 0;
    return Math.max(0, Math.floor((nowTick.value - new Date(since).getTime()) / 1000));
});

// 添加/修改任务
const showModal = ref(false); // 是否显示添加任务界面
const modalPrefill = ref(null); // null = 新建，任务对象 = 修改

// 计时记录回忆总结弹窗
const noteModal = ref({
    visible: false,
    entryId: null,
    heading: "回忆总结",
    title: "",
    body: "",
});
const taskDetailRef = ref(null); // 保存总结后用来刷新 TaskDetail 的计时记录列表

// ----------------------------------------
// 数据加载
// ----------------------------------------
/** 从后端拉取全部数据并更新本地状态 */
async function load() {
    loading.value = true;
    error.value = "";

    try {
        const data = await fetchTasks();
        nodes.value = data.nodes;
        edges.value = data.edges;
        projects.value = data.projects;
        plannedProjectRoots.value = data.planned_project_roots;
        activeProjectRoots.value = data.active_project_roots;
        archivedProjectRoots.value = data.archived_project_roots;
        trashProjectRoots.value = data.trash_project_roots;
    } catch (e) {
        error.value = e.message;
    } finally {
        loading.value = false;
    }
}

/** 加载应用设置并应用字体大小、计时时长显示格式 */
async function loadSettings() {
    try {
        settings.value = await getSettings();
        applyFontSize(settings.value.font_size);
        setDurationFormat(settings.value.duration_format);
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 写操作成功后用后端返回的最新数据刷新
 */
function applyUpdate(data) {
    nodes.value = data.nodes;
    edges.value = data.edges;
    projects.value = data.projects;
    plannedProjectRoots.value = data.planned_project_roots;
    activeProjectRoots.value = data.active_project_roots;
    archivedProjectRoots.value = data.archived_project_roots;
    trashProjectRoots.value = data.trash_project_roots;

    if (
        selectedUUID.value &&
        !data.nodes.find((n) => n.uuid === selectedUUID.value)
    ) {
        selectedUUID.value = null;
    }
}

/**
 * 新建项目（可在没有任何任务的情况下独立创建）
 *
 * @description 由 ProjectTree 的 @create-project 事件触发
 * @param {string} path - 项目路径，如 "personal.reading"
 * @param {string} stage - "planned"（计划中）| "active"（进行中）
 */
async function onCreateProject(path, stage) {
    try {
        applyUpdate(await createProject(path, stage));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 切换项目归档状态（归档会级联到所有子项目）
 *
 * @description 由 ProjectTree 的 @toggle-archive 事件触发
 * @param {string} path - 项目路径
 * @param {boolean} archived - 归档目标状态
 */
async function onToggleArchive(path, archived) {
    try {
        applyUpdate(await setProjectArchived(path, archived));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 设置项目所属阶段（计划中 / 进行中），只能对顶层项目操作，子项目跟随顶层项目一起移动
 *
 * @description 由 ProjectTree 的 @set-stage 事件触发
 * @param {string} path - 顶层项目路径
 * @param {string} stage - "planned" | "active"
 */
async function onSetStage(path, stage) {
    try {
        applyUpdate(await setProjectStage(path, stage));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 将项目移入废纸篓（级联到所有子项目，可从废纸篓恢复，不会立即删除任务）
 *
 * @description 由 ProjectTree 的 @trash-project 事件触发
 * @param {string} path - 项目路径
 */
async function onTrashProject(path) {
    try {
        applyUpdate(await trashProject(path));
        if (selectedProject.value === path) selectedProject.value = null;
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 从废纸篓恢复项目
 *
 * @description 由 ProjectTree 的 @restore-project 事件触发
 * @param {string} path - 项目路径
 */
async function onRestoreProject(path) {
    try {
        applyUpdate(await restoreProject(path));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 彻底删除项目，级联删除该项目及所有子项目下的任务，不可恢复，需用户二次确认
 *
 * @description 由 ProjectTree 的 @purge-project 事件触发
 * @param {string} path - 项目路径
 */
async function onPurgeProject(path) {
    if (!confirm(`确认彻底删除项目"${path}"？其下所有子项目和任务都无法恢复。`))
        return;

    try {
        applyUpdate(await purgeProject(path));
        if (selectedProject.value === path) selectedProject.value = null;
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 移动项目（及其子项目、任务）到新的父项目下，或移到顶层
 *
 * @description 由 ProjectTree 的 @move-project 事件触发（拖拽或右键菜单）
 * @param {string} path - 被移动的项目路径
 * @param {?string} newParent - 新的父项目路径，null 表示移到顶层
 */
async function onMoveProject(path, newParent) {
    try {
        applyUpdate(await moveProject(path, newParent));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 保存应用设置
 *
 * @description 由 SettingsModal 的 @save 事件触发
 * @param {object} newSettings - { trash_retention_days, font_size }
 */
async function onSaveSettings(newSettings) {
    try {
        settings.value = await saveSettings(newSettings);
        applyFontSize(settings.value.font_size);
        setDurationFormat(settings.value.duration_format);
        showSettings.value = false;
    } catch (e) {
        error.value = e.message;
    }
}

// ----------------------------------------
// 弹窗操作
// ----------------------------------------
/**
 * 打开新建任务弹窗
 */
function openAdd() {
    modalPrefill.value = null;
    showModal.value = true;
}

/**
 * 打开修改任务弹窗
 * @param {object} task - 任务实例对象
 */
function openModify(task) {
    modalPrefill.value = task;
    showModal.value = true;
}

/**
 * 弹窗提交
 * @param {string} mode - 模式：新建/修改
 * @param {string} uuid
 * @param {object} fields - 结构化任务字段
 */
async function onModalSubmit({ mode, uuid, fields }) {
    try {
        if (mode === "add") {
            applyUpdate(await addTask(fields));
        } else {
            applyUpdate(await modifyTask(uuid, fields));
        }

        showModal.value = false;
        error.value = "";
    } catch (e) {
        error.value = e.message;
    }
}

// ----------------------------------------
// 任务操作
// ----------------------------------------
/**
 * 标记任务完成
 *
 * @description 由 TaskDetail 的 @done 事件触发
 * @param {string} uuid - 任务UUID
 */
async function onDone(uuid) {
    try {
        applyUpdate(await doneTask(uuid));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 取消任务完成，恢复为待办
 *
 * @description 由 TaskDetail 的 @undone 事件触发
 * @param {string} uuid - 任务UUID
 */
async function onUndone(uuid) {
    try {
        applyUpdate(await undoneTask(uuid));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 设置/取消"今日任务"标记
 *
 * @description 由 TaskDetail 的 @set-today 事件触发
 * @param {string} uuid - 任务 UUID
 * @param {boolean} marked - 目标状态
 */
async function onSetToday(uuid, marked) {
    try {
        applyUpdate(await setTaskToday(uuid, marked));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 详情栏内联编辑备注，失焦后保存
 *
 * @description 由 TaskDetail 的 @update-annotation 事件触发
 * @param {string} uuid - 任务 UUID
 * @param {string} text - 备注全文，空字符串表示清空
 */
async function onUpdateAnnotation(uuid, text) {
    try {
        applyUpdate(
            await modifyTask(uuid, {
                annotation: text || null,
                clear_annotation: !text,
            }),
        );
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 开始为指定任务计时
 *
 * @description 由 TaskDetail 的 @start-timer 事件触发
 * @param {string} uuid - 任务UUID
 */
async function onStartTimer(uuid) {
    try {
        applyUpdate(await startTimer(uuid));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 停止当前正在进行的计时；结束后若有对应的计时记录，
 * 弹窗询问这段专注的回忆总结（可跳过）
 *
 * @description 由 TaskDetail 的 @stop-timer 事件触发
 */
async function onStopTimer() {
    try {
        const data = await stopTimer();
        applyUpdate(data);

        if (data.stopped_entry_id) {
            noteModal.value = {
                visible: true,
                entryId: data.stopped_entry_id,
                heading: "这段专注的回忆总结",
                title: "",
                body: "",
            };
        }
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 点击"计时记录"里已有的总结标题，打开弹窗查看/修改
 *
 * @description 由 TaskDetail 的 @edit-time-entry-note 事件触发
 * @param {object} entry - 计时记录对象 { id, note_title, note_body, ... }
 */
function onEditTimeEntryNote(entry) {
    noteModal.value = {
        visible: true,
        entryId: entry.id,
        heading: "回忆总结",
        title: entry.note_title || "",
        body: entry.note_body || "",
    };
}

/** 保存回忆总结弹窗的内容，成功后刷新任务详情里的计时记录列表 */
async function onSaveTimeEntryNote({ title, body }) {
    try {
        await saveTimeEntryNote(
            noteModal.value.entryId,
            title || null,
            body || null,
        );
        noteModal.value.visible = false;
        await taskDetailRef.value?.refresh();
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 删除某段计时记录，需用户二次确认，成功后刷新任务详情和总耗时
 *
 * @description 由 TaskDetail 的 @delete-time-entry 事件触发
 * @param {number} entryId - 计时记录 id
 */
async function onDeleteTimeEntry(entryId) {
    if (!confirm("确认删除这段计时记录？此操作不可恢复。")) return;

    try {
        applyUpdate(await deleteTimeEntry(entryId));
        await taskDetailRef.value?.refresh();
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 删除任务，需用户二次确认
 *
 * @param {string} uuid - 任务 UUID
 */
async function onDelete(uuid) {
    if (!confirm("确认删除此任务？")) return;

    try {
        applyUpdate(await deleteTask(uuid));
    } catch (e) {
        error.value = e.message;
    }
}

onMounted(() => {
    load();
    loadSettings();
});
</script>

<template>
    <div class="app">
        <!-- 顶部导航栏（兼具自定义标题栏，可拖拽） -->
        <header class="topbar" data-tauri-drag-region>
            <span class="app-title" data-tauri-drag-region>task-web</span>

            <!-- 页面切换：首页仪表盘 / 任务看板 -->
            <div class="page-nav">
                <button
                    class="page-nav-btn"
                    :class="{ active: currentPage === 'home' }"
                    @click="currentPage = 'home'"
                >
                    首页
                </button>
                <button
                    class="page-nav-btn"
                    :class="{ active: currentPage === 'board' }"
                    @click="currentPage = 'board'"
                >
                    任务看板
                </button>
            </div>

            <!-- 当前活跃计时任务：悬浮秒表，点击任务名可跳转，点击方块停止 -->
            <div v-if="activeTimingTask" class="active-timer-pill">
                <span class="active-timer-dot"></span>
                <span
                    class="active-timer-desc"
                    @click="selectedUUID = activeTimingTask.uuid"
                >
                    {{ activeTimingTask.description }}
                </span>
                <span class="active-timer-clock">
                    {{ formatDuration(activeSessionSeconds) }}
                </span>
                <button
                    class="active-timer-stop"
                    title="停止计时"
                    @click="onStopTimer"
                >
                    ■
                </button>
            </div>

            <!-- 添加任务按钮 -->
            <button class="btn-add-toggle" @click="openAdd">+ 添加任务</button>

            <!-- 刷新按钮 -->
            <button class="btn-refresh" @click="load" :disabled="loading">
                {{ loading ? "加载中…" : "↺ 刷新" }}
            </button>

            <!-- 设置按钮 -->
            <button
                class="btn-refresh"
                title="设置"
                @click="showSettings = true"
            >
                ⚙ 设置
            </button>

            <!-- 窗口控制按钮（无边框窗口自绘） -->
            <div class="window-controls">
                <button
                    class="win-btn win-min"
                    title="最小化"
                    @click="minimizeWindow"
                >
                    &#x2212;
                </button>
                <button
                    class="win-btn win-max"
                    title="最大化/还原"
                    @click="toggleMaximizeWindow"
                >
                    &#x25A1;
                </button>
                <button
                    class="win-btn win-close"
                    title="关闭"
                    @click="closeWindow"
                >
                    &#x2715;
                </button>
            </div>
        </header>

        <!-- 错误提示 -->
        <div v-if="error" class="error-bar" @click="error = ''">
            ⚠ {{ error }} <span class="dismiss">×</span>
        </div>

        <!-- 首页仪表盘 -->
        <Dashboard
            v-show="currentPage === 'home'"
            :nodes="nodes"
            @jump-to-task="onJumpToTask"
        />

        <!-- 主体三栏（任务看板） -->
        <div v-show="currentPage === 'board'" class="main">
            <ProjectTree
                :projects="projects"
                :planned-roots="plannedProjectRoots"
                :active-roots="activeProjectRoots"
                :archived-roots="archivedProjectRoots"
                :trash-roots="trashProjectRoots"
                :selected="selectedProject"
                @select="selectedProject = $event"
                @create-project="onCreateProject"
                @toggle-archive="onToggleArchive"
                @set-stage="onSetStage"
                @trash-project="onTrashProject"
                @restore-project="onRestoreProject"
                @purge-project="onPurgeProject"
                @move-project="onMoveProject"
            />

            <TaskGraph
                :nodes="nodes"
                :edges="edges"
                :selected="selectedUUID"
                :highlight-set="highlightSet"
                :project-filter="selectedProject"
                @select="
                    (uuid) => {
                        console.log('app received:', uuid);
                        selectedUUID = uuid;
                    }
                "
            />

            <TaskDetail
                ref="taskDetailRef"
                :task="selectedTask"
                :all-tasks="nodes"
                @done="onDone"
                @undone="onUndone"
                @start-timer="onStartTimer"
                @stop-timer="onStopTimer"
                @delete="onDelete"
                @modify="openModify"
                @select="selectedUUID = $event"
                @edit-time-entry-note="onEditTimeEntryNote"
                @delete-time-entry="onDeleteTimeEntry"
                @set-today="onSetToday"
                @update-annotation="onUpdateAnnotation"
            />
        </div>

        <!-- 添加任务弹出框 -->
        <TaskFormModal
            :visible="showModal"
            :prefill="modalPrefill"
            :projects="projects"
            :default-project="selectedProject"
            :all-tasks="nodes"
            @close="showModal = false"
            @submit="onModalSubmit"
        />

        <!-- 设置弹出框 -->
        <SettingsModal
            :visible="showSettings"
            :settings="settings"
            :highlight-mode="hlMode"
            @close="showSettings = false"
            @save="onSaveSettings"
            @update:highlight-mode="hlMode = $event"
        />

        <!-- 计时记录回忆总结弹窗 -->
        <TimeEntryNoteModal
            :visible="noteModal.visible"
            :heading="noteModal.heading"
            :initial-title="noteModal.title"
            :initial-body="noteModal.body"
            @close="noteModal.visible = false"
            @save="onSaveTimeEntryNote"
        />
    </div>
</template>

<style scoped>
.app {
    display: flex;
    flex-direction: column;
    height: 100vh;
}

.topbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    height: 44px;
    background: var(--bg-dark);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
}

.app-title {
    font-weight: 700;
    font-size: 1.1538rem;
    color: var(--cyan);
    margin-right: 8px;
}

/* 页面切换 */
.page-nav {
    display: flex;
    gap: 2px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 2px;
}
.page-nav-btn {
    padding: 4px 12px;
    border-radius: 5px;
    font-size: 0.8462rem;
    font-weight: 600;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.page-nav-btn:hover {
    color: var(--fg);
}
.page-nav-btn.active {
    color: var(--blue);
    background: var(--bg-panel);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
}

/* 标题栏悬浮秒表：当前活跃计时任务 */
.active-timer-pill {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 10px 3px 8px;
    border-radius: 999px;
    background: rgba(158, 206, 106, 0.12);
    border: 1px solid rgba(158, 206, 106, 0.35);
    max-width: 280px;
}

.active-timer-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--green);
    flex-shrink: 0;
    animation: active-timer-pulse 1.6s ease-in-out infinite;
}
@keyframes active-timer-pulse {
    0%,
    100% {
        opacity: 1;
    }
    50% {
        opacity: 0.35;
    }
}

.active-timer-desc {
    font-size: 0.9231rem;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
}
.active-timer-desc:hover {
    color: var(--green);
}

.active-timer-clock {
    font-size: 0.9231rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--green);
    flex-shrink: 0;
}

.active-timer-stop {
    font-size: 0.6923rem;
    color: var(--green);
    opacity: 0.7;
    flex-shrink: 0;
    padding: 2px;
    transition: opacity 0.15s;
}
.active-timer-stop:hover {
    opacity: 1;
}

.btn-add-toggle {
    padding: 4px 14px;
    border-radius: 6px;
    background: var(--blue);
    color: var(--bg);
    font-weight: 600;
    font-size: 0.9231rem;
    transition: opacity 0.15s;
}

.btn-add-toggle:hover {
    opacity: 0.85;
}

.btn-refresh {
    padding: 4px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    font-size: 0.9231rem;
    transition: all 0.15s;
}

.btn-refresh:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}

.btn-refresh:disabled {
    opacity: 0.4;
    cursor: default;
}

/* 无边框窗口的自绘控制按钮 */
.window-controls {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
    padding-right: 6px;
}

.win-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition:
        background 0.12s,
        color 0.12s;
}

.win-btn:hover {
    background: rgba(0, 0, 0, 0.06);
    color: var(--fg);
}

.win-btn.win-close:hover {
    background: var(--red);
    color: #fff;
}

/* 错误提示栏 */
.error-bar {
    padding: 8px 16px;
    background: rgba(247, 118, 142, 0.15);
    color: var(--red);
    border-bottom: 1px solid var(--red);
    font-size: 0.9231rem;
    cursor: pointer;
    flex-shrink: 0;
}

.dismiss {
    margin-left: 8px;
    font-weight: 700;
}

/* 主体三栏 */
.main {
    display: flex;
    flex: 1;
    overflow: hidden;
}
</style>
