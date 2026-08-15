<!--
    @Component: 右侧任务详情面板
    @Description:
        显示选中任务的完整信息，支持：
        - 标记完成 / 删除
        - 修改任务（原生 taskwarrior 语法输入框）
        - 显示依赖链（前置任务 / 被依赖任务）
        - 锁定状态说明
    @Author: Bin.H
-->

<script setup>
import { ref, computed, watch, onUnmounted } from "vue";
import constants from "../config/constants";
import { listTimeEntries } from "../composables/useApi";
import { formatDuration } from "../composables/useDuration";

const props = defineProps({
    task: { type: Object, default: null }, // 选中的任务对象，null表示未选中
    allTasks: { type: Array, required: true }, // 所有任务列表，用于查找依赖任务描述
});

const emit = defineEmits([
    "done",
    "undone",
    "start-timer",
    "stop-timer",
    "delete",
    "modify",
    "select",
    "edit-time-entry-note",
    "delete-time-entry",
]);

const modifyInput = ref("");
const modifyError = ref("");

// 切换任务时重置修改输入框
watch(
    () => props.task,
    () => {
        modifyInput.value = "";
        modifyError.value = "";
    },
);

// ----------------------------------------
// 计时
// ----------------------------------------
const timeEntries = ref([]); // 当前选中任务的计时记录（按开始时间倒序）
const nowTick = ref(Date.now()); // 每秒刷新一次，用于实时显示正在进行的计时时长

let tickTimer = null;
tickTimer = setInterval(() => {
    nowTick.value = Date.now();
}, 1000);
onUnmounted(() => clearInterval(tickTimer));

/** 拉取当前任务的计时记录明细 */
async function loadTimeEntries() {
    if (!props.task) {
        timeEntries.value = [];
        return;
    }
    try {
        timeEntries.value = await listTimeEntries(props.task.uuid);
    } catch {
        timeEntries.value = [];
    }
}

// 每次拿到新的任务数据（切换任务，或完成/计时等写操作触发刷新）时，
// 记下这一刻的 total_seconds 快照及拿到快照的时间点，
// 用于计算之后每秒实时递增的显示值
const snapshotSeconds = ref(0);
const snapshotAt = ref(Date.now());
watch(
    () => props.task,
    async (task) => {
        snapshotSeconds.value = task?.total_seconds ?? 0;
        snapshotAt.value = Date.now();
        await loadTimeEntries();
    },
    { immediate: true },
);

// 供父组件在保存计时回忆总结后调用，刷新当前列表
defineExpose({ refresh: loadTimeEntries });

/** 当前任务累计耗时（秒），正在计时时随 nowTick 实时递增 */
const displayTotalSeconds = computed(() => {
    if (!props.task) return 0;
    if (!props.task.is_timing) return props.task.total_seconds;

    const elapsedSinceSnapshot = Math.max(
        0,
        Math.floor((nowTick.value - snapshotAt.value) / 1000),
    );
    return snapshotSeconds.value + elapsedSinceSnapshot;
});

/** 格式化时间为 HH:MM */
function formatTime(iso) {
    if (!iso) return "";
    const d = new Date(iso);
    return `${String(d.getHours()).padStart(2, "0")}:${String(
        d.getMinutes(),
    ).padStart(2, "0")}`;
}

// 计时记录明细：按时间范围筛选
const timeFilters = [
    { key: "all", label: "全部" },
    { key: "today", label: "今天" },
    { key: "week", label: "本周" },
    { key: "month", label: "本月" },
];
const timeFilter = ref("all");

/** 筛选范围的起始日期（YYYY-MM-DD），"all" 时为 null 表示不限 */
const filterSinceDate = computed(() => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);

    if (timeFilter.value === "today") return today;

    if (timeFilter.value === "week") {
        // 周一为一周起始
        const day = (today.getDay() + 6) % 7;
        return new Date(today.getTime() - day * 86400000);
    }

    if (timeFilter.value === "month") {
        return new Date(today.getFullYear(), today.getMonth(), 1);
    }

    return null;
});

/** 按当前筛选范围过滤后的计时记录 */
const filteredEntries = computed(() => {
    const since = filterSinceDate.value;
    if (!since) return timeEntries.value;

    return timeEntries.value.filter(
        (entry) => new Date(entry.start).getTime() >= since.getTime(),
    );
});

/** 按日期（YYYY-MM-DD）分组的计时记录，用于详情栏展示 */
const entriesByDate = computed(() => {
    const groups = {};
    for (const entry of filteredEntries.value) {
        const date = entry.start.slice(0, 10);
        (groups[date] ??= []).push(entry);
    }
    return Object.entries(groups).sort((a, b) => (a[0] < b[0] ? 1 : -1));
});

const showTimeEntries = ref(false); // 计时记录明细默认折叠

// UUID 查找任务描述，用于依赖链显示
const uuidMap = computed(() =>
    Object.fromEntries(props.allTasks.map((t) => [t.uuid, t])),
);

/** 前置任务列表 (此任务依赖的任务) */
const dependsTasks = computed(() =>
    (props.task?.depends || [])
        .map((uuid) => uuidMap.value[uuid])
        .filter(Boolean),
);

/** 被依赖任务列表 (依赖此任务的任务) */
const blockingTasks = computed(() =>
    (props.task?.blocking || [])
        .map((uuid) => uuidMap.value[uuid])
        .filter(Boolean),
);

// ----------------------------------------
// 面板宽度拖拽调整
// ----------------------------------------
const PANEL_WIDTH_KEY = "task-detail-panel-width";
const MIN_PANEL_WIDTH = 220;
const MAX_PANEL_WIDTH = 600;

const panelWidth = ref(
    Number(localStorage.getItem(PANEL_WIDTH_KEY)) || 280,
);

/** 拖拽面板左侧边缘调整宽度（面板在窗口右侧，向左拖动即变宽） */
function startResize(e) {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = panelWidth.value;

    function onMove(moveEvent) {
        const delta = startX - moveEvent.clientX;
        panelWidth.value = Math.min(
            MAX_PANEL_WIDTH,
            Math.max(MIN_PANEL_WIDTH, startWidth + delta),
        );
    }

    function onUp() {
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        localStorage.setItem(PANEL_WIDTH_KEY, String(panelWidth.value));
    }

    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
}

/** 格式化日期为易读形式 */
function formatDate(iso) {
    if (!iso) return "";
    return iso.slice(0, 10);
}

/** 优先级标签颜色 */
function priorityClass(p) {
    return { H: "priority-h", M: "priority-m", L: "priority-l" }[p] || "";
}

/** 状态显示文字 */
function statusLabel(s) {
    return (
        {
            pending: constants.PENDING,
            completed: constants.COMPLETED,
            waiting: constants.WAITING,
            deleted: constants.DELETED,
        }[s] || s
    );
}
</script>

<template>
    <aside class="task-detail" :style="{ width: panelWidth + 'px' }">
        <!-- 拖拽调整面板宽度 -->
        <div class="resize-handle" @mousedown="startResize"></div>

        <!-- 未选中状态 -->
        <div v-if="!task" class="detail-empty">
            <span>点击节点查看任务详情</span>
        </div>

        <!-- 任务详情 -->
        <template v-else>
            <!-- 标题区 -->
            <div class="detail-header">
                <h2 class="detail-title">{{ task.description }}</h2>

                <div class="detail-badges">
                    <span class="badge-status" :class="`status-${task.status}`">
                        {{ statusLabel(task.status) }}
                    </span>

                    <span
                        v-if="task.priority"
                        class="badge-priority"
                        :class="priorityClass(task.priority)"
                    >
                        {{ task.priority }}
                    </span>

                    <span v-if="task.is_locked" class="badge-extra locked">
                        🔒 锁定
                    </span>

                    <span v-if="task.is_overdue" class="badge-extra overdue">
                        ⚠ 逾期
                    </span>

                    <span v-if="task.is_overdue" class="badge-extra today">
                        📅 今日到期
                    </span>
                </div>
            </div>

            <!-- 基本信息 -->
            <div class="detail-section">
                <div v-if="task.project" class="detail-row">
                    <span class="detail-key">项目</span>
                    <span class="detail-val project-val">
                        {{ task.project }}
                    </span>
                </div>

                <div v-if="task.due" class="detail-row">
                    <span class="detail-key">截止</span>
                    <span
                        class="detail-val"
                        :class="{
                            'val-overdue': task.is_overdue,
                            'val-today': task.is_due_today,
                        }"
                    >
                        {{ formatDate(task.due) }}
                    </span>
                </div>

                <div v-if="task.scheduled" class="detail-row">
                    <span class="detail-key">计划</span>
                    <span class="detail-val">
                        {{ formatDate(task.scheduled) }}
                    </span>
                </div>

                <div class="detail-row">
                    <span class="detail-key">紧迫度</span>
                    <span class="detail-val">
                        {{ task.urgency.toFixed(2) }}
                    </span>
                </div>

                <div v-if="task.tags?.length" class="detail-row">
                    <span class="detail-key">标签</span>
                    <span class="detail-val tags-val">
                        <span
                            v-for="tag in task.tags"
                            :key="tag"
                            class="tag-chip"
                        >
                            {{ tag }}
                        </span>
                    </span>
                </div>
            </div>

            <!-- 锁定说明 -->
            <div v-if="task.is_locked" class="detail-section locked-hint">
                <p>🔒 此任务有未完成的前置任务，无法开始</p>
            </div>

            <!-- 前置任务 (此任务依赖的任务) -->
            <div v-if="dependsTasks.length > 0" class="detail-section">
                <div class="section-title">前置任务</div>
                <div
                    v-for="dep in dependsTasks"
                    :key="dep.uuid"
                    class="dep-item"
                    :class="{ 'dep-done': dep.status === 'completed' }"
                    @click="emit('select', dep.uuid)"
                >
                    <span class="dep-icon">
                        {{ dep.status === "completed" ? "✔" : "○" }}
                    </span>
                    <span class="dep-desc">
                        {{ dep.description }}
                    </span>
                </div>
            </div>

            <!-- 被依赖任务 (依赖此任务的任务) -->
            <div v-if="blockingTasks.length > 0" class="detail-section">
                <div class="section-title">后续任务</div>
                <div
                    v-for="blockingTask in blockingTasks"
                    :key="blockingTask.uuid"
                    class="dep-item"
                    @click="emit('select', blockingTask.uuid)"
                >
                    <span class="dep-icon">→</span>
                    <span class="dep-desc">
                        {{ blockingTask.description }}
                    </span>

                    <span v-if="blockingTask.is_locked" class="dep-locked">
                        🔒
                    </span>
                </div>
            </div>

            <!-- 备注 -->
            <div>
                <div class="section-title">备注</div>
                <div
                    v-for="annotation in task.annotations"
                    :key="annotation.entry"
                    class="annotation"
                >
                    <span class="annotation-date">
                        {{ formatDate(ann.entry) }}
                    </span>

                    <span class="annotation-text">
                        {{ annotation.description }}
                    </span>
                </div>
            </div>

            <!-- 操作按钮 -->
            <div class="detail-actions">
                <button
                    v-if="task.status === 'pending'"
                    class="btn-done"
                    :class="{ 'btn-disabled': task.is_locked }"
                    :disabled="task.is_locked"
                    :title="task.is_locked ? '存在未完成的前置任务，无法完成' : ''"
                    @click="emit('done', task.uuid)"
                >
                    ✔ 完成
                </button>

                <button
                    v-if="task.status === 'completed'"
                    class="btn-undone"
                    @click="emit('undone', task.uuid)"
                >
                    ↺ 取消完成
                </button>

                <!-- 修改：emit 完整任务对象，由父组件（App.vue）打开 TaskFormModal -->
                <button class="btn-modify" @click="emit('modify', task)">
                    ✏ 修改
                </button>

                <button class="btn-delete" @click="emit('delete', task.uuid)">
                    ✗ 删除
                </button>
            </div>

            <!-- 计时 -->
            <div class="detail-section">
                <div class="timer-row">
                    <div class="timer-total">
                        <span class="detail-key">耗时</span>
                        <span
                            class="detail-val timer-duration"
                            :class="{ 'timer-active': task.is_timing }"
                        >
                            {{ formatDuration(displayTotalSeconds) }}
                        </span>
                    </div>

                    <button
                        v-if="task.is_timing"
                        class="btn-timer btn-timer-stop"
                        @click="emit('stop-timer')"
                    >
                        ■ 停止计时
                    </button>
                    <button
                        v-else-if="task.status === 'pending'"
                        class="btn-timer btn-timer-start"
                        :class="{ 'btn-disabled': task.is_locked }"
                        :disabled="task.is_locked"
                        :title="task.is_locked ? '存在未完成的前置任务，无法开始计时' : ''"
                        @click="emit('start-timer', task.uuid)"
                    >
                        ▶ 开始计时
                    </button>
                </div>

                <!-- 计时记录明细，按日期分组，可按时间范围筛选 -->
                <div v-if="timeEntries.length > 0" class="time-entries">
                    <button
                        class="time-entries-toggle"
                        @click="showTimeEntries = !showTimeEntries"
                    >
                        {{ showTimeEntries ? "▾" : "▸" }} 计时记录
                        ({{ filteredEntries.length }})
                    </button>

                    <template v-if="showTimeEntries">
                        <div class="time-filter-group">
                            <button
                                v-for="f in timeFilters"
                                :key="f.key"
                                class="time-filter-btn"
                                :class="{ active: timeFilter === f.key }"
                                @click="timeFilter = f.key"
                            >
                                {{ f.label }}
                            </button>
                        </div>

                        <div
                            v-if="entriesByDate.length === 0"
                            class="empty-hint"
                        >
                            此时间范围内暂无计时记录
                        </div>

                        <div v-else class="time-entries-list">
                            <div
                                v-for="[date, entries] in entriesByDate"
                                :key="date"
                                class="time-entries-group"
                            >
                                <div class="time-entries-date">
                                    {{ date }}
                                </div>
                                <div
                                    v-for="entry in entries"
                                    :key="entry.id"
                                    class="time-entry-row"
                                >
                                    <span class="time-entry-range">
                                        {{ formatTime(entry.start) }} –
                                        {{
                                            entry.end
                                                ? formatTime(entry.end)
                                                : "进行中"
                                        }}
                                    </span>
                                    <span
                                        class="time-entry-duration"
                                        :class="{
                                            'timer-active': !entry.end,
                                        }"
                                    >
                                        {{
                                            formatDuration(
                                                entry.end
                                                    ? (new Date(entry.end) -
                                                          new Date(
                                                              entry.start,
                                                          )) /
                                                          1000
                                                    : (nowTick -
                                                          new Date(
                                                              entry.start,
                                                          ).getTime()) /
                                                          1000,
                                            )
                                        }}
                                    </span>

                                    <!-- 回忆总结：默认只显示一个折叠图标，点击弹窗查看/修改完整标题和正文 -->
                                    <button
                                        class="time-entry-note-icon"
                                        :class="{ 'has-note': entry.note_title }"
                                        :title="
                                            entry.note_title || '添加回忆总结'
                                        "
                                        @click="
                                            emit('edit-time-entry-note', entry)
                                        "
                                    >
                                        📝
                                    </button>

                                    <!-- 删除这一段计时记录（不可恢复） -->
                                    <button
                                        class="time-entry-delete"
                                        title="删除这段计时记录"
                                        @click="
                                            emit('delete-time-entry', entry.id)
                                        "
                                    >
                                        ✗
                                    </button>
                                </div>
                            </div>
                        </div>
                    </template>
                </div>
            </div>

            <!-- UUID 尾部显示（前 8 位） -->
            <div class="detail-uuid">{{ task.uuid.slice(0, 8) }}…</div>
        </template>
    </aside>
</template>

<style scoped>
/* 面板容器 */
.task-detail {
    position: relative;
    flex-shrink: 0;
    background: var(--bg-panel);
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 16px;
    gap: 12px;
}
.task-detail::-webkit-scrollbar {
    width: 4px;
}
.task-detail::-webkit-scrollbar-thumb {
    background: var(--fg-dark);
    border-radius: 2px;
}

/* 拖拽调整宽度的把手 */
.resize-handle {
    position: absolute;
    left: -3px;
    top: 0;
    bottom: 0;
    width: 6px;
    cursor: col-resize;
    z-index: 5;
}
.resize-handle:hover,
.resize-handle:active {
    background: rgba(122, 162, 247, 0.4);
}

/* 未选中任何任务提示 */
.detail-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-dim);
    font-size: 1rem;
}

/* 标题区 */
.detail-header {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.detail-title {
    font-size: 1.1538rem;
    font-weight: 700;
    color: var(--fg);
    line-height: 1.4;
    word-break: break-word;
}

.detail-badges {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
}

/* 状态徽章 */
.badge-status {
    font-size: 0.8462rem;
    padding: 2px 8px;
    border-radius: 4px;
    font-weight: 600;
}
.status-pending {
    background: rgba(122, 162, 247, 0.2);
    color: var(--blue);
}
.status-completed {
    background: rgba(158, 206, 106, 0.2);
    color: var(--green);
}
.status-waiting {
    background: rgba(224, 175, 104, 0.2);
    color: var(--yellow);
}
.status-deleted {
    background: rgba(100, 100, 100, 0.2);
    color: var(--fg-dim);
}

/* 优先级徽章 */
.badge-priority {
    font-size: 0.8462rem;
    padding: 2px 8px;
    border-radius: 4px;
    font-weight: 700;
}
.priority-h {
    background: rgba(247, 118, 142, 0.2);
    color: var(--red);
}
.priority-m {
    background: rgba(224, 175, 104, 0.2);
    color: var(--yellow);
}
.priority-l {
    background: rgba(122, 162, 247, 0.2);
    color: var(--blue);
}

/* 状态徽章 (锁定/逾期/今日) */
.badge-extra {
    font-size: 0.8462rem;
}
.badge-extra.locked {
    color: var(--yellow);
}
.badge-extra.overdue {
    color: var(--red);
}
.badge-extra.today {
    color: var(--orange);
}

/* 分区 */
.detail-section {
    border-top: 1px solid var(--border);
    padding-top: 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
}

.section-title {
    font-size: 0.8462rem;
    font-weight: 700;
    color: var(--fg-dim);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 2px;
}

.detail-row {
    display: flex;
    gap: 10px;
    align-items: baseline;
}

.detail-key {
    font-size: 0.8462rem;
    color: var(--fg-dim);
    width: 44px;
    flex-shrink: 0;
}

.detail-val {
    font-size: 0.9231rem;
    color: var(--fg);
    word-break: break-all;
}
.project-val {
    color: var(--blue);
}
.val-overdue {
    color: var(--red);
}
.val-today {
    color: var(--orange);
}

/* 标签 */
.tags-val {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
}
.tag-chip {
    font-size: 0.8462rem;
    padding: 1px 6px;
    border-radius: 3px;
    background: rgba(187, 154, 247, 0.15);
    color: var(--magenta);
}

/* 计时 */
.timer-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
}
.timer-total {
    display: flex;
    align-items: baseline;
    gap: 10px;
}
.timer-duration {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
}
.timer-duration.timer-active {
    color: var(--green);
}
.btn-timer {
    padding: 5px 12px;
    border-radius: 6px;
    font-size: 0.9231rem;
    font-weight: 600;
    transition: background 0.15s;
    flex-shrink: 0;
}
.btn-timer-start {
    background: rgba(158, 206, 106, 0.2);
    color: var(--green);
    border: 1px solid rgba(158, 206, 106, 0.3);
}
.btn-timer-start:hover {
    background: rgba(158, 206, 106, 0.35);
}
.btn-timer-stop {
    background: rgba(247, 118, 142, 0.15);
    color: var(--red);
    border: 1px solid rgba(247, 118, 142, 0.3);
}
.btn-timer-stop:hover {
    background: rgba(247, 118, 142, 0.3);
}

.time-entries {
    margin-top: 4px;
}
.time-entries-toggle {
    font-size: 0.8462rem;
    color: var(--fg-dim);
    padding: 2px 0;
}
.time-entries-toggle:hover {
    color: var(--fg);
}
.time-filter-group {
    display: flex;
    gap: 4px;
    margin-top: 6px;
}
.time-filter-btn {
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.time-filter-btn:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}
.time-filter-btn.active {
    background: rgba(122, 162, 247, 0.15);
    color: var(--blue);
    border-color: var(--blue);
}
.time-entries-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 6px;
    max-height: 180px;
    overflow-y: auto;
}
.time-entries-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.time-entries-date {
    font-size: 0.7692rem;
    color: var(--fg-dim);
    font-weight: 700;
}
.time-entry-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    font-size: 0.8462rem;
    color: var(--fg);
    padding: 1px 0 1px 8px;
}
.time-entry-duration {
    font-variant-numeric: tabular-nums;
    color: var(--fg-dim);
}
.time-entry-duration.timer-active {
    color: var(--green);
}

/* 回忆总结：默认折叠成一个小图标，不占用额外的行，点击才弹窗展开详情 */
.time-entry-note-icon {
    flex-shrink: 0;
    font-size: 0.7692rem;
    line-height: 1;
    padding: 2px;
    border-radius: 4px;
    opacity: 0.25;
    transition: all 0.15s;
}
.time-entry-note-icon:hover {
    opacity: 1;
    background: rgba(0, 0, 0, 0.05);
}
.time-entry-note-icon.has-note {
    opacity: 0.9;
}

/* 删除按钮：默认隐藏，悬停这一行时才显示，避免误触 */
.time-entry-delete {
    flex-shrink: 0;
    font-size: 0.6923rem;
    line-height: 1;
    padding: 2px 3px;
    border-radius: 4px;
    color: var(--fg-dark);
    opacity: 0;
    transition: all 0.15s;
}
.time-entry-row:hover .time-entry-delete {
    opacity: 1;
}
.time-entry-delete:hover {
    color: var(--red);
    background: rgba(247, 118, 142, 0.15);
}

/* 锁定提示区 */
.locked-hint {
    font-size: 0.9231rem;
    color: var(--yellow);
    background: rgba(224, 175, 104, 0.1);
    padding: 8px;
    border-radius: 6px;
    border-color: rgba(224, 175, 104, 0.3) !important;
}

/* 依赖链条目 */
.dep-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.9231rem;
    transition: background 0.12s;
}
.dep-item:hover {
    background: rgba(0, 0, 0, 0.05);
}
.dep-item.dep-done {
    opacity: 0.5;
}
.dep-icon {
    color: var(--fg-dim);
    flex-shrink: 0;
}
.dep-desc {
    flex: 1;
    color: var(--fg);
}
.dep-locked {
    font-size: 0.8462rem;
}

/* 备注 */
.annotation {
    display: flex;
    gap: 8px;
    font-size: 0.8462rem;
}
.annotation-date {
    color: var(--fg-dim);
    flex-shrink: 0;
}
.annotation-text {
    color: var(--fg);
}

/* 操作按钮区 */
.detail-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    border-top: 1px solid var(--border);
    padding-top: 10px;
}

.btn-done {
    flex: 1;
    padding: 6px;
    border-radius: 6px;
    background: rgba(158, 206, 106, 0.2);
    color: var(--green);
    font-size: 0.9231rem;
    font-weight: 600;
    border: 1px solid rgba(158, 206, 106, 0.3);
    transition: background 0.15s;
}
.btn-done:hover {
    background: rgba(158, 206, 106, 0.35);
}

.btn-disabled,
.btn-disabled:hover {
    background: rgba(0, 0, 0, 0.05);
    color: var(--fg-dim);
    border-color: var(--border);
    cursor: not-allowed;
}

.btn-undone {
    flex: 1;
    padding: 6px;
    border-radius: 6px;
    background: rgba(224, 175, 104, 0.2);
    color: var(--orange, #e0af68);
    font-size: 0.9231rem;
    font-weight: 600;
    border: 1px solid rgba(224, 175, 104, 0.3);
    transition: background 0.15s;
}
.btn-undone:hover {
    background: rgba(224, 175, 104, 0.35);
}

.btn-modify {
    flex: 1;
    padding: 6px;
    border-radius: 6px;
    background: rgba(122, 162, 247, 0.15);
    color: var(--blue);
    font-size: 0.9231rem;
    font-weight: 600;
    border: 1px solid rgba(122, 162, 247, 0.3);
    transition: background 0.15s;
}
.btn-modify:hover {
    background: rgba(122, 162, 247, 0.3);
}

.btn-delete {
    padding: 6px 12px;
    border-radius: 6px;
    background: rgba(247, 118, 142, 0.1);
    color: var(--red);
    font-size: 0.9231rem;
    border: 1px solid rgba(247, 118, 142, 0.2);
    transition: background 0.15s;
}
.btn-delete:hover {
    background: rgba(247, 118, 142, 0.25);
}

/* UUID 尾部 */
.detail-uuid {
    font-size: 0.7692rem;
    color: var(--fg-dark);
    margin-top: auto;
    padding-top: 8px;
}
</style>
