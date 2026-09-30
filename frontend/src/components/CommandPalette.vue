<!--
  @Component: 命令面板
  @Description: Ctrl+K 打开，列出全部快捷键 action，按标题子串过滤，
    上下键选择、Enter 执行、Esc 关闭（Esc 由全局快捷键系统统一处理，见 useShortcuts.js）。
  @Author: Bin.H
-->

<script setup>
import { ref, computed, nextTick, watch } from "vue";
import { effectiveBinding, formatBindingForDisplay } from "../composables/useShortcuts";

const props = defineProps({
    visible: { type: Boolean, required: true },
    actions: { type: Array, required: true },
    shortcuts: { type: Object, default: () => ({}) },
});
const emit = defineEmits(["close"]);

const query = ref("");
const activeIndex = ref(0);
const inputRef = ref(null);

const filtered = computed(() => {
    const q = query.value.trim().toLowerCase();
    if (!q) return props.actions;
    return props.actions.filter((a) => a.label.toLowerCase().includes(q));
});

watch(
    () => props.visible,
    async (visible) => {
        if (!visible) return;
        query.value = "";
        activeIndex.value = 0;
        await nextTick();
        inputRef.value?.focus();
    },
);

watch(filtered, () => {
    activeIndex.value = 0;
});

function bindingLabel(action) {
    return formatBindingForDisplay(effectiveBinding(action, props.shortcuts));
}

function runAt(index) {
    const action = filtered.value[index];
    if (!action) return;
    emit("close");
    action.run();
}

function onKeydown(e) {
    if (e.key === "ArrowDown") {
        e.preventDefault();
        activeIndex.value = Math.min(activeIndex.value + 1, filtered.value.length - 1);
    } else if (e.key === "ArrowUp") {
        e.preventDefault();
        activeIndex.value = Math.max(activeIndex.value - 1, 0);
    } else if (e.key === "Enter") {
        e.preventDefault();
        runAt(activeIndex.value);
    }
}
</script>

<template>
    <Teleport to="body">
        <div v-if="visible" class="palette-overlay" @click.self="emit('close')">
            <div class="palette">
                <input
                    ref="inputRef"
                    v-model="query"
                    class="palette-input"
                    placeholder="搜索命令…"
                    @keydown="onKeydown"
                />
                <ul class="palette-list">
                    <li
                        v-for="(action, i) in filtered"
                        :key="action.id"
                        class="palette-item"
                        :class="{ active: i === activeIndex }"
                        @mouseenter="activeIndex = i"
                        @click="runAt(i)"
                    >
                        <span>{{ action.label }}</span>
                        <kbd class="palette-binding">{{ bindingLabel(action) }}</kbd>
                    </li>
                    <li v-if="filtered.length === 0" class="palette-empty">
                        没有匹配的命令
                    </li>
                </ul>
            </div>
        </div>
    </Teleport>
</template>

<style scoped>
.palette-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    z-index: 2100;
}

.palette {
    width: 480px;
    max-width: 90vw;
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--elevation-3);
    overflow: hidden;
}

.palette-input {
    border: none;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    padding: 12px 16px;
    font-size: 1rem;
    background: transparent;
    color: var(--fg);
}
.palette-input:focus {
    outline: none;
}

.palette-list {
    overflow-y: auto;
    padding: 6px;
}

.palette-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    cursor: pointer;
}
.palette-item.active,
.palette-item:hover {
    background: var(--bg-select);
}

.palette-binding {
    font-size: 0.75rem;
    color: var(--fg-dim);
    background: var(--bg-dark);
    padding: 2px 6px;
    border-radius: var(--radius-sm);
}

.palette-empty {
    padding: 16px;
    text-align: center;
    color: var(--fg-dim);
    font-size: 0.9rem;
}
</style>
