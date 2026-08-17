<!--
  @Component: 根组件
  @Description: 负责从后端拉取组件，管理全局状态，协调三个面板，分发写操作到API
  @Author: Bin.H
  @Date: 2026-05-23
-->

<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

import TaskFormModal from "./components/TaskFormModal.vue";
import ProjectTree from "./components/ProjectTree.vue";
import TaskGraph from "./components/TaskGraph.vue";
import TaskDetail from "./components/TaskDetail.vue";
import SettingsModal from "./components/SettingsModal.vue";
import TagManagerModal from "./components/TagManagerModal.vue";
import Dashboard from "./components/Dashboard.vue";
import ChartsPage from "./components/ChartsPage.vue";
import CalendarPage from "./components/CalendarPage.vue";
import TimeEntryNoteModal from "./components/TimeEntryNoteModal.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import { confirmDialog } from "./composables/useConfirm";
import { computeHighlight, wouldCreateCycle } from "./composables/useLayout";
import constants from "./config/constants";
import {
    formatDuration,
    setDurationFormat,
    DEFAULT_DURATION_FORMAT,
} from "./composables/useDuration";
import {
    fetchTasks,
    createProject,
    setProjectArchived,
    setProjectStage,
    trashProject,
    restoreProject,
    purgeProject,
    moveProject,
    getSettings,
    saveSettings,
    addTask,
    modifyTask,
    reconnectDependency,
    renameTag,
    setTagColor,
    deleteTag,
    doneTask,
    doneTasks,
    setTasksProject,
    undoneTask,
    setTaskToday,
    setTasksToday,
    deleteTask,
    deleteTasks,
    startTimer,
    stopTimer,
    startGroupTimer,
    saveTimeEntryNote,
    deleteTimeEntry,
    setTaskRecur,
    addTodayOrderEdge,
    removeTodayOrderEdge,
    reorderSiblings,
} from "./composables/useApi";

// 无边框窗口：自定义标题栏控制
const appWindow = getCurrentWindow();
function minimizeWindow() {
    appWindow.minimize();
}
function toggleMaximizeWindow() {
    appWindow.toggleMaximize();
}
function closeWindow() {
    appWindow.close();
}

// 全局状态
const nodes = ref([]); // 所有任务节点
const edges = ref([]); // 所有边（真实依赖关系）
const todayOrderEdges = ref([]); // "今日任务"视图下用户手动排的顺序边，独立于 edges
const siblingOrderEdges = ref([]); // DAG 视图里同一层级任务的手动纵向顺序边，独立于 edges
const projects = ref({});
const plannedProjectRoots = ref([]);
const activeProjectRoots = ref([]);
const archivedProjectRoots = ref([]);
const trashProjectRoots = ref([]);
const tags = ref({}); // 标签名 -> { name, color, task_count }
const showTagManager = ref(false);

// 应用设置（保存在独立的 settings.json 中，方便导出/同步）
const settings = ref({
    trash_retention_days: 30,
    font_size: 14,
    font_family: "sans-serif",
    node_font_family: "",
    duration_format: DEFAULT_DURATION_FORMAT,
    default_due_time: "23:59",
    node_show_project: true,
    node_show_due: true,
    node_show_priority: true,
    node_show_recur: true,
    node_label_project: constants.DEFAULT_NODE_LABELS.project,
    node_label_due: constants.DEFAULT_NODE_LABELS.due,
    node_label_priority: constants.DEFAULT_NODE_LABELS.priority,
    node_label_recur: constants.DEFAULT_NODE_LABELS.recur,
});
const showSettings = ref(false);

/** 把字体大小应用到全局 CSS 变量 */
function applyFontSize(size) {
    document.documentElement.style.setProperty("--app-font-size", `${size}px`);
}

// CSS 通用字体族关键字：这几个是浏览器保留字，写进 font-family 时不能加引号，
// 否则会被当成一个叫 "serif" / "sans-serif" 的具体字体名去找，肯定找不到
const CSS_GENERIC_FONT_FAMILIES = new Set([
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
]);

/** 把字体名转成能直接拼进 font-family 的写法：CSS 通用族关键字不加引号，具体字体名加引号；空值返回 null */
function cssFontFamilyValue(family) {
    const trimmed = (family || "").trim();
    if (!trimmed) return null;
    return CSS_GENERIC_FONT_FAMILIES.has(trimmed) ? trimmed : `"${trimmed}"`;
}

/** 把全局字体家族应用到 CSS 变量；sans-serif 兜底防止选中的字体加载失败时排版跑掉 */
function applyFontFamily(family) {
    const primary = cssFontFamilyValue(family) || "sans-serif";
    document.documentElement.style.setProperty(
        "--app-font-family",
        `${primary}, sans-serif`,
    );
}

/**
 * 把图谱任务节点卡片单独的字体应用到 CSS 变量；节点字体留空表示跟随全局字体——
 * 这里直接在 JS 里解析好最终值写进变量，而不是靠 CSS var() 对"自定义属性被显式设为空值"
 * 时是否触发 fallback 兜底（不同 WebView 引擎对这个边界情况的实现不完全一致，
 * 在 JS 里解析好更可靠，行为不用依赖具体渲染引擎）。
 */
function applyNodeFontFamily(nodeFamily, appFamily) {
    const nodePrimary = cssFontFamilyValue(nodeFamily);
    const appPrimary = cssFontFamilyValue(appFamily) || "sans-serif";
    const value = nodePrimary
        ? `${nodePrimary}, ${appPrimary}, sans-serif`
        : `${appPrimary}, sans-serif`;
    document.documentElement.style.setProperty("--app-node-font-family", value);
}

// 当前页面："home" 首页仪表盘 / "board" 任务看板（原有的三栏视图）/ "charts" 分析页 / "calendar" 日历页
const currentPage = ref("home");

/**
 * 首页"今日任务"或分析页里点击某个任务，跳转到任务看板、选中它，
 * 并把项目筛选切到它所属的项目（没有项目则切到"无项目"分类），这样看板直接显示这个任务所在的流程图，
 * 不用手动去项目树里再点一次
 *
 * @description 由 Dashboard / ChartsPage 的 @jump-to-task 事件触发
 * @param {string} uuid - 任务 UUID
 */
function onJumpToTask(uuid) {
    currentPage.value = "board";
    selectedUUID.value = uuid;

    const task = nodes.value.find((n) => n.uuid === uuid);
    selectedProject.value = task?.project || constants.INBOX_PROJECT;
}

// 当前状态
const selectedUUID = ref(null);
const selectedProject = ref(null);
const tagFilter = ref(null); // 任务看板按标签筛选，null 表示不筛选
const hlMode = ref("ancestors"); // 高亮模式
const loading = ref(false);
const error = ref("");

// 新建任务弹窗的默认项目：selectedProject 也可能是分类哨兵值（如 __stage__planned）
// 或无项目/今日任务虚拟节点，这些都不是真实项目路径，不能当默认项目回填进表单
const defaultProjectForNewTask = computed(() =>
    selectedProject.value &&
    !selectedProject.value.startsWith(constants.STAGE_FILTER_PREFIX) &&
    selectedProject.value !== constants.INBOX_PROJECT &&
    selectedProject.value !== constants.TODAY_PROJECT
        ? selectedProject.value
        : null,
);

// ----------------------------------------
// "今日任务"分类下的独立排序图：不显示真实依赖边，改显示用户手动排的今日顺序
// ----------------------------------------
const graphMode = computed(() =>
    selectedProject.value === constants.TODAY_PROJECT ? "today-order" : "depends",
);
const graphEdges = computed(() =>
    graphMode.value === "today-order" ? todayOrderEdges.value : edges.value,
);
const todayCount = computed(() => nodes.value.filter((n) => n.is_today).length);

// 任务看板图谱里框选 / Ctrl+点击多选中的任务，用于批量操作工具栏
const multiSelectedUUIDs = ref(new Set());

// 派生状态
const selectedTask = computed(
    () => nodes.value.find((n) => n.uuid === selectedUUID.value) || null,
); // 当前选中的任务对象，未选中时为null

// 用当前实际展示的边（今日任务视图下是手动排序边，否则是真实依赖）算链路高亮，
// 这样选中节点后淡化/高亮的范围跟图上画的线是同一套关系，不会对不上
const highlightSet = computed(() =>
    computeHighlight(selectedUUID.value, graphEdges.value, hlMode.value),
);

// ----------------------------------------
// 全局计时状态（标题栏悬浮秒表）
// ----------------------------------------
/** 当前正在计时的所有任务：单个任务计时时只有一个；框选多个任务批量计时时是共享同一段时间的一批 */
const activeTimingNodes = computed(() => nodes.value.filter((n) => n.is_timing));

/** 单任务计时模式下的那一个任务，供保持原有悬浮秒表 UI 不变 */
const activeTimingTask = computed(() =>
    activeTimingNodes.value.length === 1 ? activeTimingNodes.value[0] : null,
);

const nowTick = ref(Date.now()); // 每秒刷新，驱动秒表实时跳动
const tickTimer = setInterval(() => {
    nowTick.value = Date.now();
}, 1000);
onUnmounted(() => clearInterval(tickTimer));

/** 本次专注时长（秒）：从这一段计时开始到现在，不含之前的历史累计
 *  批量计时的多个任务共享同一个开始时间，取第一个即可 */
const activeSessionSeconds = computed(() => {
    const since = activeTimingNodes.value[0]?.active_since;
    if (!since) return 0;
    return Math.max(0, Math.floor((nowTick.value - new Date(since).getTime()) / 1000));
});

// 添加/修改任务
const showModal = ref(false); // 是否显示添加任务界面
const modalPrefill = ref(null); // null = 新建，任务对象 = 修改

// 计时记录回忆总结弹窗
const noteModal = ref({
    visible: false,
    entryId: null,
    heading: "回忆总结",
    title: "",
    body: "",
});
const taskDetailRef = ref(null); // 保存总结后用来刷新 TaskDetail 的计时记录列表

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
        todayOrderEdges.value = data.today_order_edges;
        siblingOrderEdges.value = data.sibling_order_edges;
        projects.value = data.projects;
        plannedProjectRoots.value = data.planned_project_roots;
        activeProjectRoots.value = data.active_project_roots;
        archivedProjectRoots.value = data.archived_project_roots;
        trashProjectRoots.value = data.trash_project_roots;
        tags.value = data.tags;
    } catch (e) {
        error.value = e.message;
    } finally {
        loading.value = false;
    }
}

/** 加载应用设置并应用字体大小、字体、计时时长显示格式 */
async function loadSettings() {
    try {
        settings.value = await getSettings();
        applyFontSize(settings.value.font_size);
        applyFontFamily(settings.value.font_family);
        applyNodeFontFamily(settings.value.node_font_family, settings.value.font_family);
        setDurationFormat(settings.value.duration_format);
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 写操作成功后用后端返回的最新数据刷新
 */
function applyUpdate(data) {
    nodes.value = data.nodes;
    edges.value = data.edges;
    todayOrderEdges.value = data.today_order_edges;
    siblingOrderEdges.value = data.sibling_order_edges;
    projects.value = data.projects;
    plannedProjectRoots.value = data.planned_project_roots;
    activeProjectRoots.value = data.active_project_roots;
    archivedProjectRoots.value = data.archived_project_roots;
    trashProjectRoots.value = data.trash_project_roots;
    tags.value = data.tags;

    // 筛选中的标签被删掉了（比如刚在标签管理面板里删除），清掉筛选
    if (tagFilter.value && !data.tags[tagFilter.value]) {
        tagFilter.value = null;
    }

    if (
        selectedUUID.value &&
        !data.nodes.find((n) => n.uuid === selectedUUID.value)
    ) {
        selectedUUID.value = null;
    }

    // 清理多选集合里已经不存在的任务（比如被其他地方删除了）
    if (multiSelectedUUIDs.value.size > 0) {
        const stillValid = new Set(
            [...multiSelectedUUIDs.value].filter((uuid) =>
                data.nodes.some((n) => n.uuid === uuid),
            ),
        );
        if (stillValid.size !== multiSelectedUUIDs.value.size) {
            multiSelectedUUIDs.value = stillValid;
        }
    }
}

/**
 * 新建项目（可在没有任何任务的情况下独立创建）
 *
 * @description 由 ProjectTree 的 @create-project 事件触发
 * @param {string} path - 项目路径，如 "personal.reading"
 * @param {string} stage - "planned"（计划中）| "active"（进行中）
 */
async function onCreateProject(path, stage) {
    try {
        applyUpdate(await createProject(path, stage));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 切换项目归档状态（归档会级联到所有子项目）
 *
 * @description 由 ProjectTree 的 @toggle-archive 事件触发
 * @param {string} path - 项目路径
 * @param {boolean} archived - 归档目标状态
 */
async function onToggleArchive(path, archived) {
    try {
        applyUpdate(await setProjectArchived(path, archived));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 设置项目所属阶段（计划中 / 进行中），只能对顶层项目操作，子项目跟随顶层项目一起移动
 *
 * @description 由 ProjectTree 的 @set-stage 事件触发
 * @param {string} path - 顶层项目路径
 * @param {string} stage - "planned" | "active"
 */
async function onSetStage(path, stage) {
    try {
        applyUpdate(await setProjectStage(path, stage));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 将项目移入废纸篓（级联到所有子项目，可从废纸篓恢复，不会立即删除任务）
 *
 * @description 由 ProjectTree 的 @trash-project 事件触发
 * @param {string} path - 项目路径
 */
async function onTrashProject(path) {
    try {
        applyUpdate(await trashProject(path));
        if (selectedProject.value === path) selectedProject.value = null;
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 从废纸篓恢复项目
 *
 * @description 由 ProjectTree 的 @restore-project 事件触发
 * @param {string} path - 项目路径
 */
async function onRestoreProject(path) {
    try {
        applyUpdate(await restoreProject(path));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 彻底删除项目，级联删除该项目及所有子项目下的任务，不可恢复，需用户二次确认
 *
 * @description 由 ProjectTree 的 @purge-project 事件触发
 * @param {string} path - 项目路径
 */
async function onPurgeProject(path) {
    const ok = await confirmDialog(
        `确认彻底删除项目"${path}"？其下所有子项目和任务都无法恢复。`,
        { danger: true, confirmText: "彻底删除" },
    );
    if (!ok) return;

    try {
        applyUpdate(await purgeProject(path));
        if (selectedProject.value === path) selectedProject.value = null;
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 移动项目（及其子项目、任务）到新的父项目下，或移到顶层
 *
 * @description 由 ProjectTree 的 @move-project 事件触发（拖拽或右键菜单）
 * @param {string} path - 被移动的项目路径
 * @param {?string} newParent - 新的父项目路径，null 表示移到顶层
 */
async function onMoveProject(path, newParent) {
    try {
        applyUpdate(await moveProject(path, newParent));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 保存应用设置
 *
 * @description 由 SettingsModal 的 @save 事件触发
 * @param {object} newSettings - { trash_retention_days, font_size, font_family, node_font_family }
 */
async function onSaveSettings(newSettings) {
    try {
        settings.value = await saveSettings(newSettings);
        applyFontSize(settings.value.font_size);
        applyFontFamily(settings.value.font_family);
        applyNodeFontFamily(settings.value.node_font_family, settings.value.font_family);
        setDurationFormat(settings.value.duration_format);
        showSettings.value = false;
    } catch (e) {
        error.value = e.message;
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
 * @param {object} fields - 结构化任务字段
 */
async function onModalSubmit({ mode, uuid, fields, recurRuleChanged, recurRule }) {
    try {
        if (mode === "add") {
            applyUpdate(await addTask(fields));
        } else {
            applyUpdate(await modifyTask(uuid, fields));
            // 只有用户实际改动过重复设置才调用，否则每次编辑任务的其它字段
            // （比如改个描述）都会误触发"重新开始周期"的重置逻辑
            if (recurRuleChanged) {
                applyUpdate(await setTaskRecur(uuid, recurRule));
            }
        }

        showModal.value = false;
        error.value = "";
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 批量重命名标签：所有用到旧标签的任务一起改成新标签
 *
 * @description 由 TaskFormModal 的 @rename-tag 事件触发
 * @param {string} oldTag
 * @param {string} newTag
 */
async function onRenameTag(oldTag, newTag) {
    try {
        applyUpdate(await renameTag(oldTag, newTag));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 设置标签颜色
 *
 * @description 由 TagManagerModal 的 @set-color 事件触发
 * @param {string} name
 * @param {string|null} color
 */
async function onSetTagColor(name, color) {
    try {
        applyUpdate(await setTagColor(name, color));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 彻底删除一个标签，需用户二次确认
 *
 * @description 由 TagManagerModal 的 @delete-tag 事件触发
 * @param {string} name
 */
async function onDeleteTag(name) {
    const ok = await confirmDialog(
        `确认删除标签"${name}"？将从所有任务上移除，此操作不可恢复。`,
        { danger: true, confirmText: "删除" },
    );
    if (!ok) return;

    try {
        applyUpdate(await deleteTag(name));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 点击某个标签：跳转到任务看板，并按这个标签筛选图谱
 *
 * @description 由 TaskDetail / TagManagerModal 的 @filter-by-tag 事件触发
 * @param {string} name
 */
function onFilterByTag(name) {
    currentPage.value = "board";
    tagFilter.value = name;
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
 * 取消任务完成，恢复为待办
 *
 * @description 由 TaskDetail 的 @undone 事件触发
 * @param {string} uuid - 任务UUID
 */
async function onUndone(uuid) {
    try {
        applyUpdate(await undoneTask(uuid));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 设置/取消"今日任务"标记
 *
 * @description 由 TaskDetail 的 @set-today 事件触发
 * @param {string} uuid - 任务 UUID
 * @param {boolean} marked - 目标状态
 */
async function onSetToday(uuid, marked) {
    try {
        applyUpdate(await setTaskToday(uuid, marked));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 详情栏内联编辑备注，失焦后保存
 *
 * @description 由 TaskDetail 的 @update-annotation 事件触发
 * @param {string} uuid - 任务 UUID
 * @param {string} text - 备注全文，空字符串表示清空
 */
async function onUpdateAnnotation(uuid, text) {
    try {
        applyUpdate(
            await modifyTask(uuid, {
                annotation: text || null,
                clear_annotation: !text,
            }),
        );
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 开始为指定任务计时
 *
 * @description 由 TaskDetail 的 @start-timer 事件触发
 * @param {string} uuid - 任务UUID
 */
async function onStartTimer(uuid) {
    try {
        applyUpdate(await startTimer(uuid));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 停止当前正在进行的计时；结束后若有对应的计时记录，
 * 弹窗询问这段专注的回忆总结（可跳过）
 *
 * @description 由 TaskDetail 的 @stop-timer 事件触发
 */
async function onStopTimer() {
    try {
        const data = await stopTimer();
        applyUpdate(data);
        multiSelectedUUIDs.value = new Set(); // 若是批量计时被停止，顺带收起工具栏

        if (data.stopped_entry_id) {
            noteModal.value = {
                visible: true,
                entryId: data.stopped_entry_id,
                heading: "这段专注的回忆总结",
                title: "",
                body: "",
            };
        }
    } catch (e) {
        error.value = e.message;
    }
}

// ----------------------------------------
// 任务看板多选：框选 / Ctrl+点击，批量操作
// ----------------------------------------
/**
 * 单击任务节点（无 Ctrl），进入普通单选模式，退出多选
 *
 * @description 由 TaskGraph 的 @select 事件触发
 * @param {string|null} uuid - 任务 UUID，再次点击同一节点时为 null
 */
function onGraphSelect(uuid) {
    multiSelectedUUIDs.value = new Set();
    selectedUUID.value = uuid;
}

/**
 * Ctrl/Cmd + 点击任务节点，切换其多选状态
 *
 * @description 由 TaskGraph 的 @toggle-multi-select 事件触发
 * @param {string} uuid - 任务 UUID
 */
function onToggleMultiSelect(uuid) {
    selectedUUID.value = null; // 退出单选链路高亮，避免和多选的视觉提示混在一起

    const next = new Set(multiSelectedUUIDs.value);
    if (next.has(uuid)) next.delete(uuid);
    else next.add(uuid);
    multiSelectedUUIDs.value = next;
}

/**
 * 右键长按拖拽框选结束，命中的任务替换当前多选集合
 *
 * @description 由 TaskGraph 的 @box-select 事件触发
 * @param {string[]} uuids - 框选命中的任务 UUID 列表
 */
function onBoxSelect(uuids) {
    selectedUUID.value = null;
    multiSelectedUUIDs.value = new Set(uuids);
}

/** 清空多选（工具栏"取消选择"按钮 / 空框选） */
function onClearMultiSelect() {
    multiSelectedUUIDs.value = new Set();
}

/**
 * 批量标记完成
 *
 * @description 由 TaskGraph 的 @bulk-done 事件触发
 * @param {string[]} uuids - 选中的任务 UUID 列表
 */
async function onBulkDone(uuids) {
    try {
        applyUpdate(await doneTasks(uuids));
        multiSelectedUUIDs.value = new Set();
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 批量删除，需用户二次确认
 *
 * @description 由 TaskGraph 的 @bulk-delete 事件触发
 * @param {string[]} uuids - 选中的任务 UUID 列表
 */
async function onBulkDelete(uuids) {
    const ok = await confirmDialog(`确认删除选中的 ${uuids.length} 个任务？`, {
        danger: true,
        confirmText: "删除",
    });
    if (!ok) return;

    try {
        applyUpdate(await deleteTasks(uuids));
        multiSelectedUUIDs.value = new Set();
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 批量转移到另一个项目下
 *
 * @description 由 TaskGraph 的 @bulk-move-project 事件触发
 * @param {{ uuids: string[], project: string|null }} payload - 选中的任务 UUID 列表和目标项目路径（null 表示无项目）
 */
async function onBulkMoveProject({ uuids, project }) {
    try {
        applyUpdate(await setTasksProject(uuids, project));
        multiSelectedUUIDs.value = new Set();
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 批量设为今日任务
 *
 * @description 由 TaskGraph 的 @bulk-today 事件触发
 * @param {string[]} uuids - 选中的任务 UUID 列表
 */
async function onBulkToday(uuids) {
    try {
        applyUpdate(await setTasksToday(uuids, true));
        multiSelectedUUIDs.value = new Set();
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 批量开始计时：选中的这批任务共享同一段开始时间，各自单独记一条计时记录，
 * 停止时（标题栏悬浮秒表的停止按钮）只结束这段计时，不会连带标记任务完成
 *
 * @description 由 TaskGraph 的 @bulk-start-timer 事件触发
 * @param {string[]} uuids - 选中的任务 UUID 列表
 */
async function onBulkStartTimer(uuids) {
    try {
        applyUpdate(await startGroupTimer(uuids));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 拖拽节点边框的连接点建立依赖关系：右侧点拖出去，起点是终点的前置任务；
 * 左侧点拖出去，起点是终点的后置任务。换算成 depends 就是"后置任务依赖前置任务"，
 * 写到后置任务的 depends 字段里
 *
 * @description 由 TaskGraph 的 @connect-nodes 事件触发
 * @param {{ fromUuid: string, fromSide: "left"|"right", toUuid: string }} payload
 */
async function onConnectNodes({ fromUuid, fromSide, toUuid }) {
    const precursorUuid = fromSide === "right" ? fromUuid : toUuid;
    const successorUuid = fromSide === "right" ? toUuid : fromUuid;

    // "今日任务"视图下拖出的连线是手动排序边，不是真实依赖关系；
    // 后端会校验不能和真实依赖图矛盾、不能在排序图里成环
    if (graphMode.value === "today-order") {
        const already = todayOrderEdges.value.some(
            (e) => e.source === precursorUuid && e.target === successorUuid,
        );
        if (already) return;

        try {
            applyUpdate(await addTodayOrderEdge(precursorUuid, successorUuid));
        } catch (e) {
            error.value = e.message;
        }
        return;
    }

    const successor = nodes.value.find((n) => n.uuid === successorUuid);
    if (!successor) return;

    if (successor.depends.includes(precursorUuid)) return; // 已经连过了

    if (wouldCreateCycle(edges.value, precursorUuid, successorUuid)) {
        error.value = "不能这样连：会在依赖关系里形成循环";
        return;
    }

    try {
        applyUpdate(
            await modifyTask(successorUuid, {
                depends: [...successor.depends, precursorUuid],
            }),
        );
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 拖拽已有依赖连线的终点：拖到空白处删除这条依赖，拖到另一个任务则改指向它
 *
 * @description 由 TaskGraph 的 @reconnect-edge 事件触发
 * @param {{ sourceUuid: string, oldTargetUuid: string, newTargetUuid: string|null }} payload
 */
async function onReconnectEdge({ sourceUuid, oldTargetUuid, newTargetUuid }) {
    if (graphMode.value === "today-order") {
        try {
            const afterRemove = await removeTodayOrderEdge(sourceUuid, oldTargetUuid);
            applyUpdate(
                newTargetUuid && newTargetUuid !== sourceUuid
                    ? await addTodayOrderEdge(sourceUuid, newTargetUuid)
                    : afterRemove,
            );
        } catch (e) {
            error.value = e.message;
        }
        return;
    }

    if (newTargetUuid) {
        if (newTargetUuid === sourceUuid) {
            error.value = "不能把依赖指向自己";
            return;
        }

        const newTarget = nodes.value.find((n) => n.uuid === newTargetUuid);
        if (!newTarget) return;

        if (newTarget.depends.includes(sourceUuid)) {
            error.value = "这个任务已经依赖它了";
            return;
        }

        // 用去掉这条旧边之后的边集合做环检测，避免把"即将被删除的旧边"也算进去误判
        const edgesWithoutOld = edges.value.filter(
            (e) => !(e.source === sourceUuid && e.target === oldTargetUuid),
        );
        if (wouldCreateCycle(edgesWithoutOld, sourceUuid, newTargetUuid)) {
            error.value = "不能这样连：会在依赖关系里形成循环";
            return;
        }
    }

    try {
        applyUpdate(
            await reconnectDependency(sourceUuid, oldTargetUuid, newTargetUuid),
        );
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * DAG 视图里长按拖拽调整了同一层级任务的纵向顺序，落定后整列节点的新顺序回传过来
 *
 * @description 由 TaskGraph 的 @reorder-siblings 事件触发
 * @param {{ uuids: string[] }} payload - 这一列节点落定后的完整新顺序
 */
async function onReorderSiblings({ uuids }) {
    try {
        applyUpdate(await reorderSiblings(uuids));
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 点击"计时记录"里已有的总结标题，打开弹窗查看/修改
 *
 * @description 由 TaskDetail 的 @edit-time-entry-note 事件触发
 * @param {object} entry - 计时记录对象 { id, note_title, note_body, ... }
 */
function onEditTimeEntryNote(entry) {
    noteModal.value = {
        visible: true,
        entryId: entry.id,
        heading: "回忆总结",
        title: entry.note_title || "",
        body: entry.note_body || "",
    };
}

/** 保存回忆总结弹窗的内容，成功后刷新任务详情里的计时记录列表 */
async function onSaveTimeEntryNote({ title, body }) {
    try {
        await saveTimeEntryNote(
            noteModal.value.entryId,
            title || null,
            body || null,
        );
        noteModal.value.visible = false;
        await taskDetailRef.value?.refresh();
    } catch (e) {
        error.value = e.message;
    }
}

/**
 * 删除某段计时记录，需用户二次确认，成功后刷新任务详情和总耗时
 *
 * @description 由 TaskDetail 的 @delete-time-entry 事件触发
 * @param {number} entryId - 计时记录 id
 */
async function onDeleteTimeEntry(entryId) {
    const ok = await confirmDialog("确认删除这段计时记录？此操作不可恢复。", {
        danger: true,
        confirmText: "删除",
    });
    if (!ok) return;

    try {
        applyUpdate(await deleteTimeEntry(entryId));
        await taskDetailRef.value?.refresh();
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
    const ok = await confirmDialog("确认删除此任务？", {
        danger: true,
        confirmText: "删除",
    });
    if (!ok) return;

    try {
        applyUpdate(await deleteTask(uuid));
    } catch (e) {
        error.value = e.message;
    }
}

onMounted(() => {
    load();
    loadSettings();
});

// "今日任务"标记、逾期等状态只在每次拉取数据时惰性重新计算（后端没有常驻定时任务），
// 应用如果一直开着不关、也不做任何触发刷新的操作（比如正好跨过午夜），
// 界面会一直停留在旧状态。这里跟 Dashboard/ChartsPage 一样加一个定时兜底刷新。
const AUTO_REFRESH_MS = 5 * 60 * 1000; // 5 分钟
let autoRefreshTimer = null;
onMounted(() => {
    autoRefreshTimer = setInterval(load, AUTO_REFRESH_MS);
});
onUnmounted(() => clearInterval(autoRefreshTimer));
</script>

<template>
    <div class="app">
        <!-- 顶部导航栏（兼具自定义标题栏，可拖拽） -->
        <header class="topbar" data-tauri-drag-region>
            <span class="app-title" data-tauri-drag-region>task-graph</span>

            <!-- 页面切换：首页仪表盘 / 任务看板 -->
            <div class="page-nav">
                <button
                    class="page-nav-btn"
                    :class="{ active: currentPage === 'home' }"
                    @click="currentPage = 'home'"
                >
                    首页
                </button>
                    
                <button
                    class="page-nav-btn"
                    :class="{ active: currentPage === 'charts' }"
                    @click="currentPage = 'charts'"
                >
                    分析
                </button>

                <button
                    class="page-nav-btn"
                    :class="{ active: currentPage === 'calendar' }"
                    @click="currentPage = 'calendar'"
                >
                    日历
                </button>

                <button
                    class="page-nav-btn"
                    :class="{ active: currentPage === 'board' }"
                    @click="currentPage = 'board'"
                >
                    任务看板
                </button>
            </div>

            <!-- 当前活跃计时任务：悬浮秒表，点击任务名可跳转，点击方块停止 -->
            <div v-if="activeTimingTask" class="active-timer-pill">
                <span class="active-timer-dot"></span>
                <span
                    class="active-timer-desc"
                    @click="selectedUUID = activeTimingTask.uuid"
                >
                    {{ activeTimingTask.description }}
                </span>
                <span class="active-timer-clock">
                    {{ formatDuration(activeSessionSeconds) }}
                </span>
                <button
                    class="active-timer-stop"
                    title="停止计时"
                    @click="onStopTimer"
                >
                    ■
                </button>
            </div>

            <!-- 批量计时（框选/Ctrl 多选后一起开始的计时）：只是给这几个任务同时记一段时长，
                 不会连带标记完成；鼠标悬浮可看到具体是哪些任务 -->
            <div
                v-else-if="activeTimingNodes.length > 1"
                class="active-timer-pill active-timer-pill-group"
            >
                <span class="active-timer-dot"></span>
                <span class="active-timer-desc">
                    正在为 {{ activeTimingNodes.length }} 个任务计时
                </span>
                <span class="active-timer-clock">
                    {{ formatDuration(activeSessionSeconds) }}
                </span>
                <button
                    class="active-timer-stop"
                    title="停止计时"
                    @click="onStopTimer"
                >
                    ■
                </button>

                <!-- 鼠标悬浮展开：这一段时间里同时在计时的任务列表 -->
                <div class="active-timer-tasklist">
                    <div
                        v-for="n in activeTimingNodes"
                        :key="n.uuid"
                        class="active-timer-tasklist-item"
                        @click="selectedUUID = n.uuid"
                    >
                        {{ n.description }}
                    </div>
                </div>
            </div>

            <!-- 添加任务按钮 -->
            <button class="btn-add-toggle" @click="openAdd">+ 添加任务</button>

            <!-- 刷新按钮 -->
            <button class="btn-refresh" @click="load" :disabled="loading">
                {{ loading ? "加载中…" : "↺ 刷新" }}
            </button>

            <!-- 标签管理按钮 -->
            <button
                class="btn-refresh"
                title="标签管理"
                @click="showTagManager = true"
            >
                🏷 标签
            </button>

            <!-- 设置按钮 -->
            <button
                class="btn-refresh"
                title="设置"
                @click="showSettings = true"
            >
                ⚙ 设置
            </button>

            <!-- 窗口控制按钮（无边框窗口自绘） -->
            <div class="window-controls">
                <button
                    class="win-btn win-min"
                    title="最小化"
                    @click="minimizeWindow"
                >
                    &#x2212;
                </button>
                <button
                    class="win-btn win-max"
                    title="最大化/还原"
                    @click="toggleMaximizeWindow"
                >
                    &#x25A1;
                </button>
                <button
                    class="win-btn win-close"
                    title="关闭"
                    @click="closeWindow"
                >
                    &#x2715;
                </button>
            </div>
        </header>

        <!-- 错误提示 -->
        <div v-if="error" class="error-bar" @click="error = ''">
            ⚠ {{ error }} <span class="dismiss">×</span>
        </div>

        <!-- 首页仪表盘 -->
        <Dashboard
            v-show="currentPage === 'home'"
            :nodes="nodes"
            :projects="projects"
            :visible="currentPage === 'home'"
            @jump-to-task="onJumpToTask"
        />

        <!-- 图表页 -->
        <ChartsPage
            v-show="currentPage === 'charts'"
            :nodes="nodes"
            :visible="currentPage === 'charts'"
            @jump-to-task="onJumpToTask"
        />

        <!-- 日历页 -->
        <CalendarPage
            v-show="currentPage === 'calendar'"
            :nodes="nodes"
            :visible="currentPage === 'calendar'"
            @jump-to-task="onJumpToTask"
        />

        <!-- 主体三栏（任务看板） -->
        <div v-show="currentPage === 'board'" class="main">
            <ProjectTree
                :projects="projects"
                :planned-roots="plannedProjectRoots"
                :active-roots="activeProjectRoots"
                :archived-roots="archivedProjectRoots"
                :trash-roots="trashProjectRoots"
                :selected="selectedProject"
                :today-count="todayCount"
                @select="selectedProject = $event"
                @create-project="onCreateProject"
                @toggle-archive="onToggleArchive"
                @set-stage="onSetStage"
                @trash-project="onTrashProject"
                @restore-project="onRestoreProject"
                @purge-project="onPurgeProject"
                @move-project="onMoveProject"
            />

            <TaskGraph
                :nodes="nodes"
                :edges="graphEdges"
                :mode="graphMode"
                :depends-edges="edges"
                :selected="selectedUUID"
                :highlight-set="highlightSet"
                :project-filter="selectedProject"
                :tag-filter="tagFilter"
                :projects="projects"
                :tags="tags"
                :multi-selected="multiSelectedUUIDs"
                :has-active-timer="activeTimingNodes.length > 0"
                :node-display="settings"
                :sibling-order-edges="siblingOrderEdges"
                @select="onGraphSelect"
                @toggle-multi-select="onToggleMultiSelect"
                @box-select="onBoxSelect"
                @clear-multi-select="onClearMultiSelect"
                @bulk-done="onBulkDone"
                @bulk-delete="onBulkDelete"
                @bulk-today="onBulkToday"
                @bulk-start-timer="onBulkStartTimer"
                @bulk-move-project="onBulkMoveProject"
                @clear-tag-filter="tagFilter = null"
                @connect-nodes="onConnectNodes"
                @reconnect-edge="onReconnectEdge"
                @reorder-siblings="onReorderSiblings"
            />

            <TaskDetail
                ref="taskDetailRef"
                :task="selectedTask"
                :all-tasks="nodes"
                :tags="tags"
                @done="onDone"
                @undone="onUndone"
                @start-timer="onStartTimer"
                @stop-timer="onStopTimer"
                @delete="onDelete"
                @modify="openModify"
                @select="selectedUUID = $event"
                @edit-time-entry-note="onEditTimeEntryNote"
                @delete-time-entry="onDeleteTimeEntry"
                @set-today="onSetToday"
                @update-annotation="onUpdateAnnotation"
                @filter-by-tag="onFilterByTag"
            />
        </div>

        <!-- 添加任务弹出框 -->
        <TaskFormModal
            :visible="showModal"
            :prefill="modalPrefill"
            :projects="projects"
            :default-project="defaultProjectForNewTask"
            :all-tasks="nodes"
            :tag-colors="tags"
            @close="showModal = false"
            @submit="onModalSubmit"
            @rename-tag="onRenameTag"
        />

        <!-- 设置弹出框 -->
        <SettingsModal
            :visible="showSettings"
            :settings="settings"
            :highlight-mode="hlMode"
            @close="showSettings = false"
            @save="onSaveSettings"
            @update:highlight-mode="hlMode = $event"
        />

        <!-- 标签管理弹出框 -->
        <TagManagerModal
            :visible="showTagManager"
            :tags="tags"
            @close="showTagManager = false"
            @rename="onRenameTag"
            @set-color="onSetTagColor"
            @delete-tag="onDeleteTag"
            @filter-by-tag="
                (name) => {
                    onFilterByTag(name);
                    showTagManager = false;
                }
            "
        />

        <!-- 计时记录回忆总结弹窗 -->
        <TimeEntryNoteModal
            :visible="noteModal.visible"
            :heading="noteModal.heading"
            :initial-title="noteModal.title"
            :initial-body="noteModal.body"
            @close="noteModal.visible = false"
            @save="onSaveTimeEntryNote"
        />

        <!-- 自绘确认弹窗：替代原生 confirm()，全局单例 -->
        <ConfirmDialog />
    </div>
</template>

<style scoped>
.app {
    display: flex;
    flex-direction: column;
    height: 100vh;
}

.topbar {
    position: relative;
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
    font-size: 1.1538rem;
    color: var(--cyan);
    margin-right: 8px;
}

/* 页面切换 */
.page-nav {
    display: flex;
    gap: 2px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 2px;
}
.page-nav-btn {
    padding: 4px 12px;
    border-radius: 5px;
    font-size: 0.8462rem;
    font-weight: 600;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.page-nav-btn:hover {
    color: var(--fg);
}
.page-nav-btn.active {
    color: var(--blue);
    background: var(--bg-panel);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
}

/* 标题栏悬浮秒表：当前活跃计时任务，悬浮在标题栏正中间，不占据两侧按钮的排列空间 */
.active-timer-pill {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 10px 3px 8px;
    border-radius: 999px;
    background: rgba(158, 206, 106, 0.12);
    border: 1px solid rgba(158, 206, 106, 0.35);
    max-width: 280px;
}

/* 批量计时鼠标悬浮时展开的任务列表 */
.active-timer-tasklist {
    display: none;
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    min-width: 220px;
    max-width: 320px;
    max-height: 240px;
    overflow-y: auto;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
    padding: 6px;
    z-index: 20;
}

.active-timer-pill-group:hover .active-timer-tasklist {
    display: block;
}

.active-timer-tasklist-item {
    padding: 5px 8px;
    font-size: 0.85rem;
    color: var(--fg);
    border-radius: 5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
}

.active-timer-tasklist-item:hover {
    background: var(--bg-select);
    color: var(--blue);
}

.active-timer-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--green);
    flex-shrink: 0;
    animation: active-timer-pulse 1.6s ease-in-out infinite;
}
@keyframes active-timer-pulse {
    0%,
    100% {
        opacity: 1;
    }
    50% {
        opacity: 0.35;
    }
}

.active-timer-desc {
    font-size: 0.9231rem;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
}
.active-timer-desc:hover {
    color: var(--green);
}

.active-timer-clock {
    font-size: 0.9231rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--green);
    flex-shrink: 0;
}

.active-timer-stop {
    font-size: 0.6923rem;
    color: var(--green);
    opacity: 0.7;
    flex-shrink: 0;
    padding: 2px;
    transition: opacity 0.15s;
}
.active-timer-stop:hover {
    opacity: 1;
}

.btn-add-toggle {
    /* 把自己和右边的刷新/设置按钮一起推到标题栏最右侧，紧挨窗口控制按钮左边 */
    margin-left: auto;
    padding: 4px 14px;
    border-radius: 6px;
    background: var(--blue);
    color: var(--bg);
    font-weight: 600;
    font-size: 0.9231rem;
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
    font-size: 0.9231rem;
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

/* 无边框窗口的自绘控制按钮 */
.window-controls {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-right: 6px;
}

.win-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition:
        background 0.12s,
        color 0.12s;
}

.win-btn:hover {
    background: rgba(0, 0, 0, 0.06);
    color: var(--fg);
}

.win-btn.win-close:hover {
    background: var(--red);
    color: #fff;
}

/* 错误提示栏 */
.error-bar {
    padding: 8px 16px;
    background: rgba(247, 118, 142, 0.15);
    color: var(--red);
    border-bottom: 1px solid var(--red);
    font-size: 0.9231rem;
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
