<!--
  @Component: 项目树单个节点
  @Description:
    渲染一个项目节点及其所有子节点（递归调用自身）。
    包含：展开/折叠图标、项目名、待办/逾期/锁定计数、进度条、
    拖拽调整父项目、右键菜单入口。
  @Author: Bin.H
  @Date: 2026-05-25
-->
<script setup>
import { computed, inject } from "vue";

const props = defineProps({
    path: { type: String, required: true },
    projects: { type: Object, required: true },
    selected: { type: String, default: null }, // 是否是当前选择的项目
    collapsed: { type: Object, required: true }, // 折叠隐藏的节点列表
});

const emit = defineEmits([
    "select",
    "toggle",
    "toggle-archive",
    "set-stage",
    "trash-project",
    "restore-project",
    "context-menu",
]);

// 拖拽状态与拖拽发起函数由 ProjectTree 统一提供，避免每个节点各自维护一份。
// 用鼠标事件手动实现拖拽（而非原生 HTML5 Drag & Drop API），因为 WebKitGTK
// （Tauri Linux 使用的内核）的原生拖放支持不可靠，dragstart/drop 经常收不到。
const dragState = inject("projectDragState");
const beginProjectDrag = inject("beginProjectDrag");

const node = computed(() => props.projects[props.path]);
const children = computed(() => node.value?.children || []);
const hasChildren = computed(() => children.value.length > 0);
const isCollapsed = computed(() => props.collapsed.has(props.path));
const isSelected = computed(() => props.selected == props.path);

// "无项目" 是虚拟归集节点，没有对应的真实项目记录，不显示归档/删除操作
const isRealProject = computed(() => props.path !== "无项目");

// 阶段（计划中/进行中）和归档只能在顶层项目上操作，子项目必须跟随顶层项目一起移动
const isRoot = computed(() => !props.path.includes("."));

// 项目节点显示名称，只显示末端
const displayName = computed(() => node.value?.name || props.path);

// 切换目标为当前阶段的对面（只在顶层项目上可操作，子项目跟随继承）
const otherStage = computed(() =>
    node.value?.stage === "planned" ? "active" : "planned",
);

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

// ----------------------------------------
// 拖拽移动项目
// ----------------------------------------
const canDrag = computed(() => isRealProject.value && !node.value?.trashed);

// 当前是否正被拖拽的节点本身
const isBeingDragged = computed(() => dragState.path === props.path);

// 拖入此节点是否合法：不能拖到自己、自己的子项目、或废纸篓中的节点上
const isValidDropTarget = computed(() => {
    const dragging = dragState.path;
    if (!dragging || !isRealProject.value || node.value?.trashed) return false;
    if (dragging === props.path) return false;
    if (props.path.startsWith(`${dragging}.`)) return false;
    return true;
});

// 鼠标当前是否悬停在此节点上方（拖拽过程中，用于高亮投放目标）
const isDropHover = computed(
    () => dragState.overPath === props.path && isValidDropTarget.value,
);

function onMouseDown(e) {
    if (!canDrag.value || e.button !== 0) return;
    // 点在展开图标/操作按钮上时不发起拖拽，避免影响它们本身的点击行为
    if (e.target.closest(".toggle-icon, .node-action-btn")) return;
    // 阻止浏览器默认的文字框选行为（鼠标按下后拖动本会触发选区）
    e.preventDefault();
    beginProjectDrag(props.path, e);
}
</script>

<template>
    <div v-if="node">
        <!-- 根节点行 -->
        <div
            class="tree-node"
            :class="{
                selected: isSelected,
                'has-overdue': node.overdue_count > 0,
                archived: node.archived,
                trashed: node.trashed,
                'drop-target': isDropHover,
                dragging: isBeingDragged,
            }"
            :style="indentStyle"
            :data-project-path="path"
            @click="emit('select', path)"
            @contextmenu.prevent="
                emit('context-menu', $event.clientX, $event.clientY, path)
            "
            @mousedown="onMouseDown"
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
                <span class="badge pending" title="待办">{{
                    node.pending_count
                }}</span>
                <span
                    v-if="node.overdue_count > 0"
                    class="badge overdue"
                    title="逾期"
                    >⚠{{ node.overdue_count }}</span
                >
                <span class="badge locked" title="锁定"
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

            <!-- 废纸篓中的项目：恢复/彻底删除；正常项目：阶段/归档/移入废纸篓 -->
            <div v-if="isRealProject && node.trashed" class="node-actions">
                <button
                    v-if="node.self_trashed"
                    class="node-action-btn"
                    title="恢复"
                    @click.stop="emit('restore-project', path)"
                >
                    ↺
                </button>
            </div>
            <div v-else-if="isRealProject" class="node-actions">
                <button
                    v-if="isRoot"
                    class="node-action-btn"
                    :title="`移到${otherStage === 'planned' ? '计划中' : '进行中'}`"
                    @click.stop="emit('set-stage', path, otherStage)"
                >
                    {{ node.stage === "planned" ? "◔" : "◕" }}
                </button>
                <button
                    v-if="isRoot && (node.self_archived || !node.archived)"
                    class="node-action-btn"
                    :title="node.self_archived ? '取消归档' : '归档'"
                    @click.stop="
                        emit(
                            'toggle-archive',
                            path,
                            !node.self_archived,
                        )
                    "
                >
                    {{ node.self_archived ? "↺" : "📦" }}
                </button>
                <button
                    class="node-action-btn danger"
                    title="移入废纸篓"
                    @click.stop="emit('trash-project', path)"
                >
                    🗑
                </button>
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
                @toggle-archive="
                    (p, archived) => emit('toggle-archive', p, archived)
                "
                @set-stage="(p, stage) => emit('set-stage', p, stage)"
                @trash-project="(p) => emit('trash-project', p)"
                @restore-project="(p) => emit('restore-project', p)"
                @context-menu="
                    (x, y, p) => emit('context-menu', x, y, p)
                "
            />
        </template>
    </div>
</template>

<style scoped>
/* 节点行容器 */
.tree-node {
    position: relative;
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
.tree-node.archived .node-name {
    color: var(--fg-dim);
}
.tree-node.trashed .node-name {
    color: var(--fg-dark);
    text-decoration: line-through;
}
.tree-node.drop-target {
    outline: 2px dashed var(--blue);
    outline-offset: -2px;
    background: rgba(122, 162, 247, 0.12);
}
.tree-node.dragging {
    opacity: 0.5;
}

/* 展开图标 */
.toggle-icon {
    font-size: 0.7692rem;
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
    font-size: 0.9231rem;
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
    font-size: 0.7692rem;
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
    width: 48px;
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

/* 操作按钮：悬浮覆盖在节点行右侧，不占用布局空间，悬停节点行时才浮现 */
.node-actions {
    position: absolute;
    right: 4px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px 3px;
    border-radius: 6px;
    background: var(--bg-panel);
    box-shadow: -10px 0 8px -2px var(--bg-panel);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.12s;
}
.tree-node:hover .node-actions {
    opacity: 1;
    pointer-events: auto;
}

.node-action-btn {
    width: 18px;
    height: 18px;
    line-height: 1;
    font-size: 0.7692rem;
    border-radius: 3px;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.node-action-btn:hover {
    color: var(--blue);
    background: rgba(122, 162, 247, 0.12);
}
.node-action-btn.danger:hover {
    color: var(--red);
    background: rgba(247, 118, 142, 0.15);
}
</style>
