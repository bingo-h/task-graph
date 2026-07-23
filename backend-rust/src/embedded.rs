//! 前端静态文件服务。
//!
//! 使用 rust-embed 在编译时将 `frontend/dist/` 目录嵌入二进制。
//! 这样发布时只需要一个可执行文件，不需要附带任何静态资源。
//!
//! 工作原理：
//!   - `cargo build --release` 时，rust-embed 读取 `../frontend/dist/`
//!     目录的所有文件，将其内容编码后编译进二进制
//!   - 运行时通过 `FrontendAssets::get(path)` 按路径读取文件内容
//!   - 所有非 API 路由都返回对应的静态文件，找不到时返回 index.html
//!     （支持 Vue Router 的 history 模式）
//!
//! 注意：编译前必须先运行 `cd frontend && npm run build`，
//! 否则编译会因找不到 dist 目录而失败。

use axum::{
    Router,
    body::Body,
    http::{HeaderValue, StatusCode, Uri, header},
    response::Response,
};
use rust_embed::RustEmbed;

/// 将 `frontend/dist/` 目录的所有文件嵌入二进制。
///
/// `#[folder]` 路径相对于 `Cargo.toml` 所在目录（即 `backend-rust/`）。
/// 编译时 rust-embed 会遍历该目录，将文件内容编码为静态字节数组。
#[derive(RustEmbed)]
#[folder = "../frontend/dist/"]
struct FrontendAssets;

/// 构建前端静态文件路由，挂载到根路径 `/`。
///
/// 返回的 `Router` 会处理所有非 `/api` 开头的请求：
/// - 请求路径对应的文件存在 → 返回该文件
/// - 文件不存在 → 返回 `index.html`（支持前端路由）
pub fn frontend_router() -> Router {
    Router::new().fallback(static_handler)
}

/// 静态文件处理器。
///
/// 将请求路径映射到嵌入的文件，处理以下情况：
/// - `/` → `index.html`
/// - `/assets/app.js` → 对应的 JS 文件
/// - 其他任何路径 → `index.html`（Vue Router history 模式回退）
async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // 根路径直接返回 index.html
    let path = if path.is_empty() { "index.html" } else { path };

    match FrontendAssets::get(path) {
        Some(content) => {
            // 根据文件扩展名推断 Content-Type
            let mime = mime_type(path);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static(mime))
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        // 文件不存在时返回 index.html，支持 Vue Router history 模式
        None => match FrontendAssets::get("index.html") {
            Some(content) => Response::builder()
                .status(StatusCode::OK)
                .header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                )
                .body(Body::from(content.data.into_owned()))
                .unwrap(),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("前端文件未找到，请先运行 npm run build"))
                .unwrap(),
        },
    }
}

/// 根据文件扩展名返回对应的 MIME 类型。
fn mime_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}
