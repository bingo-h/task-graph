<!--
  @Component: 快捷键帮助面板
  @Description: ? 或 F1 打开，只读展示全部快捷键 action 按分类分组的当前生效绑定。
    不提供编辑入口，改绑统一走设置弹窗的"快捷键"分区。
  @Author: Bin.H
-->

<script setup>
import { computed } from "vue";
import { effectiveBinding, formatBindingForDisplay } from "../composables/useShortcuts";

const props = defineProps({
    visible: { type: Boolean, required: true },
    actions: { type: Array, required: true },
    shortcuts: { type: Object, default: () => ({}) },
});
const emit = defineEmits(["close"]);

const grouped = computed(() => {
    const groups = {};
    for (const action of props.actions) {
        if (!groups[action.category]) groups[action.category] = [];
        groups[action.category].push(action);
    }
    return groups;
});

function bindingLabel(action) {
    return formatBindingForDisplay(effectiveBinding(action, props.shortcuts));
}
</script>

<template>
    <Teleport to="body">
        <div v-if="visible" class="modal-overlay" @click.self="emit('close')">
            <div class="modal help-modal">
                <div class="modal-header">
                    <span class="modal-title">快捷键帮助</span>
                    <button class="modal-close" @click="emit('close')">×</button>
                </div>
                <div class="modal-body">
                    <div v-for="(items, category) in grouped" :key="category" class="shortcut-group">
                        <div class="shortcut-group-title">{{ category }}</div>
                        <div v-for="action in items" :key="action.id" class="shortcut-row">
                            <span>{{ action.label }}</span>
                            <kbd>{{ bindingLabel(action) }}</kbd>
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
    z-index: 2100;
}

.modal.help-modal {
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 10px;
    width: 420px;
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
}
.modal-close:hover {
    color: var(--fg);
}

.modal-body {
    overflow-y: auto;
    padding: 12px 20px 20px;
}

.shortcut-group {
    margin-bottom: 14px;
}
.shortcut-group-title {
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--fg-dim);
    text-transform: uppercase;
    margin-bottom: 6px;
}
.shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 5px 0;
    font-size: 0.9rem;
}
.shortcut-row kbd {
    font-size: 0.75rem;
    color: var(--fg-dim);
    background: var(--bg-dark);
    padding: 2px 6px;
    border-radius: var(--radius-sm);
}
</style>
