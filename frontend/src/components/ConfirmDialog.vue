<!--
  @Component: 自绘确认弹窗
  @Description:
    替代浏览器原生 confirm()。全局单例，挂载一次在 App.vue 根部即可，
    具体调用方式见 useConfirm.js 的 confirmDialog()。
    可能出现在其它弹窗（比如标签管理）打开期间，z-index 比普通弹窗更高，确保盖在最上面。
  @Author: Bin.H
-->

<script setup>
import { nextTick, ref, watch } from "vue";
import { confirmState, resolveConfirm } from "../composables/useConfirm";

// 弹出时把焦点放到"取消"这个更安全的默认选项上：
// 一方面 Enter 键默认触发的是取消而不是危险操作，另一方面焦点落在弹窗内部，
// keydown 事件才会冒泡到下面的遮罩层，Esc 关闭才生效（遮罩层本身不在焦点链路上收不到按键事件）
const cancelBtnRef = ref(null);
watch(
    () => confirmState.visible,
    async (visible) => {
        if (!visible) return;
        await nextTick();
        cancelBtnRef.value?.focus();
    },
);
</script>

<template>
    <Teleport to="body">
        <div
            v-if="confirmState.visible"
            class="confirm-overlay"
            @click.self="resolveConfirm(false)"
            @keydown.esc="resolveConfirm(false)"
        >
            <div class="confirm-modal">
                <div class="confirm-header">
                    <span class="confirm-title">{{ confirmState.title }}</span>
                    <button class="modal-close" @click="resolveConfirm(false)">
                        ×
                    </button>
                </div>

                <div class="confirm-body">{{ confirmState.message }}</div>

                <div class="confirm-footer">
                    <button
                        ref="cancelBtnRef"
                        class="btn-cancel"
                        @click="resolveConfirm(false)"
                    >
                        {{ confirmState.cancelText }}
                    </button>
                    <button
                        class="btn-submit"
                        :class="{ 'btn-submit-danger': confirmState.danger }"
                        @click="resolveConfirm(true)"
                    >
                        {{ confirmState.confirmText }}
                    </button>
                </div>
            </div>
        </div>
    </Teleport>
</template>

<style scoped>
.confirm-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1100;
}

.confirm-modal {
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 10px;
    width: 380px;
    max-width: 90vw;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
}

.confirm-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
}
.confirm-title {
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

.confirm-body {
    padding: 18px 20px;
    color: var(--fg);
    font-size: 0.9231rem;
    line-height: 1.6;
    white-space: pre-line;
}

.confirm-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 14px 20px;
    border-top: 1px solid var(--border);
}
.btn-submit {
    padding: 7px 20px;
    border-radius: 6px;
    background: var(--blue);
    color: var(--bg);
    font-weight: 700;
    font-size: 1rem;
    transition: opacity 0.15s;
}
.btn-submit:hover {
    opacity: 0.85;
}
.btn-submit-danger {
    background: var(--red);
}
.btn-cancel {
    padding: 7px 16px;
    border-radius: 6px;
    border: 1px solid var(--border);
    color: var(--fg-dim);
    font-size: 1rem;
    transition: all 0.15s;
}
.btn-cancel:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}
</style>
