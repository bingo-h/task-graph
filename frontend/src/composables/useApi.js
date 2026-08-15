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

/** 新建任务，传结构化字段：{description, project, priority, due, scheduled, tags, depends} */
export async function addTask(fields) {
  return call("add_task", { args: fields });
}

/** 修改任务，传结构化字段（含 clear_* 清空标志）。 */
export async function modifyTask(uuid, fields) {
  return call("modify_task", { args: { uuid, ...fields } });
}

/** 将任务标记为完成。 */
export async function doneTask(uuid) {
  return call("done_task", { uuid });
}

/** 取消任务完成，恢复为待办。 */
export async function undoneTask(uuid) {
  return call("undone_task", { uuid });
}

/** 删除任务。 */
export async function deleteTask(uuid) {
  return call("delete_task", { uuid });
}

/** 开始为指定任务计时（若有其他任务正在计时会自动先结束）。 */
export async function startTimer(uuid) {
  return call("start_timer", { uuid });
}

/** 停止当前正在进行的计时。 */
export async function stopTimer() {
  return call("stop_timer");
}

/** 获取某任务的全部计时记录，按开始时间倒序。 */
export async function listTimeEntries(uuid) {
  return call("list_time_entries", { uuid });
}
