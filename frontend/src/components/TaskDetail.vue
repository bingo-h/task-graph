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
import { ref, computed, watch } from "vue";
import constants from "../config/constants";

const props = defineProps({
    task: { type: Object, default: null }, // 选中的任务对象，null表示未选中
    allTasks: { type: Array, required: true }, // 所有任务列表，用于查找依赖任务描述
});

const emit = defineEmits(["done", "delete", "modify", "select"]);

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
    <aside class="task-detail">
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

                    <span v-if="task.is_blocked" class="badge-extra locked">
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
            <div v-if="task.is_blocked" class="detail-section locked-hint">
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

                    <span v-if="blockingTask.is_blocked" class="dep-locked">
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
                    @click="emit('done', task.uuid)"
                >
                    ✔ 完成
                </button>

                <!-- 修改：emit 完整任务对象，由父组件（App.vue）打开 TaskFormModal -->
                <button class="btn-modify" @click="emit('modify', task)">
                    ✏ 修改
                </button>

                <button class="btn-delete" @click="emit('delete', task.uuid)">
                    ✗ 删除
                </button>
            </div>

            <!-- UUID 尾部显示（前 8 位） -->
            <div class="detail-uuid">{{ task.uuid.slice(0, 8) }}…</div>
        </template>
    </aside>
</template>

<style scoped>
/* 面板容器 */
.task-detail {
    width: 280px;
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

/* 未选中任何任务提示 */
.detail-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-dim);
    font-size: 13px;
}

/* 标题区 */
.detail-header {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.detail-title {
    font-size: 15px;
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
    font-size: 11px;
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
    font-size: 11px;
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
    font-size: 11px;
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
    font-size: 11px;
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
    font-size: 11px;
    color: var(--fg-dim);
    width: 44px;
    flex-shrink: 0;
}

.detail-val {
    font-size: 12px;
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
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 3px;
    background: rgba(187, 154, 247, 0.15);
    color: var(--magenta);
}

/* 锁定提示区 */
.locked-hint {
    font-size: 12px;
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
    font-size: 12px;
    transition: background 0.12s;
}
.dep-item:hover {
    background: rgba(255, 255, 255, 0.05);
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
    font-size: 11px;
}

/* 备注 */
.annotation {
    display: flex;
    gap: 8px;
    font-size: 11px;
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
    font-size: 12px;
    font-weight: 600;
    border: 1px solid rgba(158, 206, 106, 0.3);
    transition: background 0.15s;
}
.btn-done:hover {
    background: rgba(158, 206, 106, 0.35);
}

.btn-modify {
    flex: 1;
    padding: 6px;
    border-radius: 6px;
    background: rgba(122, 162, 247, 0.15);
    color: var(--blue);
    font-size: 12px;
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
    font-size: 12px;
    border: 1px solid rgba(247, 118, 142, 0.2);
    transition: background 0.15s;
}
.btn-delete:hover {
    background: rgba(247, 118, 142, 0.25);
}

/* UUID 尾部 */
.detail-uuid {
    font-size: 10px;
    color: var(--fg-dark);
    margin-top: auto;
    padding-top: 8px;
}
</style>
