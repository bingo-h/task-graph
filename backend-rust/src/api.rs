//! API 路由处理器。
//!
//! 所有写操作通过调用 `task` CLI 实现，成功后返回最新的完整图数据，
//! 前端收到后直接替换本地状态，无需额外刷新请求。

use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::data::{load_graph_data, run_task_command};

// ── 错误类型 ──────────────────────────────────────────────────────────────────

/// API 错误，自动转换为带错误信息的 JSON 响应。
pub struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({ "detail": self.0.to_string() });
        (StatusCode::BAD_REQUEST, Json(body)).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self { ApiError(e.into()) }
}

/// 简化写操作的 Result 类型别名。
type ApiResult<T> = Result<T, ApiError>;

// ── 请求体结构 ────────────────────────────────────────────────────────────────

/// 新建任务请求。
///
/// `command` 为原生 taskwarrior 语法字符串，例如：
/// `"修复登录 bug project:work.backend due:2026-05-20 priority:H"`
#[derive(Deserialize)]
pub struct AddTaskRequest {
    pub command: String,
}

/// 修改任务请求。
#[derive(Deserialize)]
pub struct ModifyTaskRequest {
    pub uuid:    String,
    /// 修改参数，例如 `"due:2026-05-20 priority:H +urgent"`
    pub command: String,
}

/// 仅需 UUID 的请求（完成、删除、开始、停止）。
#[derive(Deserialize)]
pub struct UuidRequest {
    pub uuid: String,
}

// ── 辅助函数 ──────────────────────────────────────────────────────────────────

/// 写操作成功后返回最新图数据。
async fn updated_graph() -> ApiResult<impl IntoResponse> {
    let data = load_graph_data()?;
    Ok(Json(data))
}

// ── API 处理器 ────────────────────────────────────────────────────────────────

/// 获取所有任务数据（节点、边、项目树）。
pub async fn get_tasks() -> ApiResult<impl IntoResponse> {
    let data = load_graph_data()?;
    Ok(Json(data))
}

/// 新建任务。
///
/// `command` 按空白分词后直接传给 `task add`，支持所有 taskwarrior 修饰符。
pub async fn add_task(Json(req): Json<AddTaskRequest>) -> ApiResult<impl IntoResponse> {
    let tokens: Vec<&str> = req.command.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(ApiError(anyhow::anyhow!("命令不能为空")));
    }
    let mut args = vec!["add"];
    args.extend(tokens);
    run_task_command(&args)?;
    updated_graph().await
}

/// 修改任务。
///
/// `command` 按空白分词后传给 `task <uuid> modify`。
pub async fn modify_task(Json(req): Json<ModifyTaskRequest>) -> ApiResult<impl IntoResponse> {
    let tokens: Vec<&str> = req.command.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(ApiError(anyhow::anyhow!("修改参数不能为空")));
    }
    let mut args = vec![req.uuid.as_str(), "modify"];
    args.extend(tokens);
    run_task_command(&args)?;
    updated_graph().await
}

/// 将指定任务标记为完成。
pub async fn done_task(Json(req): Json<UuidRequest>) -> ApiResult<impl IntoResponse> {
    run_task_command(&["done", &req.uuid])?;
    updated_graph().await
}

/// 删除指定任务。
pub async fn delete_task(Json(req): Json<UuidRequest>) -> ApiResult<impl IntoResponse> {
    run_task_command(&["delete", &req.uuid])?;
    updated_graph().await
}

/// 将任务标记为进行中（active）。
pub async fn start_task(Json(req): Json<UuidRequest>) -> ApiResult<impl IntoResponse> {
    run_task_command(&["start", &req.uuid])?;
    updated_graph().await
}

/// 停止进行中的任务。
pub async fn stop_task(Json(req): Json<UuidRequest>) -> ApiResult<impl IntoResponse> {
    run_task_command(&["stop", &req.uuid])?;
    updated_graph().await
}
