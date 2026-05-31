//! 全局常量。
//!
//! 所有跨模块共享的固定值在此定义。
//!
//! 注意：`INBOX_PROJECT` 是前后端接口契约的一部分，
//! 修改时必须同步修改前端 `frontend/src/constants.js` 中的 `INBOX_PROJECT`。

/// 无项目归属任务的虚拟项目路径标识符。
pub const INBOX_PROJECT: &str = "(无项目)";

/// 后端监听端口。
pub const DEFAULT_PORT: u16 = 8765;
