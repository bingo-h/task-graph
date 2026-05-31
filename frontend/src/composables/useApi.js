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

import constants from "../config/constants";

const BASE = constants.API_BASE;

/**
 * 统一 fetch 封装
 * @param {{}} [options={}]
 * @param {string} path - 路径
 */
async function request(path, options = {}) {
  const res = await fetch(BASE + path, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });

  if (!res.ok) {
    const err = await res.json().catch(() => ({ detail: res.statusText }));
    throw new Error(err.detail || "请求失败");
  }

  return res.json();
}

/** 获取所有任务和项目树数据 */
export async function fetchTasks() {
  return request("/tasks");
}

/**
 * 新建任务
 * @param {string} command - 原生 taskwarrior 语法，如 "修复 bug project:work due:2026-05-20"
 */
export async function addTask(command) {
  return request("/task/add", {
    method: "POST",
    body: JSON.stringify({ command }),
  });
}

/**
 * 修改任务
 * @param {string} uuid
 * @param {string} command - 修改参数，如 "due:2026-05-25 priority:H"
 */
export async function modifyTask(uuid, command) {
  console.log("modifyTask payload:", { uuid, command }); // 加这行
  return request("/task/modify", {
    method: "POST",
    body: JSON.stringify({ uuid, command }),
  });
}

/** 将任务标记为完成 */
export async function doneTask(uuid) {
  return request("/task/done", {
    method: "POST",
    body: JSON.stringify({ uuid }),
  });
}

/** 将任务标记为进行中 */
export async function startTask(uuid) {
  return request("/task/start", {
    method: "POST",
    body: JSON.stringify({ uuid }),
  });
}

/** 删除任务 */
export async function deleteTask(uuid) {
  return request("/task/delete", {
    method: "POST",
    body: JSON.stringify({ uuid }),
  });
}
