<!--
  @Component: 任务表单弹窗组件
  @Description: 用于新建和修改任务
  @Author: Bin.H
  @Date: 2026-05-23
-->

<script setup>
import constants from "../config/constants";
import { computed, ref, watch } from "vue";
import DatePicker from "./DatePicker.vue";

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
});

const emit = defineEmits([
    "close", // 关闭弹窗
    "submit", // 提交: { mode: 'add'|'modify', uuid?, fields }
]);

// 表单字段
const description = ref("");
const project = ref("");
const due = ref(""); // 格式: YYYY-MM-DD
const priority = ref(""); // H | M | L
const tags = ref([]);
const tagInput = ref(""); // 标签输入框临时值
const showTagDropdown = ref(false); // 是否显示标签下拉建议框
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

// 所有已存在的标签（从全部任务中去重收集）
const allTagOptions = computed(() => {
    const set = new Set();
    for (const t of props.allTasks) {
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

// 所有已存在的任务列表
const dependsOptions = computed(() =>
    props.allTasks.filter(
        (t) => t.status === "pending" && t.uuid != props.prefill?.uuid,
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
/** 回车、逗号确认添加标签，或从下拉框点击选择已有标签 */
function addTag(tag) {
    const t = (tag ?? tagInput.value).trim().replace(/^[+]/, "");
    if (t && !tags.value.includes(t)) {
        tags.value.push(t);
    }
    tagInput.value = "";
    showTagDropdown.value = false;
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
                    <div class="form-row">
                        <label class="form-label">项目</label>
                        <input
                            v-model="project"
                            class="form-input"
                            list="project-list"
                            placeholder="选择或输入项目路径，如 personal.reading"
                        />

                        <datalist id="project-list">
                            <option
                                v-for="p in projectOptions"
                                :key="p"
                                :value="p"
                            />
                        </datalist>
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
                                @focus="showTagDropdown = true"
                                @blur="hideTagDropdownDelayed"
                                @keydown.enter.prevent="addTag()"
                                @keydown.comma.prevent="addTag()"
                                @keydown.down.prevent="
                                    showTagDropdown = true
                                "
                                @keydown.delete="removeLastTagOnBackspace"
                                placeholder="输入标签，回车确认或从下拉选择"
                            />
                        </div>

                        <!-- 已有标签下拉建议框 -->
                        <div
                            v-if="
                                showTagDropdown &&
                                filteredTagOptions.length > 0
                            "
                            class="tag-dropdown"
                        >
                            <button
                                v-for="opt in filteredTagOptions"
                                :key="opt"
                                type="button"
                                class="tag-dropdown-item"
                                @mousedown.prevent="addTag(opt)"
                            >
                                {{ opt }}
                            </button>
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
.tag-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 10;
    max-height: 160px;
    overflow-y: auto;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.tag-dropdown-item {
    text-align: left;
    padding: 6px 8px;
    border-radius: 4px;
    font-size: 0.9231rem;
    color: var(--fg);
    transition: background 0.12s;
}
.tag-dropdown-item:hover {
    background: var(--bg-select);
    color: var(--magenta);
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
