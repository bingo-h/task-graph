//! task-web Rust 后端服务入口。
//!
//! 启动方式：
//!   cargo run --release
//!
//! 默认监听 http://localhost:8765
//!
//! 前端静态文件处理策略：
//!   - 编译时若 `../frontend/dist` 存在，用 rust-embed 嵌入二进制（生产模式）
//!   - 若编译时不存在，退回到运行时从文件系统读取（开发模式）
//!   - 两种模式对前端完全透明，访问 http://localhost:8765 即可

mod api;
mod constants;
mod data;
mod embedded;
mod project;
mod task;

use axum::{routing::{get, post}, Router};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

#[tokio::main]
async fn main() {
    // 初始化日志，格式简洁（不带模块路径）
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    // ── API 路由 ──────────────────────────────────────────────────────────────
    let api_router = Router::new()
        .route("/tasks",       get(api::get_tasks))
        .route("/task/add",    post(api::add_task))
        .route("/task/modify", post(api::modify_task))
        .route("/task/done",   post(api::done_task))
        .route("/task/delete", post(api::delete_task))
        .route("/task/start",  post(api::start_task))
        .route("/task/stop",   post(api::stop_task));

    // ── CORS（允许 Vite 开发服务器 localhost:5173 跨域访问）──────────────────
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // ── 前端静态文件路由 ──────────────────────────────────────────────────────
    // embedded 模块根据编译时是否存在 frontend/dist 决定使用哪种策略：
    //   - 嵌入模式：文件编译进二进制，单文件分发
    //   - 文件系统模式：从磁盘读取，用于开发调试
    let frontend_router = embedded::frontend_router();

    let app = Router::new()
        .nest("/api", api_router)
        .merge(frontend_router)
        .layer(cors);

    // ── 启动 ──────────────────────────────────────────────────────────────────
    let addr = format!("0.0.0.0:{}", constants::DEFAULT_PORT);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("task-web 启动：http://localhost:{}", constants::DEFAULT_PORT);

    axum::serve(listener, app).await.unwrap();
}
