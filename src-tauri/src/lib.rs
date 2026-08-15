//! task-web 应用逻辑入口。
//!
//! 数据库启动检查：应用启动时立即打开一次数据库连接，
//! 确保表结构已初始化；若失败则直接终止启动（数据库不可用时
//! 应用毫无意义）。

mod commands;
mod db;
mod models;
mod settings;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 启动时确保数据库可用（建表、迁移）
    if let Err(e) = db::open() {
        eprintln!("数据库初始化失败：{}", e);
        std::process::exit(1);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // 窗口即将关闭时（无论是自绘的关闭按钮、系统标题栏、Alt+F4 还是任务栏关闭），
        // 先结束当前正在进行的计时段，避免留下一条 end 为空、之后统计时长会一直
        // 往后漂移的"僵尸"计时记录。这里直接同步操作数据库，不经过前端，
        // 确保即使前端已经无响应也能正确落盘。
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Ok(conn) = db::open() {
                    if let Err(e) = db::time_entry::stop_active(&conn) {
                        eprintln!("退出前结束计时失败：{}", e);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_tasks,
            commands::create_project,
            commands::set_project_archived,
            commands::set_project_stage,
            commands::trash_project,
            commands::restore_project,
            commands::purge_project,
            commands::move_project,
            commands::get_settings,
            commands::save_settings,
            commands::add_task,
            commands::modify_task,
            commands::done_task,
            commands::undone_task,
            commands::set_task_today,
            commands::delete_task,
            commands::start_timer,
            commands::stop_timer,
            commands::list_time_entries,
            commands::list_all_time_entries,
            commands::save_time_entry_note,
            commands::delete_time_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
