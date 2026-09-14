import { reactive, onMounted, onUnmounted, nextTick, watch } from "vue";

/**
 * 一组宽度不固定的按钮之间的"滑动指示器"：实测当前激活按钮的 offsetLeft/offsetWidth，
 * 供调用方把 { left, width } 拼成 transform/width 内联样式做 CSS transition。
 * 页面顶部的页面切换按钮、设置/表单里的分段控件（.segmented）共用这套逻辑。
 *
 * @param {import('vue').Ref<HTMLElement|null>} containerRef - 指示器所在的定位容器
 * @param {string} activeSelector - 在容器内查找当前激活按钮的选择器，如 ".page-nav-btn.active"
 * @param {import('vue').WatchSource} [watchSource] - 激活项切换时应该重新测量的响应式来源
 */
export function useSlidingIndicator(containerRef, activeSelector, watchSource) {
    const indicator = reactive({ left: 0, width: 0 });
    let resizeObserver = null;

    function update() {
        const active = containerRef.value?.querySelector(activeSelector);
        if (!active) return;
        indicator.left = active.offsetLeft;
        indicator.width = active.offsetWidth;
    }

    if (watchSource !== undefined) {
        watch(watchSource, () => nextTick(update));
    }

    onMounted(() => {
        nextTick(update);
        // 字号等设置改变会连带改变按钮宽度，用 ResizeObserver 盯着容器自身尺寸变化即可覆盖这个场景
        resizeObserver = new ResizeObserver(update);
        if (containerRef.value) resizeObserver.observe(containerRef.value);
    });
    onUnmounted(() => resizeObserver?.disconnect());

    return indicator;
}
