/**
 * @file 自绘确认弹窗的共享状态
 * @module useConfirm
 * @description
 *  替代浏览器原生 confirm()：原生弹窗样式跟应用完全不搭，且在 WebKitGTK 下的位置/样式
 *  也不受控制。这里用一个模块级的响应式单例状态 + Promise，配合 ConfirmDialog.vue
 *  （挂载一次在 App.vue 根部）实现同样"await 一下就知道用户选了确认还是取消"的调用方式。
 * @author Bin.H
 */

import { reactive } from "vue";

export const confirmState = reactive({
    visible: false,
    title: "确认",
    message: "",
    confirmText: "确认",
    cancelText: "取消",
    danger: false, // 危险操作（删除类）确认按钮标红
    _resolve: null,
});

/**
 * 弹出确认框，返回 Promise<boolean>：用户点确认为 true，点取消/点遮罩/按 Esc 为 false。
 * @param {string} message - 提示文案
 * @param {{ title?: string, confirmText?: string, cancelText?: string, danger?: boolean }} [options]
 */
export function confirmDialog(message, options = {}) {
    return new Promise((resolve) => {
        confirmState.title = options.title ?? "确认";
        confirmState.message = message;
        confirmState.confirmText = options.confirmText ?? "确认";
        confirmState.cancelText = options.cancelText ?? "取消";
        confirmState.danger = options.danger ?? false;
        confirmState.visible = true;
        confirmState._resolve = resolve;
    });
}

/** 由 ConfirmDialog.vue 在用户做出选择时调用，结束当前这次 confirmDialog() 的等待 */
export function resolveConfirm(result) {
    confirmState.visible = false;
    confirmState._resolve?.(result);
    confirmState._resolve = null;
}
