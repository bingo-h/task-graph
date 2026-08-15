<!--
  @Component: 项目右键菜单
  @Description:
    右键点击项目节点弹出的操作菜单。正常项目：新建子项目 / 移动到.. / 移入废纸篓；
    废纸篓中的项目：恢复（如果本身就是被移入的那个节点）/ 彻底删除。
  @Author: Bin.H
  @Date: 2026-08-15
-->

<script setup>
import { ref } from "vue";

const props = defineProps({
    visible: { type: Boolean, required: true },
    x: { type: Number, default: 0 },
    y: { type: Number, default: 0 },
    node: { type: Object, default: null },
    moveTargets: { type: Array, default: () => [] },
});

const emit = defineEmits([
    "close",
    "new-subproject",
    "move",
    "toggle-archive",
    "trash",
    "restore",
    "purge",
]);

const showMoveSubmenu = ref(false);

function close() {
    showMoveSubmenu.value = false;
    emit("close");
}

function pickMoveTarget(targetPath) {
    emit("move", targetPath);
    close();
}
</script>

<template>
    <Teleport to="body">
        <div v-if="visible" class="menu-overlay" @click="close" @contextmenu.prevent="close">
            <div
                class="context-menu"
                :style="{ left: `${x}px`, top: `${y}px` }"
                @click.stop
            >
                <template v-if="node?.trashed">
                    <button
                        v-if="node.self_trashed"
                        class="menu-item"
                        @click="
                            emit('restore');
                            close();
                        "
                    >
                        ↺ 恢复
                    </button>
                    <button
                        class="menu-item danger"
                        @click="
                            emit('purge');
                            close();
                        "
                    >
                        🗑 彻底删除
                    </button>
                </template>

                <template v-else>
                    <button
                        class="menu-item"
                        @click="
                            emit('new-subproject');
                            close();
                        "
                    >
                        + 新建子项目
                    </button>

                    <div class="menu-item submenu-trigger" @click.stop="showMoveSubmenu = !showMoveSubmenu">
                        <span>→ 移动到...</span>
                        <span class="submenu-arrow">▸</span>

                        <div v-if="showMoveSubmenu" class="submenu" @click.stop>
                            <!-- 项目本身已经在顶层时，"移到顶层"等于移到自己，不显示 -->
                            <button
                                v-if="node?.depth !== 0"
                                class="menu-item"
                                @click="pickMoveTarget(null)"
                            >
                                （顶层）
                            </button>
                            <button
                                v-for="t in moveTargets"
                                :key="t.path"
                                class="menu-item"
                                @click="pickMoveTarget(t.path)"
                            >
                                {{ t.label }}
                            </button>
                            <span
                                v-if="moveTargets.length === 0 && node?.depth === 0"
                                class="empty-hint"
                            >
                                没有其它可选项目
                            </span>
                        </div>
                    </div>

                    <!-- 归档只能在顶层项目上操作，子项目跟随顶层项目一起归档 -->
                    <button
                        v-if="node?.depth === 0"
                        class="menu-item"
                        @click="
                            emit('toggle-archive', !node.self_archived);
                            close();
                        "
                    >
                        {{ node.self_archived ? "↺ 取消归档" : "📦 项目归档" }}
                    </button>

                    <button
                        class="menu-item danger"
                        @click="
                            emit('trash');
                            close();
                        "
                    >
                        🗑 移入废纸篓
                    </button>
                </template>
            </div>
        </div>
    </Teleport>
</template>

<style scoped>
.menu-overlay {
    position: fixed;
    inset: 0;
    z-index: 2000;
}

.context-menu {
    position: fixed;
    min-width: 150px;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 1px;
}

.menu-item {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 7px 10px;
    border-radius: 5px;
    font-size: 0.9231rem;
    color: var(--fg);
    text-align: left;
    transition: background 0.12s;
    cursor: pointer;
}
.menu-item:hover {
    background: var(--bg-select);
}
.menu-item.danger {
    color: var(--red);
}

.submenu-arrow {
    font-size: 0.7692rem;
    color: var(--fg-dim);
}

.submenu {
    position: absolute;
    left: 100%;
    top: 0;
    min-width: 160px;
    max-height: 260px;
    overflow-y: auto;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 1px;
}

.empty-hint {
    padding: 6px 10px;
    font-size: 0.8462rem;
    color: var(--fg-dark);
}
</style>
