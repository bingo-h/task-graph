<!--
  @Component: 项目树单个节点
  @Description:
    渲染一个项目节点及其所有子节点（递归调用自身）。
    包含：展开/折叠图标、项目名、待办/逾期/锁定计数、进度条。
  @Author: Bin.H
  @Date: 2026-05-25
-->
<script setup>
import { computed } from "vue";

const props = defineProps({
    path: { type: String, required: true },
    projects: { type: Object, required: true },
    selected: { type: String, default: null }, // 是否是当前选择的项目
    collapsed: { type: Object, required: true }, // 折叠隐藏的节点列表
});

const emit = defineEmits(["select", "toggle"]);

const node = computed(() => props.projects[props.path]);
const children = computed(() => node.value?.children || []);
const hasChildren = computed(() => children.value.length > 0);
const isCollapsed = computed(() => props.collapsed.has(props.path));
const isSelected = computed(() => props.selected == props.path);

// 项目节点显示名称，只显示末端
const displayName = computed(() => node.value?.name || props.path);

// 完成进度比例
const ratio = computed(() => {
    const n = node.value;
    if (!n) return 0;

    const total = n.pending_count + n.completed_count + n.waiting_count;

    return total === 0 ? 0 : n.completed_count / total;
});

// 根据项目路径动态计算间距
const indentStyle = computed(() => ({
    paddingLeft: `${(node.value?.depth || 0) * 16 + 12}px`,
}));
</script>

<template>
    <div v-if="node">
        <!-- 根节点行 -->
        <div
            class="tree-node"
            :class="{
                selected: isSelected,
                'has-overdue': node.overdue_count > 0,
            }"
            :style="indentStyle"
            @click="emit('select', path)"
        >
            <!-- 展开/折叠图标 -->
            <span
                v-if="hasChildren"
                class="toggle-icon"
                @click.stop="emit('toggle', path)"
                >{{ isCollapsed ? "▶" : "▼" }}</span
            >
            <span v-else class="toggle-icon placeholder">·</span>

            <!-- 项目名 -->
            <span class="node-name">{{ displayName }}</span>

            <!-- 计数标签 -->
            <span class="badges">
                <span v-if="node.pending_count > 0" class="badge pending">{{
                    node.pending_count
                }}</span>
                <span
                    v-if="node.overdue_count > 0"
                    class="badge overdue"
                    title="逾期"
                    >⚠{{ node.overdue_count }}</span
                >
                <span
                    v-if="node.locked_count > 0"
                    class="badge locked"
                    title="锁定"
                    >🔒{{ node.locked_count }}</span
                >
            </span>

            <!-- 迷你进度条 -->
            <div
                class="mini-progress"
                :title="`${Math.round(ratio * 100)}% 完成`"
            >
                <div
                    class="mini-progress-fill"
                    :style="{ width: `${ratio * 100}%` }"
                />
            </div>
        </div>

        <!-- 递归子节点 -->
        <template v-if="!isCollapsed && hasChildren">
            <ProjectTreeNode
                v-for="child in children"
                :key="child"
                :path="child"
                :projects="projects"
                :selected="selected"
                :collapsed="collapsed"
                @select="emit('select', $event)"
                @toggle="emit('toggle', $event)"
            />
        </template>
    </div>
</template>

<style scoped>
/* 节点行容器 */
.tree-node {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 12px 5px 0;
    cursor: pointer;
    border-radius: 4px;
    margin: 1px 4px;
    transition: background 0.12s;
    min-height: 28px;
}
.tree-node:hover {
    background: rgba(122, 162, 247, 0.05);
}
.tree-node.selected {
    background: var(--bg-select);
}
.tree-node.has-overdue .node-name {
    color: var(--red);
}
.tree-node.selected .node-name {
    color: var(--fg);
    font-weight: 600;
}

/* 展开图标 */
.toggle-icon {
    font-size: 10px;
    color: var(--fg-dim);
    width: 14px;
    text-align: center;
    flex-shrink: 0;
}
.toggle-icon.placeholder {
    color: var(--fg-dark);
}

/* 项目名称 */
.node-name {
    flex: 1;
    color: var(--blue);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

/* 计数徽章组 */
.badges {
    display: flex;
    gap: 3px;
    align-items: center;
}

.badge {
    font-size: 10px;
    padding: 0 4px;
    border-radius: 3px;
    font-weight: 600;
}
.badge.pending {
    color: var(--blue);
    background: rgba(122, 162, 247, 0.15);
}
.badge.overdue {
    color: var(--red);
    background: rgba(247, 118, 142, 0.15);
}
.badge.locked {
    color: var(--yellow);
    background: rgba(224, 175, 104, 0.15);
}

/* 迷你进度条 */
.mini-progress {
    width: 28px;
    height: 3px;
    background: var(--fg-dark);
    border-radius: 2px;
    flex-shrink: 0;
}

.mini-progress-fill {
    height: 100%;
    background: var(--green);
    border-radius: 2px;
    transition: width 0.3s;
}
</style>
