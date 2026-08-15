<!--
  @Component: 计时记录回忆总结弹窗
  @Description:
    用于填写/查看/修改某一段计时记录的回忆总结（标题 + 正文）。
    两种触发场景复用同一个组件：
      1. 停止计时后弹出，提示为刚结束的这段专注写总结
      2. 点击"计时记录"里已有的总结标题，查看/修改详情
  @Author: Bin.H
-->

<script setup>
import { ref, watch } from "vue";

const props = defineProps({
    visible: { type: Boolean, required: true },
    heading: { type: String, default: "回忆总结" },
    initialTitle: { type: String, default: "" },
    initialBody: { type: String, default: "" },
});

const emit = defineEmits(["close", "save"]);

const title = ref("");
const body = ref("");

// 是否是"事后修改"已有内容（而非刚结束计时的空白填写），影响取消按钮的文案
const isEditingExisting = ref(false);

watch(
    () => props.visible,
    (visible) => {
        if (!visible) return;
        title.value = props.initialTitle || "";
        body.value = props.initialBody || "";
        isEditingExisting.value = !!(props.initialTitle || props.initialBody);
    },
);

function submit() {
    emit("save", { title: title.value.trim(), body: body.value.trim() });
}
</script>

<template>
    <Teleport to="body">
        <div v-if="visible" class="modal-overlay" @click.self="emit('close')">
            <div class="modal">
                <div class="modal-header">
                    <span class="modal-title">{{ heading }}</span>
                    <button class="modal-close" @click="emit('close')">
                        ×
                    </button>
                </div>

                <div class="modal-body">
                    <div class="form-row">
                        <label class="form-label">标题</label>
                        <input
                            v-model="title"
                            class="form-input"
                            placeholder="这段专注做了什么？"
                            @keydown.enter="submit"
                        />
                    </div>
                    <div class="form-row">
                        <label class="form-label">正文</label>
                        <textarea
                            v-model="body"
                            class="form-textarea"
                            rows="6"
                            placeholder="记录具体做了什么、想法或下一步计划…"
                        ></textarea>
                    </div>
                </div>

                <div class="modal-footer">
                    <button class="btn-cancel" @click="emit('close')">
                        {{ isEditingExisting ? "取消" : "跳过" }}
                    </button>
                    <button class="btn-submit" @click="submit">保存</button>
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
    z-index: 1000;
}

.modal {
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 10px;
    width: 440px;
    max-width: 90vw;
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
    transition: color 0.15s;
}
.modal-close:hover {
    color: var(--fg);
}

.modal-body {
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
}

.form-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
}
.form-label {
    font-size: 0.8462rem;
    font-weight: 700;
    color: var(--fg-dim);
    text-transform: uppercase;
    letter-spacing: 0.05em;
}
.form-input {
    width: 100%;
}
.form-textarea {
    width: 100%;
    resize: vertical;
    font: inherit;
    color: var(--fg);
    background: var(--bg-dark);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 10px;
    outline: none;
    line-height: 1.5;
}
.form-textarea:focus {
    border-color: var(--blue);
}

.modal-footer {
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
