<!--
  @Component: 项目树面板容器
  @Description: 管理折叠状态，渲染树头部和所有根节点。
  @Author: Bin.H
  @Date: 2026-05-26
-->

<script setup>
import { ref } from "vue";
import ProjectTreeNode from "./ProjectTreeNode.vue";

const props = defineProps({
    projects: { type: Object, required: true },
    roots: { type: Array, required: true },
    selected: { type: String, default: null },
});

const emit = defineEmits(["select"]);

const collapsed = ref(new Set());

function onToggle(path) {
    if (collapsed.value.has(path)) collapsed.value.delete(path);
    else collapsed.value.add(path);

    // ref 只关注地址变化，因此要触发页面更新需要改变地址
    collapsed.value = new Set(collapsed.value);
}
</script>

<template>
    <aside class="project-tree">
        <!-- 头部标题栏 -->
        <div class="tree-header">
            <span class="tree-title">项目</span>
            <button
                class="show-all-btn"
                :class="{ active: selected === null }"
                @click="emit('select', null)"
            >
                全部
            </button>
        </div>

        <div class="tree-body">
            <ProjectTreeNode
                v-for="root in roots"
                :key="root"
                :path="root"
                :projects="projects"
                :selected="selected"
                :collapsed="collapsed"
                @select="
                    (path) => emit('select', path === selected ? null : path)
                "
                @toggle="onToggle"
            />
        </div>
    </aside>
</template>

<style scoped>
/* 面板容器 */
.project-tree {
    width: 220px;
    flex-shrink: 0;
    background: var(--bg-panel);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

/* 头部标题 */
.tree-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px 6px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
}

.tree-title {
    font-weight: 700;
    color: var(--cyan);
    font-size: 13px;
}

/* "全部"按钮 */
.show-all-btn {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    transition: all 0.15s;
}
.show-all-btn:hover {
    color: var(--fg);
}
.show-all-btn.active {
    color: var(--blue);
    border-color: var(--blue);
}

/* 滚动区域 */
.tree-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
}
.tree-body::-webkit-scrollbar {
    width: 4px;
}
.tree-body::-webkit-scrollbar-thumb {
    background: var(--fg-dark);
    border-radius: 2px;
}
</style>
