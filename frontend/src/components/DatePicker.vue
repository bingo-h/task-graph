<!--
  @Component: 日期选择器
  @Description: 纯前端实现的日期选择控件，替代原生 <input type="date">。
                 原生控件在 WebKitGTK (Tauri Linux) 下存在弹出日历点击后不自动关闭的问题，
                 因此自绘一个下拉日历，交互完全由组件自身控制（点外部/选中即关闭）。
  @Author: Bin.H
-->

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";

const props = defineProps({
    // 格式: YYYY-MM-DD，空字符串代表未设置
    modelValue: { type: String, default: "" },
    placeholder: { type: String, default: "选择日期" },
});

const emit = defineEmits(["update:modelValue"]);

const rootRef = ref(null);
const open = ref(false);

// 面板当前展示的年月（不代表选中值）
const viewDate = computed(() => {
    if (props.modelValue) {
        const [y, m] = props.modelValue.split("-").map(Number);
        return { year: y, month: m - 1 };
    }
    const now = new Date();
    return { year: now.getFullYear(), month: now.getMonth() };
});
const viewYear = ref(viewDate.value.year);
const viewMonth = ref(viewDate.value.month);

function syncViewToSelected() {
    viewYear.value = viewDate.value.year;
    viewMonth.value = viewDate.value.month;
}

function toggle() {
    if (!open.value) syncViewToSelected();
    open.value = !open.value;
}

function close() {
    open.value = false;
}

function prevMonth() {
    if (viewMonth.value === 0) {
        viewMonth.value = 11;
        viewYear.value -= 1;
    } else {
        viewMonth.value -= 1;
    }
}

function nextMonth() {
    if (viewMonth.value === 11) {
        viewMonth.value = 0;
        viewYear.value += 1;
    } else {
        viewMonth.value += 1;
    }
}

function pad(n) {
    return String(n).padStart(2, "0");
}

// 日历格子：包含上月/下月的填充日期
const cells = computed(() => {
    const firstDay = new Date(viewYear.value, viewMonth.value, 1).getDay(); // 0=周日
    const daysInMonth = new Date(
        viewYear.value,
        viewMonth.value + 1,
        0,
    ).getDate();

    const result = [];
    for (let i = 0; i < firstDay; i++) {
        result.push(null);
    }
    for (let d = 1; d <= daysInMonth; d++) {
        result.push(d);
    }
    return result;
});

const todayStr = (() => {
    const t = new Date();
    return `${t.getFullYear()}-${pad(t.getMonth() + 1)}-${pad(t.getDate())}`;
})();

function cellDateStr(day) {
    return `${viewYear.value}-${pad(viewMonth.value + 1)}-${pad(day)}`;
}

function selectDay(day) {
    if (!day) return;
    emit("update:modelValue", cellDateStr(day));
    close();
}

function clearDate() {
    emit("update:modelValue", "");
    close();
}

function goToday() {
    const t = new Date();
    emit(
        "update:modelValue",
        `${t.getFullYear()}-${pad(t.getMonth() + 1)}-${pad(t.getDate())}`,
    );
    close();
}

// 点击面板外部时收起
function handleClickOutside(e) {
    if (rootRef.value && !rootRef.value.contains(e.target)) {
        close();
    }
}

onMounted(() => {
    document.addEventListener("mousedown", handleClickOutside);
});
onBeforeUnmount(() => {
    document.removeEventListener("mousedown", handleClickOutside);
});
</script>

<template>
    <div ref="rootRef" class="date-picker">
        <button type="button" class="date-input" @click="toggle">
            <span :class="{ placeholder: !modelValue }">
                {{ modelValue || placeholder }}
            </span>
            <span class="date-icon">📅</span>
        </button>

        <div v-if="open" class="date-panel" @keydown.esc="close">
            <div class="date-panel-header">
                <button type="button" class="nav-btn" @click="prevMonth">
                    ‹
                </button>
                <span class="panel-title">
                    {{ viewYear }} 年 {{ viewMonth + 1 }} 月
                </span>
                <button type="button" class="nav-btn" @click="nextMonth">
                    ›
                </button>
            </div>

            <div class="weekday-row">
                <span v-for="w in ['日', '一', '二', '三', '四', '五', '六']" :key="w">
                    {{ w }}
                </span>
            </div>

            <div class="day-grid">
                <button
                    v-for="(day, i) in cells"
                    :key="i"
                    type="button"
                    class="day-cell"
                    :disabled="!day"
                    :class="{
                        empty: !day,
                        selected: day && cellDateStr(day) === modelValue,
                        today: day && cellDateStr(day) === todayStr,
                    }"
                    @click="selectDay(day)"
                >
                    {{ day || "" }}
                </button>
            </div>

            <div class="date-panel-footer">
                <button type="button" class="footer-btn" @click="goToday">
                    今天
                </button>
                <button type="button" class="footer-btn" @click="clearDate">
                    清空
                </button>
            </div>
        </div>
    </div>
</template>

<style scoped>
.date-picker {
    position: relative;
    width: 150px;
    flex-shrink: 0;
}

.date-input {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    font: inherit;
    color: var(--fg);
    background: var(--bg-dark);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 10px;
    outline: none;
    text-align: left;
}

.date-input .placeholder {
    color: var(--fg-dim);
}

.date-icon {
    font-size: 0.9231rem;
    opacity: 0.7;
}

.date-panel {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 10;
    width: 240px;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.25);
    padding: 10px;
}

.date-panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
}
.panel-title {
    font-size: 0.9231rem;
    font-weight: 700;
    color: var(--fg);
}
.nav-btn {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: var(--fg-dim);
    font-size: 1.0769rem;
}
.nav-btn:hover {
    background: rgba(0, 0, 0, 0.06);
    color: var(--fg);
}

.weekday-row {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    margin-bottom: 4px;
}
.weekday-row span {
    text-align: center;
    font-size: 0.7692rem;
    color: var(--fg-dim);
}

.day-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
}
.day-cell {
    aspect-ratio: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 5px;
    font-size: 0.9231rem;
    color: var(--fg);
    transition: background 0.12s;
}
.day-cell:not(.empty):hover {
    background: rgba(0, 0, 0, 0.06);
}
.day-cell.empty {
    cursor: default;
}
.day-cell.today {
    color: var(--blue);
    font-weight: 700;
}
.day-cell.selected {
    background: var(--blue);
    color: #fff;
    font-weight: 700;
}

.date-panel-footer {
    display: flex;
    justify-content: space-between;
    gap: 6px;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
}
.footer-btn {
    flex: 1;
    padding: 4px 0;
    border-radius: 5px;
    font-size: 0.8462rem;
    color: var(--fg-dim);
    transition: all 0.12s;
}
.footer-btn:hover {
    background: rgba(0, 0, 0, 0.06);
    color: var(--fg);
}
</style>
