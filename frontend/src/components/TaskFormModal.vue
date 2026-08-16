<!--
  @Component: 任务表单弹窗组件
  @Description: 用于新建和修改任务
  @Author: Bin.H
  @Date: 2026-05-23
-->

<script setup>
import constants from "../config/constants";
import { computed, nextTick, ref, watch } from "vue";
import DatePicker from "./DatePicker.vue";
import { tagChipStyle } from "../composables/useTagColor";

const props = defineProps({
    // 是否显示弹窗
    visible: { type: Boolean, required: true },

    // 预填数据
    prefill: { type: Object, default: null },

    // 已有项目列表
    projects: { type: Object, required: true },

    // 当前选中的项目路径，新建时作为默认值
    defaultProject: { type: String, default: null },

    // 所有任务列表，用于前置任务选择
    allTasks: { type: Array, required: true },

    // 标签名 -> { name, color, task_count }，用于给标签块上色
    // （命名成 tagColors 而不是 tags，避免和下面本地的"当前任务已选标签"数组撞名）
    tagColors: { type: Object, default: () => ({}) },
});

const emit = defineEmits([
    "close", // 关闭弹窗
    "submit", // 提交: { mode: 'add'|'modify', uuid?, fields }
    "rename-tag", // 重命名标签：(oldTag, newTag)，用了旧标签的任务全部一起改名
]);

// 表单字段
const description = ref("");
const project = ref("");
const due = ref(""); // 格式: YYYY-MM-DD
const priority = ref(""); // H | M | L
const tags = ref([]);
const tagInput = ref(""); // 标签输入框临时值
const showTagDropdown = ref(false); // 是否显示标签下拉建议框
const showProjectDropdown = ref(false); // 是否显示项目下拉建议框
const depends = ref([]); // 任务的uuid
const annotationInput = ref(""); // 备注输入框：只保留一条，修改时会整体替换原有内容

// 根据模式设置表单标题
const isModify = computed(() => !!props.prefill);
const title = computed(() => (isModify.value ? "修改任务" : "添加任务"));

// 所有已存在的项目列表
const projectOptions = computed(() =>
    Object.keys(props.projects)
        .filter((p) => p != constants.INBOX_PROJECT)
        .sort(),
);

// 下拉框中显示的候选项目：随输入过滤（子串匹配），不区分大小写
const filteredProjectOptions = computed(() => {
    const keyword = project.value.trim().toLowerCase();
    if (!keyword) return projectOptions.value;
    return projectOptions.value.filter((p) =>
        p.toLowerCase().includes(keyword),
    );
});

// 项目下拉框当前键盘高亮的候选项下标，-1 表示未高亮任何项
const highlightedProjectIndex = ref(-1);

watch(filteredProjectOptions, () => {
    highlightedProjectIndex.value = -1;
});

// 所有已存在的标签：限定在当前选择的项目内去重收集；未选择项目时显示全部任务的标签
const allTagOptions = computed(() => {
    const set = new Set();
    const scoped = project.value
        ? props.allTasks.filter((t) => t.project === project.value)
        : props.allTasks;
    for (const t of scoped) {
        for (const tag of t.tags || []) set.add(tag);
    }
    return [...set].sort();
});

// 下拉框中显示的候选标签：排除已选，随输入过滤
const filteredTagOptions = computed(() => {
    const keyword = tagInput.value.trim().toLowerCase();
    return allTagOptions.value.filter((t) => {
        if (tags.value.includes(t)) return false;
        return !keyword || t.toLowerCase().includes(keyword);
    });
});

// 标签下拉框当前键盘高亮的候选项下标，-1 表示未高亮任何项
const highlightedTagIndex = ref(-1);

// 候选列表变化后（输入过滤/下拉开关）之前高亮的下标可能已经对不上，统一重置
watch(filteredTagOptions, () => {
    highlightedTagIndex.value = -1;
});

// 前置任务候选：限定在当前选择的项目内的待办任务；未选择项目时显示全部待办任务
const dependsOptions = computed(() =>
    props.allTasks.filter(
        (t) =>
            t.status === "pending" &&
            t.uuid != props.prefill?.uuid &&
            (!project.value || t.project === project.value),
    ),
);

// 预填写 (如果是修改任务的话)
watch(
    () => props.visible,
    (newVisible) => {
        if (!newVisible) return;
        if (props.prefill) {
            // 修改模式，填写原有的值
            description.value = props.prefill.description || "";
            project.value = props.prefill.project || "";
            due.value = props.prefill.due ? props.prefill.due.slice(0, 10) : ""; // 截断长时间格式，只保留日期
            priority.value = props.prefill.priority || "";
            tags.value = [...(props.prefill.tags || [])]; // ...操作符代表把(数组内)的元素放入外部[新数组]内
            depends.value = [...(props.prefill.depends || [])];
            // 备注只保留一条，预填现有内容，修改后整体替换
            annotationInput.value = props.prefill.annotations?.[0]?.description || "";
        } else {
            // 新建模式，清空
            description.value = "";

            // 项目默认选中当前项目
            project.value =
                props.defaultProject &&
                props.defaultProject !== constants.INBOX_PROJECT
                    ? props.defaultProject
                    : "";

            due.value = "";
            priority.value = "";
            tags.value = [];
            depends.value = [];
            annotationInput.value = "";
        }

        tagInput.value = "";
    },
);

// ----------------------------------------
// 标签操作
// ----------------------------------------
/** 回车、逗号确认添加标签，或从下拉框点击/键盘选择已有标签 */
function addTag(tag) {
    const t = (tag ?? tagInput.value).trim().replace(/^[+]/, "");
    if (t && !tags.value.includes(t)) {
        tags.value.push(t);
    }
    tagInput.value = "";
    showTagDropdown.value = false;
    highlightedTagIndex.value = -1;
}

function removeTag(tag) {
    tags.value = tags.value.filter((t) => t !== tag);
}

/** 输入框为空时按退格键，快速删除最后一个已输入的标签 */
function removeLastTagOnBackspace() {
    if (tagInput.value) return;
    tags.value = tags.value.slice(0, -1);
}

/** 输入框失焦时延迟隐藏下拉框，使下拉项的点击事件能先触发 */
function hideTagDropdownDelayed() {
    setTimeout(() => {
        showTagDropdown.value = false;
    }, 150);
}

/** ↓ 键：打开下拉框，并把键盘高亮移到下一项（到底后回到第一项） */
function moveTagHighlightDown() {
    showTagDropdown.value = true;
    const count = filteredTagOptions.value.length;
    if (count === 0) return;
    highlightedTagIndex.value = (highlightedTagIndex.value + 1) % count;
}

/** ↑ 键：把键盘高亮移到上一项（到顶后回到最后一项） */
function moveTagHighlightUp() {
    const count = filteredTagOptions.value.length;
    if (!showTagDropdown.value || count === 0) return;
    highlightedTagIndex.value = (highlightedTagIndex.value - 1 + count) % count;
}

/** 回车：如果下拉框当前有键盘高亮的候选项，选中它；否则按原逻辑把输入框内容确认为新标签 */
function confirmTagOnEnter() {
    const opt = filteredTagOptions.value[highlightedTagIndex.value];
    if (showTagDropdown.value && opt) {
        addTag(opt);
    } else {
        addTag();
    }
}

// ----------------------------------------
// 标签重命名：下拉框每一项悬浮时可点铅笔图标改名，
// 用了这个标签的所有任务会一起改成新名字（由父组件调用后端批量重命名）
// ----------------------------------------
const renamingTag = ref(null); // 当前正在改名的标签，null 表示没有
const renameTagInput = ref("");
const renameTagInputRef = ref(null);

watch(renamingTag, async (tag) => {
    if (!tag) return;
    await nextTick();
    renameTagInputRef.value?.focus();
    renameTagInputRef.value?.select();
});

function startRenameTag(tag) {
    renamingTag.value = tag;
    renameTagInput.value = tag;
}

function cancelRenameTag() {
    renamingTag.value = null;
    renameTagInput.value = "";
}

/** 回车确认改名：通知父组件批量重命名，并同步当前表单里已选的这个标签 */
function confirmRenameTag() {
    const oldTag = renamingTag.value;
    const newTag = renameTagInput.value.trim();
    cancelRenameTag();

    if (!oldTag || !newTag || newTag === oldTag) return;

    emit("rename-tag", oldTag, newTag);

    // 去重，避免这个任务本来就同时有新旧两个标签名，改名后出现重复
    tags.value = [
        ...new Set(tags.value.map((t) => (t === oldTag ? newTag : t))),
    ];
}

// ----------------------------------------
// 项目选择：自定义下拉框（用输入过滤 + 键盘/鼠标选择），
// 不用原生 <input list>+<datalist>，因为浏览器原生的候选框样式无法自定义
// ----------------------------------------
function selectProject(p) {
    project.value = p;
    showProjectDropdown.value = false;
    highlightedProjectIndex.value = -1;
}

/** 输入框失焦时延迟隐藏下拉框，使下拉项的点击事件能先触发 */
function hideProjectDropdownDelayed() {
    setTimeout(() => {
        showProjectDropdown.value = false;
    }, 150);
}

/** ↓ 键：打开下拉框，并把键盘高亮移到下一项（到底后回到第一项） */
function moveProjectHighlightDown() {
    showProjectDropdown.value = true;
    const count = filteredProjectOptions.value.length;
    if (count === 0) return;
    highlightedProjectIndex.value = (highlightedProjectIndex.value + 1) % count;
}

/** ↑ 键：把键盘高亮移到上一项（到顶后回到最后一项） */
function moveProjectHighlightUp() {
    const count = filteredProjectOptions.value.length;
    if (!showProjectDropdown.value || count === 0) return;
    highlightedProjectIndex.value =
        (highlightedProjectIndex.value - 1 + count) % count;
}

/** 回车：如果下拉框当前有键盘高亮的候选项，选中它并收起下拉框；否则直接保留手动输入的路径 */
function confirmProjectOnEnter() {
    const opt = filteredProjectOptions.value[highlightedProjectIndex.value];
    if (showProjectDropdown.value && opt) {
        selectProject(opt);
    } else {
        showProjectDropdown.value = false;
    }
}

// ----------------------------------------
// 前置任务
// ----------------------------------------
function toggleDepend(uuid) {
    if (depends.value.includes(uuid)) {
        depends.value = depends.value.filter((u) => u !== uuid);
    } else {
        depends.value.push(uuid);
    }
}

// ----------------------------------------
// 提交命令
// ----------------------------------------
/**
 * 将表单字段整理为结构化对象并提交，直接对应后端
 * AddTaskArgs / ModifyTaskArgs 的字段形状。
 *
 * 修改模式下若某字段被清空，用 clear_* 标志告知后端删除该字段。
 */
function submit() {
    if (!description.value.trim()) return;

    if (isModify.value) {
        const fields = {
            tags: tags.value,
            depends: depends.value,
            annotation: annotationInput.value.trim() || null,
            clear_annotation: !annotationInput.value.trim(),
        };

        if (description.value !== props.prefill.description) {
            fields.description = description.value;
        }

        if (project.value) {
            fields.project = project.value;
        } else if (props.prefill.project) {
            fields.clear_project = true;
        }

        if (due.value) {
            fields.due = due.value;
        } else if (props.prefill.due) {
            fields.clear_due = true;
        }

        if (priority.value) {
            fields.priority = priority.value;
        } else if (props.prefill.priority) {
            fields.clear_priority = true;
        }

        emit("submit", {
            mode: "modify",
            uuid: props.prefill.uuid,
            fields,
        });
    } else {
        emit("submit", {
            mode: "add",
            fields: {
                description: description.value,
                project: project.value || null,
                due: due.value || null,
                priority: priority.value || null,
                scheduled: null,
                tags: tags.value,
                depends: depends.value,
                annotation: annotationInput.value.trim() || null,
            },
        });
    }
}
</script>

<template>
    <!-- 遮罩层，点击遮罩关闭弹窗 -->
    <Teleport to="body">
        <div v-if="visible" class="modal-overlay" @click.self="emit('close')">
            <div class="modal">
                <!-- 标题栏 -->
                <div class="modal-header">
                    <span class="modal-title">{{ title }}</span>
                    <button class="modal-close" @click="emit('close')">
                        ×
                    </button>
                </div>

                <!-- 表单内容 -->
                <div class="modal-body">
                    <!-- 任务描述 -->
                    <div class="form-row">
                        <label class="form-label">
                            <span>任务描述 <span class="required">*</span></span>
                        </label>
                        <input
                            ref="inputRef"
                            v-model="description"
                            class="form-input"
                            @keydown.enter="emit('submit')"
                        />
                    </div>

                    <!-- 所属项目 (下拉框选择 + 可手动输入) -->
                    <div class="form-row project-field-row">
                        <label class="form-label">项目</label>
                        <input
                            v-model="project"
                            class="form-input"
                            placeholder="选择或输入项目路径，如 personal.reading"
                            @focus="
                                showProjectDropdown = true;
                                highlightedProjectIndex = -1;
                            "
                            @blur="hideProjectDropdownDelayed"
                            @keydown.enter.prevent="confirmProjectOnEnter"
                            @keydown.down.prevent="moveProjectHighlightDown"
                            @keydown.up.prevent="moveProjectHighlightUp"
                        />

                        <!-- 已有项目下拉建议框 -->
                        <div
                            v-if="
                                showProjectDropdown &&
                                filteredProjectOptions.length > 0
                            "
                            class="suggest-dropdown"
                        >
                            <button
                                v-for="(p, i) in filteredProjectOptions"
                                :key="p"
                                type="button"
                                class="suggest-dropdown-item"
                                :class="{ active: i === highlightedProjectIndex }"
                                @mousedown.prevent="selectProject(p)"
                                @mouseenter="highlightedProjectIndex = i"
                            >
                                {{ p }}
                            </button>
                        </div>
                    </div>

                    <!-- 截止日期 -->
                    <div class="form-row">
                        <label class="form-label">截止日期</label>
                        <DatePicker v-model="due" />
                    </div>

                    <!-- 优先级 -->
                    <div class="form-row">
                        <label class="form-label">优先级</label>
                        <div class="priority-group">
                            <button
                                v-for="p in ['H', 'M', 'L']"
                                class="priority-btn"
                                :key="p"
                                @click="priority = p"
                                :class="{
                                    active: priority === p,
                                    'priority-h': p === 'H',
                                    'priority-m': p === 'M',
                                    'priority-l': p === 'L',
                                }"
                            >
                                {{ p }}
                            </button>
                        </div>
                    </div>

                    <!-- 标签 -->
                    <div class="form-row tags-row">
                        <label class="form-label">标签</label>
                        <div class="tags-editor">
                            <!-- 已存在的标签 -->
                            <span
                                v-for="tag in tags"
                                :key="tag"
                                class="tag-chip"
                                :style="tagChipStyle(tagColors[tag]?.color)"
                            >
                                {{ tag }}
                                <button
                                    class="tag-remove"
                                    @click="removeTag(tag)"
                                >
                                    ×
                                </button>
                            </span>

                            <!-- 输入新标签，随输入过滤下拉建议 -->
                            <input
                                class="tag-input"
                                v-model="tagInput"
                                @focus="
                                    showTagDropdown = true;
                                    highlightedTagIndex = -1;
                                "
                                @blur="hideTagDropdownDelayed"
                                @keydown.enter.prevent="confirmTagOnEnter"
                                @keydown.comma.prevent="addTag()"
                                @keydown.down.prevent="moveTagHighlightDown"
                                @keydown.up.prevent="moveTagHighlightUp"
                                @keydown.delete="removeLastTagOnBackspace"
                                placeholder="输入标签，↑↓ 选择、回车确认，或从下拉选择"
                            />
                        </div>

                        <!-- 已有标签下拉建议框：悬浮某一项可点铅笔图标重命名，
                             改名期间即使标签输入框失焦也要保持下拉框展开（见 v-if 的 renamingTag 分支） -->
                        <div
                            v-if="
                                (showTagDropdown || renamingTag) &&
                                filteredTagOptions.length > 0
                            "
                            class="suggest-dropdown"
                        >
                            <div
                                v-for="(opt, i) in filteredTagOptions"
                                :key="opt"
                                class="suggest-dropdown-row"
                                @mouseenter="highlightedTagIndex = i"
                            >
                                <input
                                    v-if="renamingTag === opt"
                                    :ref="(el) => (renameTagInputRef = el)"
                                    v-model="renameTagInput"
                                    class="tag-rename-input"
                                    @keydown.enter.prevent="confirmRenameTag"
                                    @keydown.esc.prevent="cancelRenameTag"
                                    @blur="cancelRenameTag"
                                />
                                <template v-else>
                                    <button
                                        type="button"
                                        class="suggest-dropdown-item"
                                        :class="{
                                            active: i === highlightedTagIndex,
                                        }"
                                        @mousedown.prevent="addTag(opt)"
                                    >
                                        {{ opt }}
                                    </button>
                                    <button
                                        type="button"
                                        class="tag-rename-btn"
                                        title="重命名标签（用了这个标签的所有任务会一起改名）"
                                        @mousedown.prevent="startRenameTag(opt)"
                                    >
                                        ✎
                                    </button>
                                </template>
                            </div>
                        </div>
                    </div>

                    <!-- 前置任务 (可多选) -->
                    <div class="form-row">
                        <label class="form-label">前置任务</label>
                        <div class="depends-list">
                            <label
                                v-for="task in dependsOptions"
                                :key="task.uuid"
                                class="depends-item"
                                :class="{
                                    selected: depends.includes(task.uuid),
                                }"
                            >
                                <input
                                    type="checkbox"
                                    :checked="depends.includes(task.uuid)"
                                    @change="toggleDepend(task.uuid)"
                                />
                                <span class="depends-desc">
                                    {{ task.description }}
                                </span>
                                <span
                                    v-if="task.project"
                                    class="depends-project"
                                >
                                    {{ task.project.split(".").pop() }}
                                </span>
                            </label>

                            <span
                                v-if="dependsOptions.length === 0"
                                class="empty-hint"
                            >
                                暂无可选的待办任务
                            </span>
                        </div>
                    </div>

                    <!-- 备注：只保留一条，每次修改都是整体替换 -->
                    <div class="form-row">
                        <label class="form-label">
                            <span>备注</span>
                        </label>
                        <textarea
                            v-model="annotationInput"
                            class="form-textarea"
                            rows="3"
                            placeholder="记录一些补充说明…"
                        ></textarea>
                    </div>
                </div>

                <!-- 底部按钮 -->
                <div class="modal-footer">
                    <button class="btn-submit" @click="emit('close')">
                        取消
                    </button>
                    <button
                        class="btn-submit"
                        @click="submit"
                        :disabled="!description.trim()"
                    >
                        {{ isModify ? "保存修改" : "添加任务" }}
                    </button>
                </div>
            </div>
        </div>
    </Teleport>
</template>

<style scoped>
/* 遮罩层 */
.modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

/* 弹窗容器 */
.modal {
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 10px;
    width: 520px;
    max-width: 90vw;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
}

/* 标题栏 */
.modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
}
.modal-title {
    font-size: 1.1538rem;
    font-weight: 700;
    color: var(--cyan);
}
.modal-close {
    font-size: 1.5385rem;
    color: var(--fg-dim);
    line-height: 1;
    padding: 0 4px;
    border-radius: 4px;
    transition: color 0.15s;
}
.modal-close:hover {
    color: var(--fg);
}

/* 表单区 */
.modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
}
.modal-body::-webkit-scrollbar {
    width: 4px;
}
.modal-body::-webkit-scrollbar-thumb {
    background: var(--fg-dark);
    border-radius: 2px;
}

.form-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
}
.form-label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 0.8462rem;
    font-weight: 700;
    color: var(--fg-dim);
    text-transform: uppercase;
    letter-spacing: 0.05em;
}
.form-hint {
    font-size: 0.8462rem;
    font-weight: 400;
    text-transform: none;
    letter-spacing: normal;
    color: var(--fg-dark);
}
.required {
    color: var(--red);
}
.form-input {
    width: 100%;
}
.form-textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
    color: var(--fg);
    background: var(--bg-dark);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 10px;
    outline: none;
    line-height: 1.5;
}
.form-textarea:focus {
    border-color: var(--blue);
}

/* 优先级单选组 */
.priority-group {
    display: flex;
    gap: 6px;
}
.priority-btn {
    padding: 5px 16px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 0.9231rem;
    font-weight: 600;
    color: var(--fg-dim);
    transition: all 0.15s;
}
.priority-btn:hover {
    border-color: var(--fg-dark);
    color: var(--fg);
}
.priority-btn.active.priority-h {
    background: rgba(247, 118, 142, 0.2);
    color: var(--red);
    border-color: var(--red);
}
.priority-btn.active.priority-m {
    background: rgba(224, 175, 104, 0.2);
    color: var(--yellow);
    border-color: var(--yellow);
}
.priority-btn.active.priority-l {
    background: rgba(122, 162, 247, 0.2);
    color: var(--blue);
    border-color: var(--blue);
}
.priority-btn.active.priority-none {
    background: rgba(0, 0, 0, 0.08);
    color: var(--fg);
    border-color: var(--fg-dark);
}

/* 项目选择：自定义下拉框需要一个定位锚点 */
.project-field-row {
    position: relative;
}

/* 标签编辑器 */
.tags-row {
    position: relative;
}
.tags-editor {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    padding: 6px 8px;
    background: var(--bg-dark);
    border: 1px solid var(--border);
    border-radius: 6px;
    min-height: 36px;
}
.tags-editor:focus-within {
    border-color: var(--blue);
}

.tag-chip {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 4px;
    background: rgba(187, 154, 247, 0.15);
    color: var(--magenta);
    font-size: 0.9231rem;
}
.tag-remove {
    font-size: 1.0769rem;
    line-height: 1;
    color: var(--fg-dim);
    padding: 0 2px;
}
.tag-remove:hover {
    color: var(--red);
}

.tag-input {
    border: none;
    background: transparent;
    outline: none;
    padding: 0;
    font-size: 0.9231rem;
    color: var(--fg);
    min-width: 100px;
    flex: 1;
}

/* 标签下拉建议框 */
.suggest-dropdown {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 10;
    max-height: 180px;
    overflow-y: auto;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow:
        0 10px 28px rgba(0, 0, 0, 0.22),
        0 2px 6px rgba(0, 0, 0, 0.12);
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: suggest-dropdown-in 0.12s ease-out;
}

@keyframes suggest-dropdown-in {
    from {
        opacity: 0;
        transform: translateY(-4px);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}

.suggest-dropdown::-webkit-scrollbar {
    width: 4px;
}
.suggest-dropdown::-webkit-scrollbar-thumb {
    background: var(--fg-dark);
    border-radius: 2px;
}

.suggest-dropdown-item {
    text-align: left;
    padding: 6px 10px;
    border-radius: 5px;
    font-size: 0.9231rem;
    color: var(--fg);
    transition:
        background 0.12s,
        color 0.12s;
}
.suggest-dropdown-item:hover {
    background: var(--bg-select);
    color: var(--magenta);
}

/* 键盘 ↑↓ 移动到的候选项：比鼠标悬浮更醒目，回车即会选中它 */
.suggest-dropdown-item.active {
    background: rgba(187, 154, 247, 0.18);
    color: var(--magenta);
    box-shadow: inset 0 0 0 1px rgba(187, 154, 247, 0.4);
}

/* 标签候选项一行：候选按钮 + 悬浮才出现的重命名图标 */
.suggest-dropdown-row {
    display: flex;
    align-items: center;
    gap: 2px;
}
.suggest-dropdown-row .suggest-dropdown-item {
    flex: 1;
    min-width: 0;
}

.tag-rename-btn {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 5px;
    font-size: 0.8rem;
    color: var(--fg-dim);
    opacity: 0;
    transition:
        opacity 0.12s,
        background 0.12s,
        color 0.12s;
}
.suggest-dropdown-row:hover .tag-rename-btn {
    opacity: 1;
}
.tag-rename-btn:hover {
    background: var(--bg-select);
    color: var(--magenta);
}

/* 标签重命名输入框：替换掉当前这一行 */
.tag-rename-input {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--magenta);
    border-radius: 5px;
    padding: 5px 9px;
    font-size: 0.9231rem;
    background: var(--bg-dark);
    color: var(--fg);
    outline: none;
}

/* 前置任务多选 */
.depends-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 160px;
    overflow-y: auto;
    padding: 4px;
    background: var(--bg-dark);
    border: 1px solid var(--border);
    border-radius: 6px;
}
.depends-list::-webkit-scrollbar {
    width: 4px;
}
.depends-list::-webkit-scrollbar-thumb {
    background: var(--fg-dark);
    border-radius: 2px;
}

.depends-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 5px;
    cursor: pointer;
    transition: background 0.12s;
    font-size: 0.9231rem;
}
.depends-item:hover {
    background: rgba(0, 0, 0, 0.05);
}
.depends-item.selected {
    background: var(--bg-select);
}
.depends-item input {
    accent-color: var(--blue);
    cursor: pointer;
}
.depends-desc {
    flex: 1;
    color: var(--fg);
}
.depends-project {
    color: var(--fg-dim);
    font-size: 0.8462rem;
}

.empty-hint {
    color: var(--fg-dim);
    font-size: 0.9231rem;
    padding: 8px;
}

/* 底部按钮 */
.modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 14px 20px;
    border-top: 1px solid var(--border);
}
.btn-submit {
    padding: 7px 20px;
    border-radius: 6px;
    background: var(--blue);
    color: var(--bg);
    font-weight: 700;
    font-size: 1rem;
    transition: opacity 0.15s;
}
.btn-submit:hover {
    opacity: 0.85;
}
.btn-submit:disabled {
    opacity: 0.35;
    cursor: default;
}
.btn-cancel {
    padding: 7px 16px;
    border-radius: 6px;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    font-size: 1rem;
    transition: all 0.15s;
}
.btn-cancel:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}
</style>
