<!--
  @Component: SegmentedControl
  @Description: 共享分段控件（.segmented），内置滑动指示器动画；option 可选携带
    activeColor/activeBg 表达语义色（比如优先级 H/M/L），不传则用中性外壳配色。
-->

<script setup>
import { ref, computed } from "vue";
import { useSlidingIndicator } from "../composables/useSlidingIndicator";

const props = defineProps({
    options: { type: Array, required: true }, // [{ key, label, activeColor?, activeBg? }]
    modelValue: { required: true },
});
const emit = defineEmits(["update:modelValue"]);

const rootEl = ref(null);
const indicator = useSlidingIndicator(rootEl, "button.active", () => props.modelValue);

const activeOption = computed(() =>
    props.options.find((o) => o.key === props.modelValue),
);

// 语义色选项（activeBg）覆盖中性外壳配色，且不分风格都不要新拟态的凸起阴影
const indicatorStyle = computed(() => ({
    transform: `translateX(${indicator.left}px)`,
    width: `${indicator.width}px`,
    ...(activeOption.value?.activeBg
        ? { background: activeOption.value.activeBg, boxShadow: "none" }
        : {}),
}));
</script>

<template>
    <div class="segmented" ref="rootEl">
        <span class="segmented-indicator" :style="indicatorStyle"></span>
        <button
            v-for="opt in options"
            :key="opt.key"
            type="button"
            :class="{ active: modelValue === opt.key }"
            :style="
                modelValue === opt.key && opt.activeColor
                    ? { color: opt.activeColor }
                    : {}
            "
            @click="emit('update:modelValue', opt.key)"
        >
            {{ opt.label }}
        </button>
    </div>
</template>
