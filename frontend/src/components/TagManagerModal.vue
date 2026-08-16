<!--
  @Component: 标签管理弹窗
  @Description:
    列出所有标签，支持改名（用了这个标签的任务一起改名）、改颜色、
    删除（解除和所有任务的关联），点击标签名可以跳转到任务看板并按它筛选。
  @Author: Bin.H
-->

<script setup>
import { nextTick, ref, watch } from "vue";
import { tagChipStyle } from "../composables/useTagColor";

const props = defineProps({
    visible: { type: Boolean, required: true },
    tags: { type: Object, required: true }, // 标签名 -> { name, color, task_count }
});

const emit = defineEmits([
    "close",
    "rename", // (oldName, newName)
    "set-color", // (name, color)
    "delete-tag", // (name)
    "filter-by-tag", // (name)
]);

// 按名字排序展示
const sortedTags = ref([]);
watch(
    () => props.tags,
    (tags) => {
        sortedTags.value = Object.values(tags).sort((a, b) =>
            a.name.localeCompare(b.name),
        );
    },
    { immediate: true, deep: true },
);

// ----------------------------------------
// 改名：点铅笔图标，该行原地变成输入框
// ----------------------------------------
const renamingTag = ref(null);
const renameInput = ref("");
const renameInputRef = ref(null);

watch(renamingTag, async (tag) => {
    if (!tag) return;
    await nextTick();
    renameInputRef.value?.focus();
    renameInputRef.value?.select();
});

function startRename(name) {
    renamingTag.value = name;
    renameInput.value = name;
}

function cancelRename() {
    renamingTag.value = null;
    renameInput.value = "";
}

function confirmRename() {
    const oldName = renamingTag.value;
    const newName = renameInput.value.trim();
    cancelRename();

    if (!oldName || !newName || newName === oldName) return;
    emit("rename", oldName, newName);
}

// ----------------------------------------
// 颜色：原生颜色选择器，选完立刻生效
// ----------------------------------------
function onColorInput(name, event) {
    emit("set-color", name, event.target.value);
}

function clearColor(name) {
    emit("set-color", name, null);
}

function confirmDelete(name) {
    emit("delete-tag", name);
}
</script>

<template>
    <Teleport to="body">
        <div v-if="visible" class="modal-overlay" @click.self="emit('close')">
            <div class="modal">
                <div class="modal-header">
                    <span class="modal-title">标签管理</span>
                    <button class="modal-close" @click="emit('close')">
                        ×
                    </button>
                </div>

                <div class="modal-body">
                    <div v-if="sortedTags.length === 0" class="empty-hint">
                        还没有任何标签，在任务表单里添加标签后会出现在这里
                    </div>

                    <div v-else class="tag-list">
                        <div
                            v-for="t in sortedTags"
                            :key="t.name"
                            class="tag-row"
                        >
                            <label
                                class="tag-color-swatch"
                                :style="{
                                    background: t.color || 'var(--fg-dark)',
                                }"
                                title="点击选择颜色"
                            >
                                <input
                                    type="color"
                                    :value="t.color || '#8250df'"
                                    @input="onColorInput(t.name, $event)"
                                />
                            </label>

                            <button
                                v-if="t.color"
                                class="tag-color-clear"
                                title="清除颜色，恢复默认"
                                @click="clearColor(t.name)"
                            >
                                ✕
                            </button>

                            <input
                                v-if="renamingTag === t.name"
                                :ref="(el) => (renameInputRef = el)"
                                v-model="renameInput"
                                class="tag-rename-input"
                                @keydown.enter.prevent="confirmRename"
                                @keydown.esc.prevent="cancelRename"
                                @blur="cancelRename"
                            />
                            <button
                                v-else
                                class="tag-name-btn"
                                :style="tagChipStyle(t.color)"
                                title="点击按这个标签筛选任务看板"
                                @click="emit('filter-by-tag', t.name)"
                            >
                                {{ t.name }}
                            </button>

                            <span class="tag-count">{{ t.task_count }} 个任务</span>

                            <button
                                class="tag-row-btn"
                                title="重命名（用了这个标签的任务会一起改名）"
                                @click="startRename(t.name)"
                            >
                                ✎
                            </button>
                            <button
                                class="tag-row-btn tag-row-btn-danger"
                                title="删除标签（从所有任务上移除）"
                                @click="confirmDelete(t.name)"
                            >
                                🗑
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </Teleport>
</template>

<style scoped>
.modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

.modal {
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 10px;
    width: 480px;
    max-width: 90vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
}

.modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
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

.modal-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 14px;
}

.empty-hint {
    padding: 20px 8px;
    text-align: center;
    color: var(--fg-dim);
    font-size: 0.9231rem;
}

.tag-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.tag-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 8px;
    transition: background 0.12s;
}
.tag-row:hover {
    background: var(--bg-select);
}

.tag-color-swatch {
    position: relative;
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
}
.tag-color-swatch input[type="color"] {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
}

.tag-color-clear {
    flex-shrink: 0;
    font-size: 0.6923rem;
    color: var(--fg-dim);
    opacity: 0;
    transition: opacity 0.12s;
}
.tag-row:hover .tag-color-clear {
    opacity: 1;
}
.tag-color-clear:hover {
    color: var(--red);
}

.tag-name-btn {
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 0.9231rem;
    font-weight: 600;
    transition: filter 0.12s;
}
.tag-name-btn:hover {
    filter: brightness(0.92);
}

.tag-rename-input {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--magenta);
    border-radius: 6px;
    padding: 4px 9px;
    font-size: 0.9231rem;
    background: var(--bg-dark);
    color: var(--fg);
    outline: none;
}

.tag-count {
    flex: 1;
    text-align: right;
    font-size: 0.8462rem;
    color: var(--fg-dim);
    white-space: nowrap;
}

.tag-row-btn {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    font-size: 0.8rem;
    color: var(--fg-dim);
    opacity: 0;
    transition:
        opacity 0.12s,
        background 0.12s,
        color 0.12s;
}
.tag-row:hover .tag-row-btn {
    opacity: 1;
}
.tag-row-btn:hover {
    background: var(--bg-dark);
    color: var(--magenta);
}
.tag-row-btn-danger:hover {
    color: var(--red);
}
</style>
