<!--
  @Component: 颜色选择器（圆形色块 + 弹出面板）
  @Description:
    纯前端实现的取色控件，不依赖系统原生 <input type="color">。
    原生颜色选择器在部分 Linux 发行版（尤其 NixOS）上会因为打包环境缺少
    GTK 的 GSettings schema 而在弹出瞬间直接崩溃整个应用，所以这里换成
    预设色板 + HSV 取色盘 + 手动输入 hex 的组合，跨平台表现一致，
    也不再有这个崩溃风险。
  @Author: Bin.H
-->

<script setup>
import { nextTick, onBeforeUnmount, ref } from "vue";

const props = defineProps({
    // 当前颜色，hex 字符串（如 "#8250df"），空字符串/null 表示未设置
    modelValue: { type: String, default: "" },
    // 色块直径，单位 px
    size: { type: Number, default: 20 },
});

const emit = defineEmits(["update:modelValue"]);

// 预设色板，取自 style.css 里已有的强调色 token，跟应用其它地方的配色保持一致
const PRESETS = [
    "#d1242f", // 红
    "#bc4c00", // 橙
    "#9a6700", // 黄
    "#1a7f37", // 绿
    "#0598bc", // 青
    "#1a73e8", // 蓝
    "#8250df", // 洋红
    "#57606a", // 灰
];

const open = ref(false);
const hexInput = ref("");
const swatchRef = ref(null);
const popoverRef = ref(null);
const popoverStyle = ref({});

// 取色盘状态：色相 0-360，饱和度/明度 0-1
const hue = ref(260);
const sat = ref(0.65);
const val = ref(0.87);
const svRef = ref(null);
const hueRef = ref(null);

function isValidHex(v) {
    return /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(v || "");
}

function normalizeHex(v) {
    const h = v.trim();
    if (h.length === 4) {
        // #abc -> #aabbcc
        return "#" + h.slice(1).split("").map((c) => c + c).join("");
    }
    return h.toLowerCase();
}

function hexToRgb(hex) {
    const h = normalizeHex(hex).replace("#", "");
    const int = parseInt(h, 16);
    return { r: (int >> 16) & 255, g: (int >> 8) & 255, b: int & 255 };
}

function rgbToHex(r, g, b) {
    return (
        "#" +
        [r, g, b]
            .map((v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0"))
            .join("")
    );
}

function rgbToHsv(r, g, b) {
    r /= 255;
    g /= 255;
    b /= 255;
    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    const d = max - min;
    let h = 0;
    if (d !== 0) {
        if (max === r) h = ((g - b) / d) % 6;
        else if (max === g) h = (b - r) / d + 2;
        else h = (r - g) / d + 4;
        h *= 60;
        if (h < 0) h += 360;
    }
    const s = max === 0 ? 0 : d / max;
    return { h, s, v: max };
}

function hsvToRgb(h, s, v) {
    const c = v * s;
    const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
    const m = v - c;
    let r = 0,
        g = 0,
        b = 0;
    if (h < 60) [r, g, b] = [c, x, 0];
    else if (h < 120) [r, g, b] = [x, c, 0];
    else if (h < 180) [r, g, b] = [0, c, x];
    else if (h < 240) [r, g, b] = [0, x, c];
    else if (h < 300) [r, g, b] = [x, 0, c];
    else [r, g, b] = [c, 0, x];
    return { r: (r + m) * 255, g: (g + m) * 255, b: (b + m) * 255 };
}

/** 用当前 hue/sat/val 算出 hex，同步到 hex 输入框并对外 emit */
function emitFromHsv() {
    const { r, g, b } = hsvToRgb(hue.value, sat.value, val.value);
    const hex = rgbToHex(r, g, b);
    hexInput.value = hex;
    emit("update:modelValue", hex);
}

/** 用一个 hex 反推 hue/sat/val，让取色盘上的指示器跟着对上 */
function syncFromHex(hex) {
    if (!isValidHex(hex)) return;
    const { r, g, b } = hexToRgb(hex);
    const { h, s, v } = rgbToHsv(r, g, b);
    hue.value = h;
    sat.value = s;
    val.value = v;
}

function positionPopover() {
    const rect = swatchRef.value?.getBoundingClientRect();
    if (!rect) return;
    popoverStyle.value = {
        top: `${rect.bottom + 6}px`,
        left: `${Math.min(rect.left, window.innerWidth - 216)}px`,
    };
}

function onClickOutside(e) {
    if (
        popoverRef.value?.contains(e.target) ||
        swatchRef.value?.contains(e.target)
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
    const current = props.modelValue || "#8250df";
    hexInput.value = current;
    syncFromHex(current);
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

function pick(hex) {
    syncFromHex(hex);
    hexInput.value = hex;
    emit("update:modelValue", hex);
    close();
}

function applyHexInput() {
    const v = hexInput.value.trim();
    if (isValidHex(v)) {
        const hex = normalizeHex(v);
        syncFromHex(hex);
        emit("update:modelValue", hex);
    }
}

// ----------------------------------------
// 取色盘拖拽：饱和度/明度方形区域
// ----------------------------------------
function updateFromSvEvent(e) {
    const rect = svRef.value?.getBoundingClientRect();
    if (!rect) return;
    const x = Math.min(Math.max(e.clientX - rect.left, 0), rect.width);
    const y = Math.min(Math.max(e.clientY - rect.top, 0), rect.height);
    sat.value = rect.width ? x / rect.width : 0;
    val.value = rect.height ? 1 - y / rect.height : 0;
    emitFromHsv();
}

function startSvDrag(e) {
    updateFromSvEvent(e);
    const move = (ev) => updateFromSvEvent(ev);
    const up = () => {
        document.removeEventListener("pointermove", move);
        document.removeEventListener("pointerup", up);
    };
    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", up);
}

// ----------------------------------------
// 取色盘拖拽：色相滑条
// ----------------------------------------
function updateFromHueEvent(e) {
    const rect = hueRef.value?.getBoundingClientRect();
    if (!rect) return;
    const x = Math.min(Math.max(e.clientX - rect.left, 0), rect.width);
    hue.value = rect.width ? (x / rect.width) * 360 : 0;
    emitFromHsv();
}

function startHueDrag(e) {
    updateFromHueEvent(e);
    const move = (ev) => updateFromHueEvent(ev);
    const up = () => {
        document.removeEventListener("pointermove", move);
        document.removeEventListener("pointerup", up);
    };
    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", up);
}

onBeforeUnmount(() => {
    document.removeEventListener("mousedown", onClickOutside);
    document.removeEventListener("keydown", onKeydown);
});
</script>

<template>
    <span class="color-swatch-picker">
        <button
            ref="swatchRef"
            type="button"
            class="swatch-btn"
            :style="{
                background: modelValue || 'var(--fg-dark)',
                width: size + 'px',
                height: size + 'px',
            }"
            title="点击选择颜色"
            @click="togglePopover"
        ></button>

        <Teleport to="body">
            <div
                v-if="open"
                ref="popoverRef"
                class="swatch-popover"
                :style="popoverStyle"
            >
                <div class="swatch-presets">
                    <button
                        v-for="c in PRESETS"
                        :key="c"
                        type="button"
                        class="preset-dot"
                        :class="{
                            active:
                                (modelValue || '').toLowerCase() === c,
                        }"
                        :style="{ background: c }"
                        :title="c"
                        @click="pick(c)"
                    ></button>
                </div>

                <!-- 取色盘：饱和度/明度方形 + 色相滑条 -->
                <div
                    ref="svRef"
                    class="sv-square"
                    :style="{ backgroundColor: `hsl(${hue}, 100%, 50%)` }"
                    @pointerdown="startSvDrag"
                >
                    <span
                        class="sv-thumb"
                        :style="{
                            left: sat * 100 + '%',
                            top: (1 - val) * 100 + '%',
                        }"
                    ></span>
                </div>

                <div
                    ref="hueRef"
                    class="hue-slider"
                    @pointerdown="startHueDrag"
                >
                    <span
                        class="hue-thumb"
                        :style="{ left: (hue / 360) * 100 + '%' }"
                    ></span>
                </div>

                <div class="swatch-hex-row">
                    <span
                        class="hex-preview"
                        :style="{
                            background: isValidHex(hexInput)
                                ? hexInput
                                : 'transparent',
                        }"
                    ></span>
                    <input
                        v-model="hexInput"
                        class="hex-input"
                        placeholder="#8250df"
                        maxlength="7"
                        spellcheck="false"
                        @keydown.enter.prevent="applyHexInput"
                        @blur="applyHexInput"
                    />
                </div>
            </div>
        </Teleport>
    </span>
</template>

<style scoped>
.color-swatch-picker {
    display: inline-flex;
}

.swatch-btn {
    flex-shrink: 0;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
}

.swatch-popover {
    position: fixed;
    z-index: 2000;
    width: 196px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
}

.swatch-presets {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
}

.preset-dot {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
    transition: transform 0.1s;
}
.preset-dot:hover {
    transform: scale(1.1);
}
.preset-dot.active {
    box-shadow:
        0 0 0 2px var(--bg-popup),
        0 0 0 4px var(--blue);
}

/* 取色盘：饱和度(x) / 明度(y) 方形 */
.sv-square {
    position: relative;
    width: 100%;
    height: 110px;
    border-radius: 6px;
    cursor: crosshair;
    touch-action: none;
    background-image:
        linear-gradient(to bottom, rgba(0, 0, 0, 0) 0%, #000 100%),
        linear-gradient(to right, #fff 0%, rgba(255, 255, 255, 0) 100%);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
}
.sv-thumb {
    position: absolute;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid #fff;
    box-shadow:
        0 0 0 1px rgba(0, 0, 0, 0.5),
        0 1px 3px rgba(0, 0, 0, 0.4);
    transform: translate(-50%, -50%);
    pointer-events: none;
}

/* 取色盘：色相滑条 */
.hue-slider {
    position: relative;
    width: 100%;
    height: 12px;
    border-radius: 999px;
    cursor: pointer;
    touch-action: none;
    background: linear-gradient(
        to right,
        #f00 0%,
        #ff0 17%,
        #0f0 33%,
        #0ff 50%,
        #00f 67%,
        #f0f 83%,
        #f00 100%
    );
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
}
.hue-thumb {
    position: absolute;
    top: 50%;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #fff;
    border: 2px solid #fff;
    box-shadow:
        0 0 0 1px rgba(0, 0, 0, 0.5),
        0 1px 3px rgba(0, 0, 0, 0.4);
    transform: translate(-50%, -50%);
    pointer-events: none;
}

.swatch-hex-row {
    display: flex;
    align-items: center;
    gap: 6px;
}

.hex-preview {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
}

.hex-input {
    flex: 1;
    min-width: 0;
    padding: 4px 8px;
    font-family: monospace;
    font-size: 0.85rem;
    background: var(--bg-dark);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--fg);
    outline: none;
}
.hex-input:focus {
    border-color: var(--blue);
}
</style>
