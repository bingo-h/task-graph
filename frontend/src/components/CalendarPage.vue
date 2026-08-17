<!--
  @Component: 日历页
  @Description:
    月视图：展示所有重复任务列表 + 当前连续完成天数，月历网格每天用圆点/图标
    展示当天各重复任务的打卡结果。点击某一天切换到日视图，显示这一天每个时段做的事
    （复用全部计时记录，按时间轴排布）。
  @Author: Bin.H
-->

<script setup>
import { ref, computed, onMounted, watch } from "vue";
import { listAllTimeEntries, listRecurLog } from "../composables/useApi";
import { formatDuration } from "../composables/useDuration";
import { DEFAULT_TAG_COLOR } from "../composables/useTagColor";

const props = defineProps({
    nodes: { type: Array, default: () => [] },
    // 父组件用 v-show 切换页面，不会重新挂载，切回这一页时要重新拉一次数据
    visible: { type: Boolean, default: true },
});

const emit = defineEmits(["jump-to-task"]);

const view = ref("month"); // "month" | "day"
const selectedDate = ref(""); // "YYYY-MM-DD"，只在 day 视图下有意义

const today = new Date();
const viewYear = ref(today.getFullYear());
const viewMonth = ref(today.getMonth()); // 0-11

// ----------------------------------------
// 数据加载
// ----------------------------------------
const recurringTasks = computed(() => props.nodes.filter((n) => n.is_recurring));

const entries = ref([]); // 全部计时记录，日视图用
const recurLogByTask = ref({}); // task_uuid -> RecurLogEntry[]（按 cycle_due 倒序）
const loading = ref(true);
const error = ref("");

async function loadAll() {
    loading.value = true;
    error.value = "";
    try {
        const [allEntries, ...logs] = await Promise.all([
            listAllTimeEntries(),
            ...recurringTasks.value.map((t) => listRecurLog(t.uuid)),
        ]);
        entries.value = allEntries;

        const byTask = {};
        recurringTasks.value.forEach((t, i) => {
            byTask[t.uuid] = logs[i];
        });
        recurLogByTask.value = byTask;
    } catch (e) {
        error.value = e.message;
    } finally {
        loading.value = false;
    }
}
onMounted(loadAll);
watch(
    () => props.visible,
    (v) => {
        if (v) loadAll();
    },
);

/**
 * 某个重复任务的"有效完成记录"：recur_log 的历史 + 当前这一轮如果已完成但还没被
 * 回滚算法结算进 recur_log（要等下一个周期到来才会写入），补一条合成记录，
 * 这样月历格子和连续天数才能立刻反映"刚刚打卡"的这一次，不用等到下个周期才看到
 */
function effectiveLog(task) {
    const log = recurLogByTask.value[task.uuid] || [];
    if (task.status !== "completed" || !task.due) return log;

    const cycleDate = task.due.slice(0, 10);
    const alreadyLogged = log.some((e) => e.cycle_due.slice(0, 10) === cycleDate);
    if (alreadyLogged) return log;

    const outcome = task.end && task.end <= task.due ? "completed_on_time" : "completed_late";
    return [
        { id: -1, task_uuid: task.uuid, cycle_due: task.due, outcome, completed_at: task.end },
        ...log,
    ];
}

/** 连续完成天数：从最近一个周期往前数，遇到第一条 missed 就停 */
function computeStreak(log) {
    let streak = 0;
    for (const entry of log) {
        if (entry.outcome === "missed") break;
        streak += 1;
    }
    return streak;
}

const recurringTaskRows = computed(() =>
    recurringTasks.value
        .map((t) => ({
            task: t,
            streak: computeStreak(effectiveLog(t)),
        }))
        .sort((a, b) => b.streak - a.streak),
);

function taskDotColor(task) {
    return task.color || DEFAULT_TAG_COLOR;
}

// ----------------------------------------
// 月视图
// ----------------------------------------
const MONTH_LABEL = [
    "一月", "二月", "三月", "四月", "五月", "六月",
    "七月", "八月", "九月", "十月", "十一月", "十二月",
];
const WEEKDAY_HEADER = ["一", "二", "三", "四", "五", "六", "日"];

function prevMonth() {
    if (viewMonth.value === 0) {
        viewMonth.value = 11;
        viewYear.value -= 1;
    } else {
        viewMonth.value -= 1;
    }
}
function nextMonth() {
    if (viewMonth.value === 11) {
        viewMonth.value = 0;
        viewYear.value += 1;
    } else {
        viewMonth.value += 1;
    }
}
function jumpToCurrentMonth() {
    viewYear.value = today.getFullYear();
    viewMonth.value = today.getMonth();
}

/** 月历网格的日期数组：补齐首尾让整月凑成完整的周（周一开头），格子外的日期为 null */
const monthGridDays = computed(() => {
    const first = new Date(viewYear.value, viewMonth.value, 1);
    const daysInMonth = new Date(viewYear.value, viewMonth.value + 1, 0).getDate();
    const leading = (first.getDay() + 6) % 7; // 0=周一

    const cells = [];
    for (let i = 0; i < leading; i++) cells.push(null);
    for (let d = 1; d <= daysInMonth; d++) {
        cells.push(
            `${viewYear.value}-${String(viewMonth.value + 1).padStart(2, "0")}-${String(d).padStart(2, "0")}`,
        );
    }
    while (cells.length % 7 !== 0) cells.push(null);
    return cells;
});

const todayDateStr = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;

/** 每天的打卡记录：日期 -> [{task, outcome}]，用于格子渲染 */
const dayRecords = computed(() => {
    const map = {};
    for (const t of recurringTasks.value) {
        for (const entry of effectiveLog(t)) {
            const date = entry.cycle_due.slice(0, 10);
            (map[date] ??= []).push({ task: t, outcome: entry.outcome });
        }
    }
    return map;
});

function recordsForDay(dateStr) {
    return dateStr ? dayRecords.value[dateStr] || [] : [];
}

const MAX_DOTS_PER_CELL = 6;

function openDay(dateStr) {
    if (!dateStr) return;
    selectedDate.value = dateStr;
    view.value = "day";
}

// ----------------------------------------
// 日视图：0-24 点时间轴，按时段展示计时记录
// ----------------------------------------
const uuidToTask = computed(() => Object.fromEntries(props.nodes.map((t) => [t.uuid, t])));

/** "YYYY-MM-DD" 对应的本地日历天起止时刻（毫秒时间戳）；计时记录 start/end
 *  是真实发生过的时刻，日视图应该按用户本地的一天划分，而不是 UTC 的一天 */
function localDayBounds(dateStr) {
    const [y, m, d] = dateStr.split("-").map(Number);
    const start = new Date(y, m - 1, d).getTime();
    return { start, end: start + 86400 * 1000 };
}

const dayEntries = computed(() => {
    if (!selectedDate.value) return [];
    const { start: dayStart, end: dayEnd } = localDayBounds(selectedDate.value);
    const now = Date.now();

    return entries.value
        .map((e) => {
            const start = new Date(e.start).getTime();
            const end = e.end ? new Date(e.end).getTime() : now;
            return { ...e, start, end };
        })
        .filter((e) => e.start < dayEnd && e.end > dayStart)
        .map((e) => ({
            ...e,
            clampedStart: Math.max(e.start, dayStart),
            clampedEnd: Math.min(e.end, dayEnd),
            task: uuidToTask.value[e.task_uuid],
        }))
        .sort((a, b) => a.clampedStart - b.clampedStart);
});

function blockStyle(entry) {
    const { start: dayStart } = localDayBounds(selectedDate.value);
    const topPct = ((entry.clampedStart - dayStart) / 86400000) * 100;
    const heightPct = Math.max(
        0.6,
        ((entry.clampedEnd - entry.clampedStart) / 86400000) * 100,
    );
    const color = entry.task?.color || "var(--blue)";
    return {
        top: `${topPct}%`,
        height: `${heightPct}%`,
        background: color,
    };
}

function formatHM(ms) {
    const d = new Date(ms);
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

const HOUR_MARKS = Array.from({ length: 25 }, (_, h) => h);

function backToMonth() {
    view.value = "month";
}
</script>

<template>
    <div class="calendar-page">
        <div class="calendar-toolbar">
            <h2 class="calendar-title">日历</h2>
            <button v-if="view === 'day'" class="back-btn" @click="backToMonth">
                ← 返回月视图
            </button>
        </div>

        <div v-if="error" class="calendar-error">⚠ {{ error }}</div>
        <div v-else-if="loading" class="calendar-empty">加载中…</div>

        <!-- 月视图 -->
        <div v-else-if="view === 'month'" class="calendar-body">
            <!-- 重复任务列表 + 连续天数 -->
            <div class="streak-panel">
                <div class="panel-title">重复任务</div>
                <div v-if="recurringTaskRows.length === 0" class="empty-hint">
                    还没有设置任何重复任务
                </div>
                <div v-else class="streak-list">
                    <button
                        v-for="row in recurringTaskRows"
                        :key="row.task.uuid"
                        class="streak-row"
                        @click="emit('jump-to-task', row.task.uuid)"
                    >
                        <span
                            class="streak-dot"
                            :style="{ background: taskDotColor(row.task) }"
                        >
                            {{ row.task.icon || "" }}
                        </span>
                        <span class="streak-desc">{{ row.task.description }}</span>
                        <span class="streak-count">🔥 {{ row.streak }}</span>
                    </button>
                </div>
            </div>

            <!-- 月历网格 -->
            <div class="month-grid-card">
                <div class="month-nav">
                    <button class="month-nav-btn" @click="prevMonth">‹</button>
                    <span class="month-label">
                        {{ viewYear }} 年 {{ MONTH_LABEL[viewMonth] }}
                    </span>
                    <button class="month-nav-btn" @click="nextMonth">›</button>
                    <button class="month-today-btn" @click="jumpToCurrentMonth">
                        今天
                    </button>
                </div>

                <div class="weekday-header">
                    <span v-for="w in WEEKDAY_HEADER" :key="w">{{ w }}</span>
                </div>

                <div class="month-grid">
                    <div
                        v-for="(dateStr, i) in monthGridDays"
                        :key="i"
                        class="day-cell"
                        :class="{
                            empty: !dateStr,
                            today: dateStr === todayDateStr,
                            clickable: !!dateStr,
                        }"
                        @click="openDay(dateStr)"
                    >
                        <template v-if="dateStr">
                            <span class="day-number">{{ Number(dateStr.slice(8)) }}</span>
                            <div class="day-dots">
                                <span
                                    v-for="(r, idx) in recordsForDay(dateStr).slice(0, MAX_DOTS_PER_CELL)"
                                    :key="idx"
                                    class="day-dot"
                                    :class="{ missed: r.outcome === 'missed' }"
                                    :style="{
                                        background:
                                            r.outcome === 'missed'
                                                ? 'transparent'
                                                : taskDotColor(r.task),
                                        borderColor: taskDotColor(r.task),
                                    }"
                                    :title="r.task.description"
                                >
                                    {{ r.outcome !== 'missed' ? r.task.icon || '' : '' }}
                                </span>
                                <span
                                    v-if="recordsForDay(dateStr).length > MAX_DOTS_PER_CELL"
                                    class="day-dot-more"
                                >
                                    +{{ recordsForDay(dateStr).length - MAX_DOTS_PER_CELL }}
                                </span>
                            </div>
                        </template>
                    </div>
                </div>
            </div>
        </div>

        <!-- 日视图：0-24 点时间轴 -->
        <div v-else class="day-view-card">
            <div class="day-view-header">{{ selectedDate }}</div>

            <div v-if="dayEntries.length === 0" class="empty-hint">
                这一天没有任何计时记录
            </div>

            <div v-else class="day-timeline">
                <div class="hour-ruler">
                    <div
                        v-for="h in HOUR_MARKS"
                        :key="h"
                        class="hour-mark"
                        :style="{ top: `${(h / 24) * 100}%` }"
                    >
                        <span class="hour-label">{{ String(h).padStart(2, '0') }}:00</span>
                    </div>
                </div>

                <div class="timeline-track">
                    <div
                        v-for="entry in dayEntries"
                        :key="entry.id"
                        class="timeline-block"
                        :style="blockStyle(entry)"
                        :title="entry.task?.description || '(已删除的任务)'"
                    >
                        <span class="block-label">
                            {{ entry.task?.icon }} {{ entry.task?.description || '(已删除的任务)' }}
                            · {{ formatHM(entry.clampedStart) }}–{{ formatHM(entry.clampedEnd) }}
                            ({{ formatDuration((entry.clampedEnd - entry.clampedStart) / 1000) }})
                        </span>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.calendar-page {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px 32px;
    display: flex;
    flex-direction: column;
    gap: 18px;
}

.calendar-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
}
.calendar-title {
    font-size: 1.2308rem;
    font-weight: 700;
    color: var(--fg);
}
.back-btn {
    padding: 5px 14px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.back-btn:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}

.calendar-error {
    padding: 8px 16px;
    border-radius: 6px;
    background: rgba(209, 36, 47, 0.1);
    color: var(--red);
    font-size: 0.9231rem;
}
.calendar-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-dim);
    font-size: 0.9231rem;
}

.calendar-body {
    display: flex;
    gap: 18px;
    flex: 1;
    min-height: 0;
}

/* 重复任务列表面板 */
.streak-panel {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex: 1;
    min-width: 220px;
    max-width: 280px;
    overflow-y: auto;
}
.panel-title {
    font-size: 0.9231rem;
    font-weight: 700;
    color: var(--fg);
}
.streak-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
}
.streak-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: 7px;
    transition: background 0.12s;
}
.streak-row:hover {
    background: var(--bg-select);
}
.streak-dot {
    flex-shrink: 0;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.8rem;
}
.streak-desc {
    flex: 1;
    min-width: 0;
    text-align: left;
    font-size: 0.8462rem;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.streak-count {
    flex-shrink: 0;
    font-size: 0.8462rem;
    font-weight: 700;
    color: var(--orange);
}

/* 月历卡片 */
.month-grid-card {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex: 3;
    min-width: 0;
}
.month-nav {
    display: flex;
    align-items: center;
    gap: 10px;
}
.month-nav-btn {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    font-size: 1.0769rem;
    transition: all 0.15s;
}
.month-nav-btn:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}
.month-label {
    font-size: 0.9231rem;
    font-weight: 700;
    color: var(--fg);
    min-width: 110px;
}
.month-today-btn {
    margin-left: auto;
    padding: 4px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.month-today-btn:hover {
    color: var(--blue);
    border-color: var(--blue);
}

.weekday-header {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    font-size: 0.7692rem;
    color: var(--fg-dim);
    text-align: center;
    padding-bottom: 4px;
    border-bottom: 1px solid var(--border);
}

.month-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    grid-auto-rows: minmax(64px, 1fr);
    gap: 4px;
    flex: 1;
}
.day-cell {
    border-radius: 6px;
    padding: 5px 6px;
    background: var(--bg);
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow: hidden;
}
.day-cell.clickable {
    cursor: pointer;
    transition: background 0.12s;
}
.day-cell.clickable:hover {
    background: var(--bg-select);
}
.day-cell.empty {
    background: transparent;
}
.day-cell.today {
    box-shadow: inset 0 0 0 1.5px var(--blue);
}
.day-number {
    font-size: 0.7692rem;
    color: var(--fg-dim);
    font-variant-numeric: tabular-nums;
}
.day-dots {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
}
.day-dot {
    width: 15px;
    height: 15px;
    border-radius: 50%;
    border: 1.5px solid transparent;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.6154rem;
    line-height: 1;
}
.day-dot.missed {
    border-style: dashed;
    opacity: 0.5;
}
.day-dot-more {
    font-size: 0.6923rem;
    color: var(--fg-dim);
    align-self: center;
}

.empty-hint {
    font-size: 0.8462rem;
    color: var(--fg-dark);
    padding: 8px 0;
}

/* 日视图 */
.day-view-card {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
}
.day-view-header {
    font-size: 1.0769rem;
    font-weight: 700;
    color: var(--fg);
}

.day-timeline {
    position: relative;
    display: flex;
    gap: 10px;
    min-height: 1440px; /* 每分钟 1px，24 小时一目了然又不会太长 */
}
.hour-ruler {
    position: relative;
    width: 48px;
    flex-shrink: 0;
}
.hour-mark {
    position: absolute;
    left: 0;
    right: 0;
    border-top: 1px dashed var(--border);
}
.hour-label {
    position: absolute;
    top: -7px;
    left: 0;
    font-size: 0.6923rem;
    color: var(--fg-dim);
    background: var(--bg-panel);
    padding-right: 4px;
}

.timeline-track {
    position: relative;
    flex: 1;
    border-left: 1px solid var(--border);
}
.timeline-block {
    position: absolute;
    left: 6px;
    right: 6px;
    border-radius: 5px;
    padding: 2px 8px;
    overflow: hidden;
    opacity: 0.85;
}
.block-label {
    font-size: 0.7692rem;
    color: #fff;
    white-space: nowrap;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
}
</style>
