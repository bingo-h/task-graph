//! task-web 应用逻辑入口。
//!
//! 数据库启动检查：应用启动时立即打开一次数据库连接，
//! 确保表结构已初始化；若失败则直接终止启动（数据库不可用时
//! 应用毫无意义）。

mod commands;
mod db;
mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 启动时确保数据库可用（建表、迁移）
    if let Err(e) = db::open() {
        eprintln!("数据库初始化失败：{}", e);
        std::process::exit(1);
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_tasks,
            commands::add_task,
            commands::modify_task,
            commands::done_task,
            commands::undone_task,
            commands::delete_task,
            commands::start_timer,
            commands::stop_timer,
            commands::list_time_entries,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
