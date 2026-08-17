<!--
  @Component: 图标选择器（emoji 色块 + 弹出网格面板）
  @Description:
    任务图标用于日历页展示打卡记录，只从预设 emoji 里点选，不提供
    手动输入框（避免用户输错、贴入多字符文本等边界情况）。
  @Author: Bin.H
-->

<script setup>
import { nextTick, onBeforeUnmount, ref } from "vue";

const props = defineProps({
    // 当前图标，单个 emoji，空字符串表示未设置
    modelValue: { type: String, default: "" },
    // 按钮直径，单位 px
    size: { type: Number, default: 26 },
});

const emit = defineEmits(["update:modelValue"]);

// 预设图标，围绕"习惯打卡"这个使用场景挑的一批常见 emoji
const PRESETS = [
    "🔥", "💪", "📚", "💧", "🏃", "🧘", "😴", "🎯",
    "✅", "⭐", "🌱", "🍎", "🎨", "💻", "🧹", "💰",
    "📝", "☀️", "🌙", "🎵", "🧠", "❤️", "🐱", "🎮",
];

const open = ref(false);
const btnRef = ref(null);
const popoverRef = ref(null);
const popoverStyle = ref({});

function positionPopover() {
    const rect = btnRef.value?.getBoundingClientRect();
    if (!rect) return;
    popoverStyle.value = {
        top: `${rect.bottom + 6}px`,
        left: `${Math.min(rect.left, window.innerWidth - 224)}px`,
    };
}

function onClickOutside(e) {
    if (
        popoverRef.value?.contains(e.target) ||
        btnRef.value?.contains(e.target)
    ) {
        return;
    }
    close();
}

function onKeydown(e) {
    if (e.key === "Escape") close();
}

async function togglePopover() {
    if (open.value) {
        close();
        return;
    }
    open.value = true;
    await nextTick();
    positionPopover();
    document.addEventListener("mousedown", onClickOutside);
    document.addEventListener("keydown", onKeydown);
}

function close() {
    open.value = false;
    document.removeEventListener("mousedown", onClickOutside);
    document.removeEventListener("keydown", onKeydown);
}

function pick(icon) {
    emit("update:modelValue", icon);
    close();
}

onBeforeUnmount(() => {
    document.removeEventListener("mousedown", onClickOutside);
    document.removeEventListener("keydown", onKeydown);
});
</script>

<template>
    <span class="icon-picker">
        <button
            ref="btnRef"
            type="button"
            class="icon-btn"
            :class="{ empty: !modelValue }"
            :style="{ width: size + 'px', height: size + 'px' }"
            title="点击选择图标"
            @click="togglePopover"
        >{{ modelValue || "+" }}</button>

        <Teleport to="body">
            <div
                v-if="open"
                ref="popoverRef"
                class="icon-popover"
                :style="popoverStyle"
            >
                <div class="icon-grid">
                    <button
                        v-for="ic in PRESETS"
                        :key="ic"
                        type="button"
                        class="icon-cell"
                        :class="{ active: ic === modelValue }"
                        @click="pick(ic)"
                    >
                        {{ ic }}
                    </button>
                </div>

                <button
                    type="button"
                    class="icon-clear-btn"
                    :disabled="!modelValue"
                    @click="pick('')"
                >
                    清除图标
                </button>
            </div>
        </Teleport>
    </span>
</template>

<style scoped>
.icon-picker {
    display: inline-flex;
}

.icon-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    font-size: 1.1rem;
    line-height: 1;
    background: var(--bg-dark);
    box-shadow: inset 0 0 0 1px var(--border);
    cursor: pointer;
    transition: box-shadow 0.12s;
}
.icon-btn.empty {
    color: var(--fg-dim);
    font-size: 1rem;
}
.icon-btn:hover {
    box-shadow: inset 0 0 0 1px var(--fg-dark);
}

.icon-popover {
    position: fixed;
    z-index: 2000;
    width: 204px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
}

.icon-grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 4px;
}

.icon-cell {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    font-size: 1.1rem;
    line-height: 1;
    transition: background 0.12s;
}
.icon-cell:hover {
    background: var(--bg-select);
}
.icon-cell.active {
    background: var(--bg-select);
    box-shadow: inset 0 0 0 1.5px var(--blue);
}

.icon-clear-btn {
    padding: 5px 0;
    border-radius: 6px;
    font-size: 0.8rem;
    color: var(--fg-dim);
    background: var(--bg-dark);
}
.icon-clear-btn:hover:not(:disabled) {
    color: var(--red);
}
.icon-clear-btn:disabled {
    opacity: 0.4;
    cursor: default;
}
</style>
