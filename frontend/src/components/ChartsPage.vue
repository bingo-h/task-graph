<!--
  @Component: 图表页
  @Description:
    专门展示统计图表的页面，通过视图切换按钮在两种图表间切换：
    - 视图1：今日任务耗时（横向柱状图），可切换按任务/按项目汇总
    - 视图2：过去一段时间的任务完成趋势（柱状图/折线图可切换）
  @Author: Bin.H
-->

<script setup>
import { ref, computed, onMounted } from "vue";
import { listAllTimeEntries } from "../composables/useApi";
import { formatDuration } from "../composables/useDuration";

const props = defineProps({
    // 全部任务节点，用于取任务描述/项目、以及任务完成趋势统计
    nodes: { type: Array, default: () => [] },
});

const emit = defineEmits(["jump-to-task"]);

// 当前展示的视图
const VIEW_OPTIONS = [
    { key: "today-time", label: "今日耗时" },
    { key: "completion-trend", label: "完成趋势" },
];
const activeView = ref("today-time");

// ----------------------------------------
// 数据加载：全部计时记录
// ----------------------------------------
const entries = ref([]);
const loading = ref(true);
const error = ref("");

async function loadEntries() {
    loading.value = true;
    error.value = "";
    try {
        entries.value = await listAllTimeEntries();
    } catch (e) {
        error.value = e.message;
    } finally {
        loading.value = false;
    }
}
onMounted(loadEntries);
defineExpose({ reload: loadEntries });

const uuidMap = computed(() =>
    Object.fromEntries(props.nodes.map((t) => [t.uuid, t])),
);

function startOfDay(d) {
    const r = new Date(d);
    r.setHours(0, 0, 0, 0);
    return r;
}

/** 紧凑的时长展示，如 "1小时20分钟" / "20分钟"，用于图表标签和提示 */
function formatCompact(seconds) {
    const totalMin = Math.round(seconds / 60);
    const h = Math.floor(totalMin / 60);
    const m = totalMin % 60;
    if (h === 0) return `${m}分钟`;
    return m === 0 ? `${h}小时` : `${h}小时${m}分钟`;
}

// ----------------------------------------
// 视图1：今日任务耗时（横向柱状图）
// ----------------------------------------
const groupBy = ref("task"); // "task" | "project"

/** 一段 [start, end) 区间与今天的重叠秒数 */
function overlapWithTodaySeconds(startMs, endMs) {
    const todayStart = startOfDay(new Date()).getTime();
    const todayEnd = todayStart + 86400 * 1000;
    const overlap = Math.min(endMs, todayEnd) - Math.max(startMs, todayStart);
    return overlap > 0 ? overlap / 1000 : 0;
}

const todayTimeRows = computed(() => {
    const now = Date.now();
    const totals = new Map(); // key -> { key, label, seconds }

    for (const entry of entries.value) {
        const startMs = new Date(entry.start).getTime();
        const endMs = entry.end ? new Date(entry.end).getTime() : now;
        const seconds = overlapWithTodaySeconds(startMs, endMs);
        if (seconds <= 0) continue;

        const task = uuidMap.value[entry.task_uuid];
        const key =
            groupBy.value === "task"
                ? entry.task_uuid
                : task?.project || "(无项目)";
        const label =
            groupBy.value === "task"
                ? task?.description || "(已删除的任务)"
                : task?.project || "(无项目)";

        const row = totals.get(key) || {
            key,
            label,
            seconds: 0,
            taskUuid: groupBy.value === "task" ? entry.task_uuid : null,
        };
        row.seconds += seconds;
        totals.set(key, row);
    }

    return [...totals.values()].sort((a, b) => b.seconds - a.seconds);
});

const todayTimeMax = computed(() =>
    Math.max(1, ...todayTimeRows.value.map((r) => r.seconds)),
);

function hbarWidthPct(seconds) {
    return Math.max(2, (seconds / todayTimeMax.value) * 100);
}

const todayTotalSeconds = computed(() =>
    todayTimeRows.value.reduce((s, r) => s + r.seconds, 0),
);

// ----------------------------------------
// 视图2：任务完成趋势（柱状图/折线图可切换）
// ----------------------------------------
const TREND_RANGE_OPTIONS = [
    { key: 7, label: "近 7 天" },
    { key: 14, label: "近 14 天" },
    { key: 30, label: "近 30 天" },
];
const trendRangeDays = ref(7);
const trendChartType = ref("bar"); // "bar" | "line"

const monthDayFormatter = {
    format: (d) =>
        `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`,
};

const trendBucketStarts = computed(() => {
    const n = trendRangeDays.value;
    const today = startOfDay(new Date());
    return Array.from(
        { length: n },
        (_, i) => new Date(today.getTime() - (n - 1 - i) * 86400 * 1000),
    );
});

/** 找出某个时间点落在哪一天的分段里，找不到返回 -1 */
function dayBucketIndexFor(time, starts) {
    for (let i = 0; i < starts.length; i++) {
        const bs = starts[i].getTime();
        if (time >= bs && time < bs + 86400 * 1000) return i;
    }
    return -1;
}

const trendData = computed(() => {
    const counts = new Array(trendBucketStarts.value.length).fill(0);
    for (const t of props.nodes) {
        if (t.status !== "completed" || !t.end) continue;
        const idx = dayBucketIndexFor(
            new Date(t.end).getTime(),
            trendBucketStarts.value,
        );
        if (idx >= 0) counts[idx] += 1;
    }
    return trendBucketStarts.value.map((d, i) => ({
        key: d.toISOString(),
        date: d,
        count: counts[i],
    }));
});

const trendTotal = computed(() =>
    trendData.value.reduce((s, d) => s + d.count, 0),
);

// Y 轴取一个"好看"的上限刻度，避免全 0 时图表塌陷
const NICE_STEPS_COUNT = [1, 2, 3, 5, 8, 10, 15, 20, 30, 50, 80, 100, 150, 200];
const trendMaxCount = computed(() => {
    const maxCount = Math.max(0, ...trendData.value.map((d) => d.count));
    return (
        NICE_STEPS_COUNT.find((s) => s >= maxCount) ||
        NICE_STEPS_COUNT[NICE_STEPS_COUNT.length - 1]
    );
});

const trendGridLines = computed(() =>
    [1, 0.5].map((r) => ({
        count: Math.round(trendMaxCount.value * r),
        pct: r * 100,
    })),
);

function trendBarHeightPct(count) {
    if (count <= 0) return 0;
    return Math.max(2, (count / trendMaxCount.value) * 100);
}

/** 横坐标标签：范围较长时每隔几天标一次，避免文字挤在一起 */
function trendXLabel(d, index) {
    const step = trendRangeDays.value <= 7 ? 1 : trendRangeDays.value <= 14 ? 2 : 5;
    return index % step === 0 ? monthDayFormatter.format(d.date) : "";
}

const trendHoverIndex = ref(null);

const trendLinePoints = computed(() =>
    trendData.value.map((d, i) => ({
        x: ((i + 0.5) / trendData.value.length) * 100,
        y: 100 - trendBarHeightPct(d.count),
    })),
);
const trendLinePolyline = computed(() =>
    trendLinePoints.value.map((p) => `${p.x},${p.y}`).join(" "),
);
const trendAreaPath = computed(() => {
    const pts = trendLinePoints.value;
    if (pts.length === 0) return "";
    const first = pts[0];
    const last = pts[pts.length - 1];
    const body = pts.map((p) => `L ${p.x},${p.y}`).join(" ");
    return `M ${first.x},100 ${body} L ${last.x},100 Z`;
});
</script>

<template>
    <div class="charts-page">
        <div class="charts-toolbar">
            <h2 class="charts-title">图表</h2>
            <div class="view-nav">
                <button
                    v-for="v in VIEW_OPTIONS"
                    :key="v.key"
                    class="view-nav-btn"
                    :class="{ active: activeView === v.key }"
                    @click="activeView = v.key"
                >
                    {{ v.label }}
                </button>
            </div>
        </div>

        <div v-if="error" class="charts-error">⚠ {{ error }}</div>
        <div v-else-if="loading" class="charts-empty">加载中…</div>

        <!-- 视图1：今日任务耗时（横向柱状图） -->
        <div v-else-if="activeView === 'today-time'" class="chart-card">
            <div class="chart-header">
                <span class="chart-title">
                    今日{{ groupBy === "task" ? "任务" : "项目" }}耗时
                    <span class="chart-hint">
                        （共 {{ formatCompact(todayTotalSeconds) }}）
                    </span>
                </span>
                <div class="group-by-group">
                    <button
                        class="group-by-btn"
                        :class="{ active: groupBy === 'task' }"
                        @click="groupBy = 'task'"
                    >
                        按任务
                    </button>
                    <button
                        class="group-by-btn"
                        :class="{ active: groupBy === 'project' }"
                        @click="groupBy = 'project'"
                    >
                        按项目
                    </button>
                </div>
            </div>

            <div v-if="todayTimeRows.length === 0" class="empty-hint">
                今天还没有任何计时记录
            </div>

            <div v-else class="hbar-list">
                <div
                    v-for="row in todayTimeRows"
                    :key="row.key"
                    class="hbar-row"
                    :class="{ clickable: !!row.taskUuid }"
                    @click="row.taskUuid && emit('jump-to-task', row.taskUuid)"
                >
                    <span class="hbar-label" :title="row.label">
                        {{ row.label }}
                    </span>
                    <div class="hbar-track">
                        <div
                            class="hbar-fill"
                            :style="{ width: `${hbarWidthPct(row.seconds)}%` }"
                        ></div>
                    </div>
                    <span class="hbar-value">
                        {{ formatDuration(row.seconds) }}
                    </span>
                </div>
            </div>
        </div>

        <!-- 视图2：任务完成趋势（柱状图/折线图可切换） -->
        <div v-else class="chart-card">
            <div class="chart-header">
                <span class="chart-title">
                    任务完成趋势
                    <span class="chart-hint">
                        （共完成 {{ trendTotal }} 个）
                    </span>
                </span>
                <div class="chart-header-actions">
                    <div class="range-group">
                        <button
                            v-for="r in TREND_RANGE_OPTIONS"
                            :key="r.key"
                            class="range-btn"
                            :class="{ active: trendRangeDays === r.key }"
                            @click="trendRangeDays = r.key"
                        >
                            {{ r.label }}
                        </button>
                    </div>
                    <div class="chart-type-group">
                        <button
                            class="chart-type-btn"
                            :class="{ active: trendChartType === 'bar' }"
                            title="柱状图"
                            @click="trendChartType = 'bar'"
                        >
                            📊
                        </button>
                        <button
                            class="chart-type-btn"
                            :class="{ active: trendChartType === 'line' }"
                            title="折线图"
                            @click="trendChartType = 'line'"
                        >
                            📈
                        </button>
                    </div>
                </div>
            </div>

            <div class="chart-plot">
                <div class="grid-lines">
                    <div
                        v-for="g in trendGridLines"
                        :key="g.count"
                        class="grid-line"
                        :style="{ bottom: `${g.pct}%` }"
                    >
                        <span class="grid-label">{{ g.count }} 个</span>
                    </div>
                </div>

                <svg
                    v-if="trendChartType === 'line'"
                    class="line-svg"
                    viewBox="0 0 100 100"
                    preserveAspectRatio="none"
                >
                    <path class="area-fill" :d="trendAreaPath" />
                    <polyline
                        class="line-path"
                        :points="trendLinePolyline"
                        vector-effect="non-scaling-stroke"
                    />
                </svg>

                <div class="bars-row">
                    <div
                        v-for="(d, i) in trendData"
                        :key="d.key"
                        class="bar-slot"
                        @mouseenter="trendHoverIndex = i"
                        @mouseleave="trendHoverIndex = null"
                    >
                        <div v-if="trendHoverIndex === i" class="bar-tooltip">
                            <div class="tooltip-date">
                                {{ monthDayFormatter.format(d.date) }}
                            </div>
                            <div class="tooltip-row">
                                完成 {{ d.count }} 个任务
                            </div>
                        </div>

                        <div
                            v-if="trendChartType === 'bar'"
                            class="bar"
                            :class="{ hovered: trendHoverIndex === i }"
                            :style="{ height: `${trendBarHeightPct(d.count)}%` }"
                        ></div>
                        <div
                            v-else
                            class="line-marker"
                            :class="{ hovered: trendHoverIndex === i }"
                            :style="{ bottom: `${trendBarHeightPct(d.count)}%` }"
                        ></div>

                        <span class="bar-x-label">{{
                            trendXLabel(d, i)
                        }}</span>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.charts-page {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px 32px;
    display: flex;
    flex-direction: column;
    gap: 18px;
}

.charts-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
}
.charts-title {
    font-size: 1.2308rem;
    font-weight: 700;
    color: var(--fg);
}

.view-nav {
    display: flex;
    gap: 2px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 2px;
}
.view-nav-btn {
    padding: 5px 14px;
    border-radius: 5px;
    font-size: 0.8462rem;
    font-weight: 600;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.view-nav-btn:hover {
    color: var(--fg);
}
.view-nav-btn.active {
    color: var(--blue);
    background: var(--bg-panel);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
}

.charts-error {
    padding: 8px 16px;
    border-radius: 6px;
    background: rgba(209, 36, 47, 0.1);
    color: var(--red);
    font-size: 0.9231rem;
}
.charts-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-dim);
    font-size: 0.9231rem;
}

.chart-card {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 18px 20px 14px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    flex: 1;
    min-height: 280px;
    min-width: 0;
}
.chart-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px;
}
.chart-header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
}
.chart-title {
    font-size: 0.9231rem;
    font-weight: 700;
    color: var(--fg);
}
.chart-hint {
    font-size: 0.8462rem;
    font-weight: 500;
    color: var(--fg-dim);
}

/* 视图1/视图2 共用的分组切换按钮样式 */
.group-by-group,
.range-group {
    display: flex;
    gap: 6px;
}
.group-by-btn,
.range-btn {
    padding: 5px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.group-by-btn:hover,
.range-btn:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}
.group-by-btn.active,
.range-btn.active {
    color: var(--blue);
    border-color: var(--blue);
    background: rgba(26, 115, 232, 0.1);
}

.chart-type-group {
    display: flex;
    gap: 2px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px;
}
.chart-type-btn {
    width: 24px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    font-size: 0.7692rem;
    opacity: 0.5;
    transition: all 0.15s;
}
.chart-type-btn:hover {
    opacity: 0.8;
}
.chart-type-btn.active {
    opacity: 1;
    background: var(--bg-panel);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
}

/* 视图1：横向柱状图 */
.hbar-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
}
.hbar-row {
    display: flex;
    align-items: center;
    gap: 10px;
}
.hbar-row.clickable {
    cursor: pointer;
}
.hbar-label {
    width: 160px;
    flex-shrink: 0;
    font-size: 0.8462rem;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: right;
}
.hbar-row.clickable:hover .hbar-label {
    color: var(--blue);
}
.hbar-track {
    flex: 1;
    height: 18px;
    background: var(--bg);
    border-radius: 4px;
    overflow: hidden;
}
.hbar-fill {
    height: 100%;
    background: var(--blue);
    border-radius: 4px;
    transition:
        width 0.2s,
        background 0.15s;
}
.hbar-row.clickable:hover .hbar-fill {
    background: var(--cyan);
}
.hbar-value {
    flex-shrink: 0;
    width: 90px;
    font-size: 0.8462rem;
    font-variant-numeric: tabular-nums;
    color: var(--fg-dim);
}

/* 视图2：柱状/折线图（与 Dashboard 的专注时长图表同一套视觉规范） */
.chart-plot {
    position: relative;
    flex: 1;
    min-height: 200px;
    padding: 8px 4px 26px 4px;
}
.grid-lines {
    position: absolute;
    left: 4px;
    right: 4px;
    top: 8px;
    bottom: 26px;
}
.grid-line {
    position: absolute;
    left: 0;
    right: 0;
    border-top: 1px dashed var(--border);
}
.grid-label {
    position: absolute;
    left: 2px;
    top: -16px;
    font-size: 0.7692rem;
    color: var(--fg-dim);
    white-space: nowrap;
}

.line-svg {
    position: absolute;
    left: 4px;
    right: 4px;
    top: 8px;
    bottom: 26px;
    width: calc(100% - 8px);
    height: calc(100% - 34px);
    overflow: visible;
}
.area-fill {
    fill: var(--blue);
    opacity: 0.12;
    stroke: none;
}
.line-path {
    fill: none;
    stroke: var(--blue);
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
}

.bars-row {
    position: absolute;
    left: 4px;
    right: 4px;
    top: 8px;
    bottom: 26px;
    display: flex;
    align-items: flex-end;
    gap: 3px;
}
.bar-slot {
    position: relative;
    flex: 1;
    height: 100%;
    display: flex;
    align-items: flex-end;
    justify-content: center;
}
.bar {
    width: 55%;
    min-width: 3px;
    background: var(--blue);
    border-radius: 4px 4px 0 0;
    transition:
        opacity 0.15s,
        background 0.15s;
}
.bar.hovered {
    background: var(--cyan);
}
.line-marker {
    position: absolute;
    left: 50%;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--blue);
    border: 2px solid var(--bg-panel);
    transform: translate(-50%, 50%);
    transition: background 0.15s;
    z-index: 2;
}
.line-marker.hovered {
    background: var(--cyan);
}
.bar-x-label {
    position: absolute;
    bottom: -20px;
    left: 50%;
    transform: translateX(-50%);
    font-size: 0.7692rem;
    color: var(--fg-dim);
    white-space: nowrap;
}

.bar-tooltip {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 5;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 10px;
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.2);
    white-space: nowrap;
    pointer-events: none;
}
.tooltip-date {
    font-size: 0.8462rem;
    font-weight: 700;
    color: var(--fg);
    margin-bottom: 4px;
}
.tooltip-row {
    font-size: 0.7692rem;
    color: var(--fg-dim);
    line-height: 1.5;
}

.empty-hint {
    font-size: 0.8462rem;
    color: var(--fg-dark);
    padding: 4px 0;
}
</style>
