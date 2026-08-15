/**
 * @file 应用内自动更新
 * @module useUpdater
 * @description
 *  封装 Tauri updater 插件：检查更新、下载并安装、重启应用。
 * @author Bin.H
 * @date 2026-08-15
 */

import { ref } from "vue";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// idle -> checking -> (up-to-date | available -> downloading -> ready) -> restarting
export const updateStatus = ref("idle");
export const updateError = ref("");
export const latestVersion = ref("");
export const updateNotes = ref("");

let pendingUpdate = null;

export async function checkForUpdate() {
    updateStatus.value = "checking";
    updateError.value = "";
    try {
        const update = await check();
        if (update) {
            pendingUpdate = update;
            latestVersion.value = update.version;
            updateNotes.value = update.body || "";
            updateStatus.value = "available";
        } else {
            updateStatus.value = "up-to-date";
        }
    } catch (err) {
        updateError.value = err instanceof Error ? err.message : String(err);
        updateStatus.value = "error";
    }
}

export async function downloadAndInstallUpdate() {
    if (!pendingUpdate) return;
    updateStatus.value = "downloading";
    updateError.value = "";
    try {
        await pendingUpdate.downloadAndInstall();
        updateStatus.value = "ready";
    } catch (err) {
        updateError.value = err instanceof Error ? err.message : String(err);
        updateStatus.value = "error";
    }
}

export async function restartToApply() {
    updateStatus.value = "restarting";
    await relaunch();
}
