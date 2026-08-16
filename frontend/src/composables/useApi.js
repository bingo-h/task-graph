/**
 * @file API 客户端
 * @module useApi
 * @description
 *  封装所有与后端的通信，统一错误处理。
 *  所有写操作成功后后端返回最新的完整图数据，
 *  调用方直接用返回值更新状态。
 * @author Bin.H
 * @date 2026-05-29
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * 统一错误处理：Tauri command 失败时 invoke() 会 reject 一个字符串，
 * 这里转换成标准 Error 对象，便于上层用 e.message 统一处理。
 */
async function call(command, args) {
  try {
    return await invoke(command, args);
  } catch (err) {
    // Tauri command 的 Err(String) 会原样作为 reject 的值
    throw new Error(typeof err === "string" ? err : "请求失败");
  }
}

/** 获取所有任务和项目树数据。 */
export async function fetchTasks() {
  return call("get_tasks");
}

/**
 * 新建项目，允许在没有任何任务的情况下独立创建。path 如 "personal.reading"。
 * stage: "planned"（计划中）| "active"（进行中），不传时后端默认 "active"。
 */
export async function createProject(path, stage) {
  return call("create_project", { args: { path, stage } });
}

/** 设置项目归档状态，归档会级联到所有子项目。 */
export async function setProjectArchived(path, archived) {
  return call("set_project_archived", { args: { path, archived } });
}

/** 设置项目所属阶段："planned"（计划中）| "active"（进行中）。 */
export async function setProjectStage(path, stage) {
  return call("set_project_stage", { args: { path, stage } });
}

/** 将项目移入废纸篓（软删除，级联到所有子项目，可恢复）。 */
export async function trashProject(path) {
  return call("trash_project", { path });
}

/** 从废纸篓恢复项目。 */
export async function restoreProject(path) {
  return call("restore_project", { path });
}

/** 彻底删除项目，级联删除该项目及所有子项目下的任务，不可恢复。 */
export async function purgeProject(path) {
  return call("purge_project", { path });
}

/** 移动项目（及其子项目、任务）到新的父项目下；newParent 为 null/空 表示移到顶层。 */
export async function moveProject(path, newParent) {
  return call("move_project", { args: { path, new_parent: newParent || null } });
}

/** 获取应用设置：{ trash_retention_days, font_size }。 */
export async function getSettings() {
  return call("get_settings");
}

/** 保存应用设置：{ trash_retention_days, font_size }。 */
export async function saveSettings(settings) {
  return call("save_settings", { settings });
}

/** 新建任务，传结构化字段：{description, project, priority, due, scheduled, tags, depends} */
export async function addTask(fields) {
  return call("add_task", { args: fields });
}

/** 修改任务，传结构化字段（含 clear_* 清空标志）。 */
export async function modifyTask(uuid, fields) {
  return call("modify_task", { args: { uuid, ...fields } });
}

/**
 * 改变一条依赖关系的终点（拖拽图谱里已有连线的终点）：
 * newTargetUuid 传 null 表示拖到空白处，直接删除这条依赖。
 */
export async function reconnectDependency(sourceUuid, oldTargetUuid, newTargetUuid) {
  return call("reconnect_dependency", {
    args: {
      source_uuid: sourceUuid,
      old_target_uuid: oldTargetUuid,
      new_target_uuid: newTargetUuid || null,
    },
  });
}

/** 重命名标签（用到旧名字的任务一起改名）；若新名字已存在则合并为同一个标签。 */
export async function renameTag(oldTag, newTag) {
  return call("rename_tag", { old_tag: oldTag, new_tag: newTag });
}

/** 设置标签颜色，color 传 null/空字符串表示清空（前端回退到默认颜色）。 */
export async function setTagColor(name, color) {
  return call("set_tag_color", { args: { name, color: color || null } });
}

/** 彻底删除一个标签，解除它和所有任务的关联。 */
export async function deleteTag(name) {
  return call("delete_tag", { name });
}

/** 将任务标记为完成。 */
export async function doneTask(uuid) {
  return call("done_task", { uuid });
}

/** 批量将多个任务标记为完成（框选/Ctrl 多选后的批量操作）。 */
export async function doneTasks(uuids) {
  return call("done_tasks", { uuids });
}

/** 取消任务完成，恢复为待办。 */
export async function undoneTask(uuid) {
  return call("undone_task", { uuid });
}

/** 设置/取消"今日任务"标记，过了当天会在下次读取数据时自动清除。 */
export async function setTaskToday(uuid, marked) {
  return call("set_task_today", { uuid, marked });
}

/** 批量设置多个任务的"今日任务"标记（框选/Ctrl 多选后的批量操作）。 */
export async function setTasksToday(uuids, marked) {
  return call("set_tasks_today", { args: { uuids, marked } });
}

/** 删除任务。 */
export async function deleteTask(uuid) {
  return call("delete_task", { uuid });
}

/** 批量删除多个任务（框选/Ctrl 多选后的批量操作）。 */
export async function deleteTasks(uuids) {
  return call("delete_tasks", { uuids });
}

/** 开始为指定任务计时（若有其他任务正在计时会自动先结束）。 */
export async function startTimer(uuid) {
  return call("start_timer", { uuid });
}

/** 停止当前正在进行的计时。 */
export async function stopTimer() {
  return call("stop_timer");
}

/**
 * 同时为多个任务开始计时（框选/Ctrl 多选后批量计时），
 * 这些任务共享同一段计时，各自单独记一条计时记录；若有其他任务正在计时会自动先结束。
 * 停止时直接调用 stopTimer 即可，不会连带标记任务完成。
 */
export async function startGroupTimer(uuids) {
  return call("start_group_timer", { uuids });
}

/** 获取某任务的全部计时记录，按开始时间倒序。 */
export async function listTimeEntries(uuid) {
  return call("list_time_entries", { uuid });
}

/** 获取所有任务的全部计时记录（用于首页仪表盘按天聚合统计）。 */
export async function listAllTimeEntries() {
  return call("list_all_time_entries");
}

/** 保存/修改某段计时记录的回忆总结（标题 + 正文）。 */
export async function saveTimeEntryNote(id, title, body) {
  return call("save_time_entry_note", { args: { id, title, body } });
}

/** 删除某条计时记录（不可恢复）。 */
export async function deleteTimeEntry(id) {
  return call("delete_time_entry", { id });
}
