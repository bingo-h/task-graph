<!--
  @Component: 根组件
  @Description: 负责从后端拉取组件，管理全局状态，协调三个面板，分发写操作到API
  @Author: Bin.H
  @Date: 2026-05-23
-->

<script setup>
import { ref, computed, onMounted } from "vue";

import TaskFormModal from "./components/TaskFormModal.vue";
import ProjectTree from "./components/ProjectTree.vue";
import TaskGraph from "./components/TaskGraph.vue";
import TaskDetail from "./components/TaskDetail.vue";
import { computeHighlight } from "./composables/useLayout";
import {
    fetchTasks,
    addTask,
    modifyTask,
    doneTask,
    deleteTask,
} from "./composables/useApi";

// 全局状态
const nodes = ref([]); // 所有任务节点
const edges = ref([]); // 所有边
const projects = ref({});
const projectRoots = ref([]);

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

// 添加/修改任务
const showModal = ref(false); // 是否显示添加任务界面
const modalPrefill = ref(null); // null = 新建，任务对象 = 修改

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
        projectRoots.value = data.project_roots;
    } catch (e) {
        error.value = e.message;
    } finally {
        loading.value = false;
    }
}

/**
 * 写操作成功后用后端返回的最新数据刷新
 */
function applyUpdate(data) {
    nodes.value = data.nodes;
    edges.value = data.edges;
    projects.value = data.projects;
    projectRoots.value = data.project_roots;

    if (
        selectedUUID.value &&
        !data.nodes.find((n) => n.uuid === selectedUUID.value)
    ) {
        selectedUUID.value = null;
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
 * @param {string} command - 命令字符串
 */
async function onModalSubmit({ mode, uuid, command }) {
    try {
        if (mode === "add") {
            applyUpdate(await addTask(command));
        } else {
            applyUpdate(await modifyTask(uuid, command));
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

onMounted(load);
</script>

<template>
    <div class="app">
        <!-- 顶部导航栏 -->
        <header class="topbar">
            <span class="app-title">task-web</span>

            <!-- 高亮模式选择 -->
            <div class="hightlight-mode">
                <span class="mode-label">高亮模式: </span>
                <button
                    v-for="m in [
                        { key: 'ancestors', label: '祖先链路' },
                        { key: 'neighbors', label: '直接上下游' },
                        { key: 'full', label: '完整链路' },
                    ]"
                    :key="m.key"
                    class="mode-btn"
                    :class="{ active: hlMode === m.key }"
                    @click="hlMode = m.key"
                >
                    {{ m.label }}
                </button>
            </div>

            <!-- 添加任务按钮 -->
            <button class="btn-add-toggle" @click="openAdd">+ 添加任务</button>

            <!-- 刷新按钮 -->
            <button class="btn-refresh" @click="load" :disabled="loading">
                {{ loading ? "加载中…" : "↺ 刷新" }}
            </button>
        </header>

        <!-- 错误提示 -->
        <div v-if="error" class="error-bar" @click="error = ''">
            ⚠ {{ error }} <span class="dismiss">×</span>
        </div>

        <!-- 主体三栏 -->
        <div class="main">
            <ProjectTree
                :projects="projects"
                :roots="projectRoots"
                :selected="selectedProject"
                @select="selectedProject = $event"
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
                :task="selectedTask"
                :all-tasks="nodes"
                @done="onDone"
                @delete="onDelete"
                @modify="openModify"
                @select="selectedUUID = $event"
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
    font-size: 15px;
    color: var(--cyan);
    margin-right: 8px;
}

.highlight-mode {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
}

.mode-label {
    color: var(--fg-dim);
    font-size: 12px;
}

.mode-btn {
    padding: 3px 10px;
    border-radius: 4px;
    font-size: 12px;
    color: var(--fg-dim);
}

.mode-btn:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}

.mode-btn.active {
    color: var(--blue);
    border-color: var(--blue);
    background: rgba(122, 162, 247, 0.1);
}

.btn-add-toggle {
    padding: 4px 14px;
    border-radius: 6px;
    background: var(--blue);
    color: var(--bg);
    font-weight: 600;
    font-size: 12px;
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
    font-size: 12px;
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

/* 错误提示栏 */
.error-bar {
    padding: 8px 16px;
    background: rgba(247, 118, 142, 0.15);
    color: var(--red);
    border-bottom: 1px solid var(--red);
    font-size: 12px;
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
