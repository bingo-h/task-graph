<!--
  @Component: 项目树面板容器
  @Description:
    管理折叠状态，把项目按"计划中 / 进行中 / 已归档 / 废纸篓"四个分组渲染，
    每组标题栏可独立折叠，所有项目必须落在其中一个分组内。
    同时负责：面板宽度拖拽调整、拖拽移动项目、右键菜单（新建子项目/移动到/移入废纸篓）。
  @Author: Bin.H
  @Date: 2026-05-26
-->

<script setup>
import { computed, nextTick, provide, reactive, ref } from "vue";
import ProjectTreeNode from "./ProjectTreeNode.vue";
import ProjectContextMenu from "./ProjectContextMenu.vue";

const props = defineProps({
    projects: { type: Object, required: true },
    plannedRoots: { type: Array, default: () => [] },
    activeRoots: { type: Array, default: () => [] },
    archivedRoots: { type: Array, default: () => [] },
    trashRoots: { type: Array, default: () => [] },
    selected: { type: String, default: null },
});

const emit = defineEmits([
    "select",
    "create-project",
    "toggle-archive",
    "set-stage",
    "trash-project",
    "restore-project",
    "purge-project",
    "move-project",
]);

const collapsed = ref(new Set());

function onToggle(path) {
    if (collapsed.value.has(path)) collapsed.value.delete(path);
    else collapsed.value.add(path);

    // ref 只关注地址变化，因此要触发页面更新需要改变地址
    collapsed.value = new Set(collapsed.value);
}

// 四个分组的折叠状态，已归档/废纸篓默认折叠
const collapsedGroups = reactive({
    planned: false,
    active: false,
    archived: true,
    trash: true,
});

function toggleGroup(key) {
    collapsedGroups[key] = !collapsedGroups[key];
}

// 每个分组的展示信息，所有项目必须落在其中一个分组内
const groups = computed(() => [
    { key: "planned", label: "计划中", roots: props.plannedRoots },
    { key: "active", label: "进行中", roots: props.activeRoots },
    { key: "archived", label: "已归档", roots: props.archivedRoots },
    { key: "trash", label: "废纸篓", roots: props.trashRoots },
]);

// ----------------------------------------
// 面板宽度：可拖拽调整
// ----------------------------------------
const WIDTH_STORAGE_KEY = "task-web:project-tree-width";
const MIN_WIDTH = 160;
const MAX_WIDTH = 480;

const panelWidth = ref(
    Number(localStorage.getItem(WIDTH_STORAGE_KEY)) || 220,
);
const resizing = ref(false);

function startResize(e) {
    resizing.value = true;
    const startX = e.clientX;
    const startWidth = panelWidth.value;

    function onMove(moveEvent) {
        const next = startWidth + (moveEvent.clientX - startX);
        panelWidth.value = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, next));
    }

    function onUp() {
        resizing.value = false;
        localStorage.setItem(WIDTH_STORAGE_KEY, String(panelWidth.value));
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
    }

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
}

// ----------------------------------------
// 拖拽移动项目：共享的拖拽状态，提供给所有 ProjectTreeNode
//
// 用鼠标事件（mousedown/mousemove/mouseup）手动实现，而非原生 HTML5
// Drag & Drop API：WebKitGTK（Tauri Linux 下使用的内核）的原生拖放支持
// 不可靠，dragstart/dragover/drop 经常收不到，手动实现能在所有平台稳定工作。
// ----------------------------------------
const dragState = reactive({ path: null, overPath: null, overRoot: false });
provide("projectDragState", dragState);

// 跟随光标的拖拽提示牌：显示当前拖动的项目名，随鼠标位置实时移动
const dragGhost = reactive({ visible: false, x: 0, y: 0, label: "" });

/** 拖拽目标是否合法：不能拖到自己、自己的子项目、废纸篓中的项目 */
function isValidMoveTarget(dragging, target) {
    if (!dragging || !target) return false;
    if (dragging === target) return false;
    if (target.startsWith(`${dragging}.`)) return false;
    if (props.projects[target]?.trashed) return false;
    return true;
}

/** 由 ProjectTreeNode 在 mousedown 时调用，发起一次拖拽 */
function beginProjectDrag(path, e) {
    if (dragState.path) return;
    dragState.path = path;
    dragState.overPath = null;
    dragState.overRoot = false;

    dragGhost.visible = true;
    dragGhost.x = e.clientX;
    dragGhost.y = e.clientY;
    dragGhost.label = props.projects[path]?.name || path.split(".").pop();

    // 拖拽期间禁止全局文字被意外框选（原生 mousedown+移动默认会触发文本选区）
    const prevUserSelect = document.body.style.userSelect;
    document.body.style.userSelect = "none";

    function onMove(e) {
        dragGhost.x = e.clientX;
        dragGhost.y = e.clientY;

        const el = document.elementFromPoint(e.clientX, e.clientY);
        dragState.overRoot = !!el?.closest(".drop-to-root");
        dragState.overPath = el?.closest("[data-project-path]")?.dataset
            .projectPath;
    }

    function onUp() {
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
        document.body.style.userSelect = prevUserSelect;

        if (dragState.overRoot) {
            emit("move-project", dragState.path, null);
        } else if (isValidMoveTarget(dragState.path, dragState.overPath)) {
            emit("move-project", dragState.path, dragState.overPath);
        }

        dragState.path = null;
        dragState.overPath = null;
        dragState.overRoot = false;
        dragGhost.visible = false;
    }

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
}
provide("beginProjectDrag", beginProjectDrag);

// ----------------------------------------
// 新建项目 / 新建子项目：内联输入框，可选归属阶段
// ----------------------------------------
const showNewProjectInput = ref(false);
const newProjectPath = ref("");
const newProjectStage = ref("active");
const newProjectPrefix = ref(""); // 新建子项目时的父路径前缀
const newProjectInputRef = ref(null);

async function openNewProjectInput(parentPath = null) {
    showNewProjectInput.value = true;
    newProjectPrefix.value = parentPath ? `${parentPath}.` : "";
    newProjectPath.value = newProjectPrefix.value;
    newProjectStage.value = "active";
    await nextTick();
    newProjectInputRef.value?.focus();
}

function submitNewProject() {
    const path = newProjectPath.value.trim();
    if (!path || path === newProjectPrefix.value) {
        showNewProjectInput.value = false;
        return;
    }

    emit("create-project", path, newProjectStage.value);
    showNewProjectInput.value = false;
    newProjectPath.value = "";
}

// ----------------------------------------
// 右键菜单
// ----------------------------------------
const contextMenu = reactive({ visible: false, x: 0, y: 0, path: null });
const contextMenuNode = computed(() =>
    contextMenu.path ? props.projects[contextMenu.path] : null,
);

function onContextMenu(x, y, path) {
    contextMenu.visible = true;
    contextMenu.x = x;
    contextMenu.y = y;
    contextMenu.path = path;
}

function closeContextMenu() {
    contextMenu.visible = false;
}

// "移动到..." 候选目标：排除废纸篓/已归档项目、虚拟无项目节点、自己及自己的子项目、
// 以及自己当前已经所在的父项目（移过去等于没移）
const moveTargets = computed(() => {
    const path = contextMenu.path;
    if (!path) return [];

    const parts = path.split(".");
    const currentParent = parts.length > 1 ? parts.slice(0, -1).join(".") : null;
    const excludePrefix = `${path}.`;

    return Object.keys(props.projects)
        .filter((p) => p !== "无项目")
        .filter((p) => p !== path && !p.startsWith(excludePrefix))
        .filter((p) => p !== currentParent)
        .filter((p) => !props.projects[p]?.trashed)
        .filter((p) => !props.projects[p]?.archived)
        .sort()
        .map((p) => ({ path: p, label: p }));
});

function onMenuNewSubproject() {
    openNewProjectInput(contextMenu.path);
}

function onMenuMove(target) {
    emit("move-project", contextMenu.path, target);
}

function onMenuArchive(archived) {
    emit("toggle-archive", contextMenu.path, archived);
}

function onMenuTrash() {
    emit("trash-project", contextMenu.path);
}

function onMenuRestore() {
    emit("restore-project", contextMenu.path);
}

function onMenuPurge() {
    emit("purge-project", contextMenu.path);
}
</script>

<template>
    <aside
        class="project-tree"
        :style="{ width: `${panelWidth}px` }"
        :class="{ resizing: resizing || !!dragState.path }"
    >
        <!-- 头部标题栏 -->
        <div class="tree-header">
            <span class="tree-title">项目</span>
            <div class="tree-header-actions">
                <button
                    class="new-project-btn"
                    title="新建项目"
                    @click="openNewProjectInput()"
                >
                    +
                </button>
                <button
                    class="show-all-btn"
                    :class="{ active: selected === null }"
                    @click="emit('select', null)"
                >
                    全部
                </button>
            </div>

            <!-- 拖拽时悬浮显示，覆盖在标题栏上方，不挤占列表空间 -->
            <div
                v-if="dragState.path"
                class="drop-to-root"
                :class="{ active: dragState.overRoot }"
            >
                ⤓ 拖到此处移到顶层
            </div>
        </div>

        <!-- 新建项目输入框 -->
        <div v-if="showNewProjectInput" class="new-project-row">
            <input
                ref="newProjectInputRef"
                v-model="newProjectPath"
                class="new-project-input"
                placeholder="项目路径，如 personal.reading"
                @keydown.enter="submitNewProject"
                @keydown.esc="showNewProjectInput = false"
                @blur="showNewProjectInput = false"
            />
            <div class="new-project-stage">
                <button
                    class="stage-btn"
                    :class="{ active: newProjectStage === 'planned' }"
                    @mousedown.prevent="newProjectStage = 'planned'"
                >
                    计划中
                </button>
                <button
                    class="stage-btn"
                    :class="{ active: newProjectStage === 'active' }"
                    @mousedown.prevent="newProjectStage = 'active'"
                >
                    进行中
                </button>
            </div>
        </div>

        <div class="tree-body">
            <template v-for="group in groups" :key="group.key">
                <div
                    v-if="
                        (group.key !== 'archived' && group.key !== 'trash') ||
                        group.roots.length > 0
                    "
                    class="group-section"
                >
                    <div
                        class="group-header"
                        @click="toggleGroup(group.key)"
                    >
                        <span class="toggle-icon">{{
                            collapsedGroups[group.key] ? "▶" : "▼"
                        }}</span>
                        <span>{{ group.label }} ({{ group.roots.length }})</span>
                    </div>

                    <template v-if="!collapsedGroups[group.key]">
                        <ProjectTreeNode
                            v-for="root in group.roots"
                            :key="root"
                            :path="root"
                            :projects="projects"
                            :selected="selected"
                            :collapsed="collapsed"
                            @select="
                                (path) =>
                                    emit(
                                        'select',
                                        path === selected ? null : path,
                                    )
                            "
                            @toggle="onToggle"
                            @toggle-archive="
                                (path, archived) =>
                                    emit('toggle-archive', path, archived)
                            "
                            @set-stage="
                                (path, stage) => emit('set-stage', path, stage)
                            "
                            @trash-project="
                                (path) => emit('trash-project', path)
                            "
                            @restore-project="
                                (path) => emit('restore-project', path)
                            "
                            @context-menu="onContextMenu"
                        />

                        <div
                            v-if="group.roots.length === 0"
                            class="empty-hint"
                        >
                            暂无项目
                        </div>
                    </template>
                </div>
            </template>
        </div>

        <!-- 面板右边缘拖拽调整宽度的把手 -->
        <div class="resize-handle" @mousedown="startResize"></div>

        <ProjectContextMenu
            :visible="contextMenu.visible"
            :x="contextMenu.x"
            :y="contextMenu.y"
            :node="contextMenuNode"
            :move-targets="moveTargets"
            @close="closeContextMenu"
            @new-subproject="onMenuNewSubproject"
            @move="onMenuMove"
            @toggle-archive="onMenuArchive"
            @trash="onMenuTrash"
            @restore="onMenuRestore"
            @purge="onMenuPurge"
        />

        <!-- 跟随光标的拖拽提示牌 -->
        <Teleport to="body">
            <div
                v-if="dragGhost.visible"
                class="drag-ghost"
                :style="{
                    transform: `translate(${dragGhost.x}px, ${dragGhost.y}px) translate(-50%, -140%)`,
                }"
            >
                📁 {{ dragGhost.label }}
            </div>
        </Teleport>
    </aside>
</template>

<style scoped>
/* 面板容器 */
.project-tree {
    position: relative;
    flex-shrink: 0;
    background: var(--bg-panel);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
}
.project-tree.resizing {
    user-select: none;
}

/* 头部标题 */
.tree-header {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px 6px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
}

.tree-title {
    font-weight: 700;
    color: var(--cyan);
    font-size: 1rem;
}

.tree-header-actions {
    display: flex;
    align-items: center;
    gap: 6px;
}

/* 新建项目按钮 */
.new-project-btn {
    width: 18px;
    height: 18px;
    line-height: 1;
    border-radius: 4px;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    font-size: 1rem;
    transition: all 0.15s;
}
.new-project-btn:hover {
    color: var(--blue);
    border-color: var(--blue);
}

/* 新建项目输入行 */
.new-project-row {
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
}
.new-project-input {
    width: 100%;
    font-size: 0.9231rem;
}
.new-project-stage {
    display: flex;
    gap: 4px;
}
.stage-btn {
    flex: 1;
    font-size: 0.8462rem;
    padding: 3px 0;
    border-radius: 4px;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    transition: all 0.15s;
}
.stage-btn:hover {
    color: var(--fg);
}
.stage-btn.active {
    color: var(--blue);
    border-color: var(--blue);
    background: rgba(122, 162, 247, 0.1);
}

/* "全部"按钮 */
.show-all-btn {
    font-size: 0.8462rem;
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    transition: all 0.15s;
}
.show-all-btn:hover {
    color: var(--fg);
}
.show-all-btn.active {
    color: var(--blue);
    border-color: var(--blue);
}

/* 拖拽时的"移到顶层"投放区：悬浮覆盖在标题栏上方，不挤占列表空间 */
.drop-to-root {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-panel);
    border: 1px dashed var(--fg-dark);
    border-radius: 4px;
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition: all 0.12s;
}
.drop-to-root.active {
    border-color: var(--blue);
    background: rgba(122, 162, 247, 0.12);
    color: var(--blue);
}

/* 分组 */
.group-section {
    margin-top: 6px;
}
.group-section + .group-section {
    border-top: 1px solid var(--border);
    padding-top: 4px;
}
.group-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    cursor: pointer;
    color: var(--fg-dim);
    font-size: 0.8462rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    transition: color 0.15s;
}
.group-header:hover {
    color: var(--fg);
}
.group-header .toggle-icon {
    font-size: 0.7692rem;
    width: 14px;
    text-align: center;
    flex-shrink: 0;
}

.empty-hint {
    padding: 4px 16px 8px;
    color: var(--fg-dark);
    font-size: 0.8462rem;
}

/* 滚动区域 */
.tree-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
}
.tree-body::-webkit-scrollbar {
    width: 4px;
}
.tree-body::-webkit-scrollbar-thumb {
    background: var(--fg-dark);
    border-radius: 2px;
}

/* 右边缘宽度拖拽把手 */
.resize-handle {
    position: absolute;
    top: 0;
    right: -3px;
    width: 6px;
    height: 100%;
    cursor: col-resize;
    z-index: 10;
}
.resize-handle:hover {
    background: rgba(122, 162, 247, 0.25);
}

/* 跟随光标的拖拽提示牌 */
.drag-ghost {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 2000;
    pointer-events: none;
    padding: 5px 12px;
    border-radius: 6px;
    background: var(--bg-popup);
    border: 1px solid var(--blue);
    color: var(--fg);
    font-size: 0.85rem;
    font-weight: 600;
    white-space: nowrap;
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.25);
    transition:
        transform 0.08s ease-out,
        opacity 0.15s;
    animation: drag-ghost-in 0.15s ease-out;
}

@keyframes drag-ghost-in {
    from {
        opacity: 0;
        scale: 0.85;
    }
    to {
        opacity: 1;
        scale: 1;
    }
}
</style>
