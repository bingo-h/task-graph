<!--
  @Component: 首页仪表盘
  @Description:
    展示专注时长看板：数据汇总卡片 + 专注时长图表（柱状/折线可切换）+ 任务汇总。
    统计范围支持天/周/月切换；"天"范围下图表按小时分段展示。
  @Author: Bin.H
-->

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { listAllTimeEntries } from "../composables/useApi";
import { formatDuration } from "../composables/useDuration";
import { isoToLocalDate } from "../composables/useLocalTime";

const props = defineProps({
    // 全部任务节点，用于统计完成任务数、到期任务、优先级分布
    nodes: { type: Array, default: () => [] },
    // 项目树（path -> ProjectNode），用于排除已归档/废纸篓项目下的任务
    projects: { type: Object, default: () => ({}) },
    // 当前是否是正在展示的页面（父组件用 v-show 切换，不会重新挂载组件），
    // 只在挂载时拉取一次计时记录的话，长时间挂在后台会跨天而不自动刷新
    visible: { type: Boolean, default: true },
});

// ----------------------------------------
// 排除已归档 / 废纸篓项目下的任务：这些不应该计入首页的汇总统计
// ----------------------------------------
/** 任务没有项目（收件箱）时始终计入；否则看所属项目当前是否归档/在废纸篓中（含级联自祖先项目） */
function isInActiveProject(project) {
    if (!project) return true;
    const p = props.projects[project];
    return !p || (!p.archived && !p.trashed);
}

const activeNodes = computed(() =>
    props.nodes.filter((t) => isInActiveProject(t.project)),
);
const activeTaskUuids = computed(
    () => new Set(activeNodes.value.map((t) => t.uuid)),
);

// 界面语言，决定图表里星期/小时标签的格式（Intl 会按 locale 自动换算）。
// TODO: 后续加入语言设置后，改为从全局设置读取，而不是写死。
const locale = ref("zh-CN");

// 日期统一用"MM-DD"补零格式（如 08-15），不用 Intl 的 8/15 这种写法，
// 避免不同 locale/月份下位数不对齐、也更符合日期惯用记法
const monthDayFormatter = {
    format: (d) =>
        `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`,
};

const weekdayFormatter = computed(
    () => new Intl.DateTimeFormat(locale.value, { weekday: "short" }),
);
const hourFormatter = computed(
    () => new Intl.DateTimeFormat(locale.value, { hour: "numeric" }),
);
const timeFormatter = computed(
    () => new Intl.DateTimeFormat(locale.value, { hour: "numeric", minute: "2-digit" }),
);

const RANGE_DAYS = { day: 1, week: 7, month: 30 };
const RANGE_OPTIONS = [
    { key: "day", label: "今天" },
    { key: "week", label: "本周" },
    { key: "month", label: "本月" },
];
const rangeKey = ref("week");

const chartType = ref("bar"); // "bar" | "line"

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

// 切回这个页面时自动刷新一次；组件本身因为用 v-show 常驻不会重新挂载，
// 如果一直不切走（比如正好停留在这一页跨过午夜），也用定时器兜底定期刷新
watch(
    () => props.visible,
    (v) => {
        if (v) loadEntries();
    },
);

const AUTO_REFRESH_MS = 5 * 60 * 1000; // 5 分钟
let autoRefreshTimer = null;
onMounted(() => {
    autoRefreshTimer = setInterval(loadEntries, AUTO_REFRESH_MS);
});
onUnmounted(() => clearInterval(autoRefreshTimer));

// ----------------------------------------
// 分段聚合：默认按自然日分段；"今天"范围下按小时分段，
// 这样切到"今天"才有意义——单独一天只有一根柱子没有信息量。
// ----------------------------------------
const HOUR_MS = 3600 * 1000;
const DAY_MS = 86400 * 1000;

function startOfDay(d) {
    const r = new Date(d);
    r.setHours(0, 0, 0, 0);
    return r;
}

// 各分段的起始时间点
const bucketStarts = computed(() => {
    if (rangeKey.value === "day") {
        const base = startOfDay(new Date());
        return Array.from({ length: 24 }, (_, h) => new Date(base.getTime() + h * HOUR_MS));
    }
    const n = RANGE_DAYS[rangeKey.value];
    const today = startOfDay(new Date());
    return Array.from({ length: n }, (_, i) => new Date(today.getTime() - (n - 1 - i) * DAY_MS));
});
const bucketMs = computed(() => (rangeKey.value === "day" ? HOUR_MS : DAY_MS));

/** 把一段 [start, end) 区间按重叠时长分摊进各个分段，比"整段计入起点所在分段"更准确 */
function bucketOverlapSeconds(intervals, starts, ms) {
    const totals = new Array(starts.length).fill(0);
    for (const [s, e] of intervals) {
        if (e <= s) continue;
        for (let i = 0; i < starts.length; i++) {
            const bs = starts[i].getTime();
            const be = bs + ms;
            const overlap = Math.min(e, be) - Math.max(s, bs);
            if (overlap > 0) totals[i] += overlap / 1000;
        }
    }
    return totals;
}

/** 找出某个时间点落在哪个分段里，找不到返回 -1（用于按完成时间点归类，而非区间） */
function bucketIndexFor(time, starts, ms) {
    for (let i = 0; i < starts.length; i++) {
        const bs = starts[i].getTime();
        if (time >= bs && time < bs + ms) return i;
    }
    return -1;
}

// 已归档/废纸篓项目产生的计时记录也一并排除，和任务侧的口径保持一致
const activeEntries = computed(() =>
    entries.value.filter((e) => activeTaskUuids.value.has(e.task_uuid)),
);

const focusTotals = computed(() => {
    const now = Date.now();
    const intervals = activeEntries.value.map((e) => [
        new Date(e.start).getTime(),
        e.end ? new Date(e.end).getTime() : now,
    ]);
    return bucketOverlapSeconds(intervals, bucketStarts.value, bucketMs.value);
});

const completedTotals = computed(() => {
    const totals = new Array(bucketStarts.value.length).fill(0);
    for (const t of activeNodes.value) {
        if (t.status !== "completed" || !t.end) continue;
        const idx = bucketIndexFor(new Date(t.end).getTime(), bucketStarts.value, bucketMs.value);
        if (idx >= 0) totals[idx] += 1;
    }
    return totals;
});

const chartData = computed(() =>
    bucketStarts.value.map((d, i) => {
        const focusSeconds = focusTotals.value[i] || 0;
        const periodSeconds = bucketMs.value / 1000;
        return {
            key: d.toISOString(),
            date: d,
            focusSeconds,
            completedCount: completedTotals.value[i] || 0,
            occupancyPct: (focusSeconds / periodSeconds) * 100,
        };
    }),
);

// 数据表只列出有专注时长的分段，避免大段空行把有数据的行淹没
const chartTableRows = computed(() =>
    chartData.value.filter((d) => d.focusSeconds > 0),
);

// ----------------------------------------
// 数据汇总（始终按"整个统计范围"计算，不受图表分段粒度影响：
// chartData 各分段之和恰好等于整个范围的总量，天/周/月求平均时按 1/7/30 天计）
// ----------------------------------------
const DAY_SECONDS = 86400;

const summary = computed(() => {
    const list = chartData.value;
    const totalFocus = list.reduce((s, d) => s + d.focusSeconds, 0);
    const n = RANGE_DAYS[rangeKey.value];
    return {
        totalFocus,
        avgFocus: totalFocus / n,
        // 专注时间占比：随范围选择器变化，选定范围内专注时长占该范围总时长的比例
        occupancyPct: (totalFocus / (n * DAY_SECONDS)) * 100,
    };
});

// 完成任务：总览口径，展示"已完成 / 全部任务"，不受范围选择器影响（不含已被删除的任务）
const taskCompletionStats = computed(() => {
    const relevant = activeNodes.value.filter((t) => t.status !== "deleted");
    return {
        completed: relevant.filter((t) => t.status === "completed").length,
        total: relevant.length,
    };
});

// ----------------------------------------
// 图表：Y 轴取一个"好看"的上限刻度，避免全 0 时图表塌陷、
// 也避免单段峰值把其它段压成看不见的细线
// ----------------------------------------
const NICE_STEPS_HOUR = [5, 10, 15, 20, 30, 45, 60];
const NICE_STEPS_DAY = [30, 60, 120, 180, 240, 300, 360, 480, 600, 720, 1440];

const chartMaxMinutes = computed(() => {
    const steps = rangeKey.value === "day" ? NICE_STEPS_HOUR : NICE_STEPS_DAY;
    const maxSeconds = Math.max(0, ...chartData.value.map((d) => d.focusSeconds));
    const maxMinutes = maxSeconds / 60;
    return steps.find((s) => s >= maxMinutes) || steps[steps.length - 1];
});

const gridLines = computed(() =>
    [1, 0.5].map((r) => ({
        minutes: Math.round(chartMaxMinutes.value * r),
        pct: r * 100,
    })),
);

function barHeightPct(focusSeconds) {
    if (focusSeconds <= 0) return 0;
    const minutes = focusSeconds / 60;
    return Math.max(2, (minutes / chartMaxMinutes.value) * 100);
}

/** 横坐标标签：天范围显示小时（每 3 小时标一次），周范围显示星期，月范围每 5 天标一次 */
function pointXLabel(d, index) {
    if (rangeKey.value === "day") {
        return index % 3 === 0 ? hourFormatter.value.format(d.date) : "";
    }
    if (rangeKey.value === "week") {
        return weekdayFormatter.value.format(d.date);
    }
    return index % 5 === 0 ? monthDayFormatter.format(d.date) : "";
}

/** 悬浮提示里的时间标签：天范围显示具体的一小时区间，否则显示日期 */
function pointTooltipLabel(d) {
    if (rangeKey.value === "day") {
        const hourEnd = new Date(d.date.getTime() + HOUR_MS);
        return `${timeFormatter.value.format(d.date)} - ${timeFormatter.value.format(hourEnd)}`;
    }
    return monthDayFormatter.format(d.date);
}

/** 紧凑的时长展示，如 "1小时20分钟" / "20分钟"，用于图表标签和提示 */
function formatCompact(seconds) {
    const totalMin = Math.round(seconds / 60);
    const h = Math.floor(totalMin / 60);
    const m = totalMin % 60;
    if (h === 0) return `${m}分钟`;
    return m === 0 ? `${h}小时` : `${h}小时${m}分钟`;
}

const hoverIndex = ref(null);
const showTable = ref(false);

// ----------------------------------------
// 折线图连线几何（百分比坐标系，纯图形不含文字，
// 非等比缩放不会导致文字变形；线宽用 vector-effect 保持视觉粗细恒定）
// ----------------------------------------
const linePoints = computed(() =>
    chartData.value.map((d, i) => ({
        x: ((i + 0.5) / chartData.value.length) * 100,
        y: 100 - barHeightPct(d.focusSeconds),
    })),
);
const linePolyline = computed(() =>
    linePoints.value.map((p) => `${p.x},${p.y}`).join(" "),
);
const areaPath = computed(() => {
    const pts = linePoints.value;
    if (pts.length === 0) return "";
    const first = pts[0];
    const last = pts[pts.length - 1];
    const body = pts.map((p) => `L ${p.x},${p.y}`).join(" ");
    return `M ${first.x},100 ${body} L ${last.x},100 Z`;
});

// ----------------------------------------
// 任务汇总：优先级分布 + 即将到期
// ----------------------------------------
const priorityStats = computed(() => {
    const pending = activeNodes.value.filter((t) => t.status === "pending");
    return {
        high: pending.filter((t) => t.priority === "H").length,
        medium: pending.filter((t) => t.priority === "M").length,
        low: pending.filter((t) => t.priority === "L").length,
    };
});

const dueSoonTasks = computed(() =>
    activeNodes.value
        .filter((t) => t.status === "pending" && t.due)
        .sort((a, b) => new Date(a.due) - new Date(b.due))
        .slice(0, 6),
);

/** due 是目标日期（后端统一钉死 UTC 23:59:59），不能经过 Date 对象换算成本地时区——
 *  正偏移时区下换算会把 getDate() 进位到下一天，导致显示的日期比用户实际设置的晚一天 */
function formatDueLabel(due) {
    return isoToLocalDate(due).slice(5, 10);
}

// ----------------------------------------
// 今日任务：手动标记的、今天要专注处理的任务；过了这一天后端会自动清除标记
// ----------------------------------------
const emit = defineEmits(["jump-to-task"]);

const todayTasks = computed(() => activeNodes.value.filter((t) => t.is_today));
</script>

<template>
    <div class="dashboard">
        <div class="dashboard-toolbar">
            <h2 class="dashboard-title">专注概览</h2>
            <div class="range-group">
                <button
                    v-for="r in RANGE_OPTIONS"
                    :key="r.key"
                    class="range-btn"
                    :class="{ active: rangeKey === r.key }"
                    @click="rangeKey = r.key"
                >
                    {{ r.label }}
                </button>
            </div>
        </div>

        <div v-if="error" class="dashboard-error">⚠ {{ error }}</div>

        <!-- 数据汇总 -->
        <div class="summary-grid">
            <div class="summary-card">
                <span class="summary-label">总专注时长</span>
                <span class="summary-value">{{
                    formatDuration(summary.totalFocus)
                }}</span>
            </div>
            <div class="summary-card">
                <span class="summary-label">日均专注</span>
                <span class="summary-value">{{
                    formatCompact(summary.avgFocus)
                }}</span>
            </div>
            <div class="summary-card">
                <span class="summary-label">完成任务</span>
                <span class="summary-value"
                    >{{ taskCompletionStats.completed }}/{{
                        taskCompletionStats.total
                    }}</span
                >
            </div>
            <div class="summary-card">
                <span class="summary-label">专注时间占比</span>
                <span class="summary-value"
                    >{{ summary.occupancyPct.toFixed(1) }}<span
                        class="summary-unit"
                        >%</span
                    ></span
                >
            </div>
        </div>

        <!-- 今日任务：手动标记的、今天要专注处理的任务；点击跳转到任务看板并选中 -->
        <div class="today-tasks-card">
            <div class="chart-header">
                <span class="chart-title">☀ 今日任务</span>
                <span v-if="todayTasks.length > 0" class="chart-hint">
                    {{
                        todayTasks.filter((t) => t.status === "pending")
                            .length
                    }}
                    个待完成，共 {{ todayTasks.length }} 个
                </span>
            </div>

            <div v-if="todayTasks.length === 0" class="empty-hint">
                还没有任务被标记为今日任务，去任务详情里点标题旁的 ☀ 图标添加
            </div>
            <div v-else class="today-tasks-list">
                <button
                    v-for="t in todayTasks"
                    :key="t.uuid"
                    class="today-task-item"
                    :class="{ done: t.status === 'completed' }"
                    @click="emit('jump-to-task', t.uuid)"
                >
                    <span class="today-task-status">{{
                        t.status === "completed" ? "✔" : "○"
                    }}</span>
                    <span class="today-task-desc">{{ t.description }}</span>
                    <span
                        v-if="t.priority"
                        class="today-task-priority"
                        :class="`priority-${t.priority.toLowerCase()}`"
                    >
                        {{ t.priority }}
                    </span>
                </button>
            </div>
        </div>

        <div class="dashboard-lower">
            <!-- 专注时长图表：柱状/折线可切换 -->
            <div class="chart-card">
                <div class="chart-header">
                    <span class="chart-title">{{
                        rangeKey === "day" ? "今日各时段专注时长" : "每日专注时长"
                    }}</span>
                    <div class="chart-header-actions">
                        <div class="chart-type-group">
                            <button
                                class="chart-type-btn"
                                :class="{ active: chartType === 'bar' }"
                                title="柱状图"
                                @click="chartType = 'bar'"
                            >
                                📊
                            </button>
                            <button
                                class="chart-type-btn"
                                :class="{ active: chartType === 'line' }"
                                title="折线图"
                                @click="chartType = 'line'"
                            >
                                📈
                            </button>
                        </div>
                        <button
                            class="table-toggle"
                            @click="showTable = !showTable"
                        >
                            {{ showTable ? "查看图表" : "查看数据表" }}
                        </button>
                    </div>
                </div>

                <div v-if="loading" class="chart-empty">加载中…</div>

                <template v-else>
                    <div v-if="!showTable" class="chart-plot">
                        <div class="grid-lines">
                            <div
                                v-for="g in gridLines"
                                :key="g.minutes"
                                class="grid-line"
                                :style="{ bottom: `${g.pct}%` }"
                            >
                                <span class="grid-label">{{
                                    formatCompact(g.minutes * 60)
                                }}</span>
                            </div>
                        </div>

                        <!-- 折线图连线：纯几何图形，不含文字，避免非等比缩放导致文字变形 -->
                        <svg
                            v-if="chartType === 'line'"
                            class="line-svg"
                            viewBox="0 0 100 100"
                            preserveAspectRatio="none"
                        >
                            <path class="area-fill" :d="areaPath" />
                            <polyline
                                class="line-path"
                                :points="linePolyline"
                                vector-effect="non-scaling-stroke"
                            />
                        </svg>

                        <div class="bars-row">
                            <div
                                v-for="(d, i) in chartData"
                                :key="d.key"
                                class="bar-slot"
                                @mouseenter="hoverIndex = i"
                                @mouseleave="hoverIndex = null"
                            >
                                <div v-if="hoverIndex === i" class="bar-tooltip">
                                    <div class="tooltip-date">
                                        {{ pointTooltipLabel(d) }}
                                    </div>
                                    <div class="tooltip-row">
                                        专注 {{ formatCompact(d.focusSeconds) }}
                                    </div>
                                    <div class="tooltip-row">
                                        占比 {{ d.occupancyPct.toFixed(1) }}%
                                    </div>
                                    <div class="tooltip-row">
                                        完成 {{ d.completedCount }} 个任务
                                    </div>
                                </div>

                                <div
                                    v-if="chartType === 'bar'"
                                    class="bar"
                                    :class="{ hovered: hoverIndex === i }"
                                    :style="{
                                        height: `${barHeightPct(d.focusSeconds)}%`,
                                    }"
                                ></div>
                                <div
                                    v-else
                                    class="line-marker"
                                    :class="{ hovered: hoverIndex === i }"
                                    :style="{
                                        bottom: `${barHeightPct(d.focusSeconds)}%`,
                                    }"
                                ></div>

                                <span class="bar-x-label">{{
                                    pointXLabel(d, i)
                                }}</span>
                            </div>
                        </div>
                    </div>

                    <!-- 数据表：作为图表的可访问性替代视图 -->
                    <div v-else class="data-table-wrap">
                        <div
                            v-if="chartTableRows.length === 0"
                            class="empty-hint"
                        >
                            这段时间内暂无专注记录
                        </div>
                        <table v-else class="data-table">
                            <thead>
                                <tr>
                                    <th>{{ rangeKey === "day" ? "时段" : "日期" }}</th>
                                    <th>专注时长</th>
                                    <th>时间占比</th>
                                    <th>完成任务</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr v-for="d in chartTableRows" :key="d.key">
                                    <td>{{ pointTooltipLabel(d) }}</td>
                                    <td>{{ formatCompact(d.focusSeconds) }}</td>
                                    <td>{{ d.occupancyPct.toFixed(1) }}%</td>
                                    <td>{{ d.completedCount }}</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </template>
            </div>

            <!-- 任务汇总 -->
            <div class="task-summary-card">
                <div class="chart-header">
                    <span class="chart-title">任务汇总</span>
                </div>

                <div class="priority-stats">
                    <div class="priority-stat priority-h">
                        <span class="priority-stat-label">高优先级待办</span>
                        <span class="priority-stat-value">{{
                            priorityStats.high
                        }}</span>
                    </div>
                    <div class="priority-stat priority-m">
                        <span class="priority-stat-label">中优先级待办</span>
                        <span class="priority-stat-value">{{
                            priorityStats.medium
                        }}</span>
                    </div>
                    <div class="priority-stat priority-l">
                        <span class="priority-stat-label">低优先级待办</span>
                        <span class="priority-stat-value">{{
                            priorityStats.low
                        }}</span>
                    </div>
                </div>

                <div class="due-soon">
                    <span class="due-soon-title">即将到期</span>
                    <div
                        v-if="dueSoonTasks.length === 0"
                        class="empty-hint"
                    >
                        暂无设置截止日期的待办任务
                    </div>
                    <div v-else class="due-soon-list">
                        <div
                            v-for="t in dueSoonTasks"
                            :key="t.uuid"
                            class="due-soon-item"
                        >
                            <span class="due-soon-desc">{{
                                t.description
                            }}</span>
                            <span
                                class="due-soon-date"
                                :class="{ overdue: t.is_overdue }"
                            >
                                {{ t.is_overdue ? "已逾期 · " : "" }}{{
                                    formatDueLabel(t.due)
                                }}
                            </span>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.dashboard {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px 32px;
    display: flex;
    flex-direction: column;
    gap: 18px;
}

.dashboard-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
}
.dashboard-title {
    font-size: 1.2308rem;
    font-weight: 700;
    color: var(--fg);
}

.range-group {
    display: flex;
    gap: 6px;
}
.range-btn {
    padding: 5px 14px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 0.9231rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.range-btn:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}
.range-btn.active {
    color: var(--blue);
    border-color: var(--blue);
    background: rgba(26, 115, 232, 0.1);
}

.dashboard-error {
    padding: 8px 16px;
    border-radius: 6px;
    background: rgba(209, 36, 47, 0.1);
    color: var(--red);
    font-size: 0.9231rem;
}

/* 数据汇总卡片 */
.summary-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 14px;
}
.summary-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px 18px;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
}
.summary-label {
    font-size: 0.8462rem;
    color: var(--fg-dim);
}
.summary-value {
    font-size: 1.5385rem;
    font-weight: 700;
    color: var(--fg);
    font-variant-numeric: tabular-nums;
}
.summary-unit {
    font-size: 0.9231rem;
    font-weight: 500;
    color: var(--fg-dim);
}

/* 今日任务卡片 */
.today-tasks-card {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex-shrink: 0;
}
.chart-hint {
    font-size: 0.8462rem;
    color: var(--fg-dim);
}

.today-tasks-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}
.today-task-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 7px;
    background: var(--bg);
    border: 1px solid var(--border);
    transition: all 0.15s;
}
.today-task-item:hover {
    border-color: var(--cyan);
    background: rgba(125, 207, 255, 0.08);
}
.today-task-item.done {
    opacity: 0.55;
}
.today-task-status {
    flex-shrink: 0;
    font-size: 0.8462rem;
    color: var(--green);
}
.today-task-item:not(.done) .today-task-status {
    color: var(--fg-dark);
}
.today-task-desc {
    font-size: 0.8462rem;
    color: var(--fg);
    white-space: normal;
    word-break: break-word;
}
.today-task-item.done .today-task-desc {
    text-decoration: line-through;
}
.today-task-priority {
    flex-shrink: 0;
    font-size: 0.6923rem;
    font-weight: 700;
    padding: 0 4px;
    border-radius: 3px;
}
.today-task-priority.priority-h {
    color: var(--red);
    background: rgba(247, 118, 142, 0.15);
}
.today-task-priority.priority-m {
    color: var(--yellow);
    background: rgba(224, 175, 104, 0.15);
}
.today-task-priority.priority-l {
    color: var(--blue);
    background: rgba(122, 162, 247, 0.15);
}

/* 图表卡片 + 任务汇总卡片并排布局 */
.dashboard-lower {
    display: flex;
    gap: 18px;
    flex: 1;
    min-height: 0;
}

.chart-card {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 18px 20px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex: 2;
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
.table-toggle {
    font-size: 0.8462rem;
    color: var(--fg-dim);
    padding: 3px 8px;
    border-radius: 5px;
    transition: all 0.15s;
}
.table-toggle:hover {
    color: var(--blue);
    background: rgba(26, 115, 232, 0.1);
}

.chart-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-dim);
    font-size: 0.9231rem;
}

/* 柱状图 */
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

/* 折线图连线（与 .bars-row 完全同框，坐标系一致） */
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

/* 数据表 */
.data-table-wrap {
    flex: 1;
    overflow: auto;
}
.data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8462rem;
}
.data-table th {
    text-align: left;
    padding: 6px 10px;
    color: var(--fg-dim);
    font-weight: 600;
    border-bottom: 1px solid var(--border);
}
.data-table td {
    padding: 6px 10px;
    color: var(--fg);
    border-bottom: 1px solid var(--border);
}

/* 任务汇总卡片 */
.task-summary-card {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    flex: 1;
    min-width: 220px;
    overflow-y: auto;
}

.priority-stats {
    display: flex;
    flex-direction: column;
    gap: 8px;
}
.priority-stat {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    border-radius: 7px;
    background: var(--bg);
}
.priority-stat-label {
    font-size: 0.8462rem;
    color: var(--fg-dim);
}
.priority-stat-value {
    font-size: 1.0769rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
}
.priority-stat.priority-h .priority-stat-value {
    color: var(--red);
}
.priority-stat.priority-m .priority-stat-value {
    color: var(--yellow);
}
.priority-stat.priority-l .priority-stat-value {
    color: var(--blue);
}

.due-soon {
    display: flex;
    flex-direction: column;
    gap: 8px;
}
.due-soon-title {
    font-size: 0.8462rem;
    font-weight: 700;
    color: var(--fg);
}
.due-soon-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.due-soon-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 8px;
    border-radius: 6px;
    transition: background 0.12s;
}
.due-soon-item:hover {
    background: var(--bg);
}
.due-soon-desc {
    flex: 1;
    min-width: 0;
    font-size: 0.8462rem;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.due-soon-date {
    flex-shrink: 0;
    font-size: 0.7692rem;
    color: var(--fg-dim);
    font-variant-numeric: tabular-nums;
}
.due-soon-date.overdue {
    color: var(--red);
    font-weight: 600;
}

.empty-hint {
    font-size: 0.8462rem;
    color: var(--fg-dark);
    padding: 4px 0;
}
</style>
