<!--
  @Component: 任务看板列表视图
  @Description:
    任务看板中间面板的另一种展现形式，替代 DAG 图谱：把当前筛选出的任务按
    紧迫度（后端已排好序）平铺成一行行列表，适合只是想快速扫一遍/勾选完成，
    不需要看依赖关系图的场景。跟 TaskGraph 共用同一套项目/标签筛选逻辑
    （useLayout.js 的 filterNodes），点击一行选中效果和图谱点节点一致。
    不支持框选/拖拽依赖这些图谱特有的操作，这些还是要切回导图模式。
  @Author: Bin.H
-->

<script setup>
import { computed } from "vue";
import { filterNodes } from "../composables/useLayout";
import { formatRecurSummary } from "../composables/useRecur";
import { tagChipStyle } from "../composables/useTagColor";
import { isoToLocalDate } from "../composables/useLocalTime";
import constants from "../config/constants";

const props = defineProps({
    nodes: { type: Array, default: () => [] },
    selected: { type: String, default: null },
    projectFilter: { type: String, default: null },
    tagFilter: { type: String, default: null },
    projects: { type: Object, default: () => ({}) },
    tags: { type: Object, default: () => ({}) }, // 标签名 -> { name, color, task_count }
});

const emit = defineEmits(["select", "done", "undone", "clear-tag-filter"]);

// 和图谱共用同一套项目/标签筛选逻辑，两种视图看到的任务范围完全一致；
// 后端返回时已经按紧迫度降序排好，这里不用再重新排序
const filteredNodes = computed(() =>
    filterNodes(props.nodes, props.projectFilter, props.projects).filter(
        (n) => !props.tagFilter || n.tags?.includes(props.tagFilter),
    ),
);

const tagFilterColor = computed(
    () => props.tags[props.tagFilter]?.color || "#8250df",
);

function statusLabel(status) {
    return (
        {
            pending: constants.PENDING,
            completed: constants.COMPLETED,
            waiting: constants.WAITING,
            deleted: constants.DELETED,
        }[status] || status
    );
}

function onRowClick(task) {
    emit("select", task.uuid === props.selected ? null : task.uuid);
}

function projectLabel(task) {
    if (!task.project) return props.projects[constants.INBOX_PROJECT]?.name || constants.INBOX_PROJECT;
    return task.project.split(".").pop();
}
</script>

<template>
    <div class="list-view">
        <!-- 标签筛选提示条：和图谱模式下的样式/交互保持一致 -->
        <div
            v-if="tagFilter"
            class="tag-filter-badge"
            :style="{ borderColor: tagFilterColor, color: tagFilterColor }"
        >
            <span>🏷 {{ tagFilter }}</span>
            <button
                class="tag-filter-clear"
                title="清除标签筛选"
                @click="emit('clear-tag-filter')"
            >
                ✕
            </button>
        </div>

        <div class="list-scroll">
            <div v-if="filteredNodes.length === 0" class="empty-hint">
                这里没有任务
            </div>

            <div
                v-for="task in filteredNodes"
                :key="task.uuid"
                class="task-row"
                :class="{
                    selected: task.uuid === selected,
                    done: task.status === 'completed',
                    overdue: task.is_overdue,
                    'due-today': task.is_due_today && !task.is_overdue,
                }"
                @click="onRowClick(task)"
            >
                <!-- 完成/取消完成：待办任务存在未完成前置任务时禁用，跟任务详情面板的按钮逻辑一致 -->
                <button
                    v-if="task.status === 'pending'"
                    type="button"
                    class="row-status-btn"
                    :class="{ locked: task.is_locked }"
                    :disabled="task.is_locked"
                    :title="task.is_locked ? '存在未完成的前置任务，无法完成' : '标记完成'"
                    @click.stop="emit('done', task.uuid)"
                >
                    {{ task.is_locked ? "🔒" : "○" }}
                </button>
                <button
                    v-else-if="task.status === 'completed'"
                    type="button"
                    class="row-status-btn done-mark"
                    title="取消完成"
                    @click.stop="emit('undone', task.uuid)"
                >
                    ✔
                </button>
                <span v-else class="row-status-btn placeholder" :title="statusLabel(task.status)">
                    ·
                </span>

                <span
                    v-if="task.priority"
                    class="row-priority"
                    :class="`priority-${task.priority.toLowerCase()}`"
                >
                    {{ task.priority }}
                </span>
                <span v-else class="row-priority placeholder" />

                <span class="row-desc" :title="task.description">
                    {{ task.icon ? `${task.icon} ` : "" }}{{ task.description }}
                </span>

                <span class="row-project" :title="projectLabel(task)">
                    {{ projectLabel(task) }}
                </span>

                <span v-if="task.tags?.length" class="row-tags">
                    <span
                        v-for="tag in task.tags"
                        :key="tag"
                        class="row-tag-chip"
                        :style="tagChipStyle(tags[tag]?.color)"
                    >
                        {{ tag }}
                    </span>
                </span>

                <span v-if="task.is_recurring" class="row-recur" :title="formatRecurSummary(task.recur_rule)">
                    🔁
                </span>

                <span v-if="task.due" class="row-due">
                    {{ isoToLocalDate(task.due) }}
                </span>
            </div>
        </div>
    </div>
</template>

<style scoped>
.list-view {
    flex: 1;
    position: relative;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg);
}

.list-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
}

.empty-hint {
    padding: 40px 0;
    text-align: center;
    color: var(--fg-dim);
    font-size: 0.9231rem;
}

.task-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    margin-bottom: 3px;
    border-radius: 6px;
    border-left: 3px solid transparent;
    background: var(--bg-panel);
    cursor: pointer;
    transition:
        background 0.12s,
        border-color 0.12s;
}
.task-row:hover {
    background: var(--bg-select);
}
.task-row.selected {
    background: var(--bg-select);
    outline: 1px solid var(--blue);
    outline-offset: -1px;
}
.task-row.overdue {
    border-left-color: var(--red);
}
.task-row.due-today {
    border-left-color: var(--yellow);
}
.task-row.done {
    opacity: 0.6;
}
.task-row.done .row-desc {
    text-decoration: line-through;
}

.row-status-btn {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    border: 1px solid var(--border);
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.row-status-btn:hover:not(:disabled) {
    border-color: var(--blue);
    color: var(--blue);
}
.row-status-btn.locked {
    cursor: not-allowed;
    opacity: 0.6;
}
.row-status-btn.done-mark {
    border-color: var(--green);
    color: var(--green);
    background: rgba(26, 127, 55, 0.12);
}
.row-status-btn.placeholder {
    border: none;
    color: var(--fg-dark);
}

.row-priority {
    flex-shrink: 0;
    width: 20px;
    text-align: center;
    font-size: 0.8462rem;
    font-weight: 700;
    border-radius: 4px;
}
.row-priority.priority-h {
    color: var(--red);
    background: rgba(247, 118, 142, 0.15);
}
.row-priority.priority-m {
    color: var(--yellow);
    background: rgba(224, 175, 104, 0.15);
}
.row-priority.priority-l {
    color: var(--blue);
    background: rgba(122, 162, 247, 0.15);
}
.row-priority.placeholder {
    background: none;
}

.row-desc {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 700;
    color: var(--fg);
}

.row-project {
    flex-shrink: 0;
    max-width: 140px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 0.8462rem;
    color: var(--fg-dim);
}

.row-tags {
    flex-shrink: 0;
    display: flex;
    gap: 4px;
    max-width: 220px;
    overflow: hidden;
}
.row-tag-chip {
    padding: 1px 7px;
    border-radius: 999px;
    font-size: 0.7692rem;
    white-space: nowrap;
}

.row-recur {
    flex-shrink: 0;
    font-size: 0.8462rem;
}

.row-due {
    flex-shrink: 0;
    width: 78px;
    text-align: right;
    font-size: 0.8462rem;
    color: var(--fg-dim);
    font-variant-numeric: tabular-nums;
}
.task-row.overdue .row-due {
    color: var(--red);
    font-weight: 700;
}
.task-row.due-today .row-due {
    color: var(--yellow);
    font-weight: 700;
}

/* 标签筛选提示条：和 TaskGraph 视觉保持一致 */
.tag-filter-badge {
    position: absolute;
    top: 16px;
    left: 16px;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 5px 12px;
    border-radius: 999px;
    background: var(--bg-panel);
    border: 1px solid;
    font-size: 0.8462rem;
    font-weight: 600;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12);
}
.tag-filter-clear {
    width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    font-size: 0.6923rem;
    color: inherit;
    opacity: 0.7;
}
.tag-filter-clear:hover {
    opacity: 1;
    background: rgba(0, 0, 0, 0.08);
}
</style>
