mod commands;
mod proxy;
mod state;
mod types;
mod ytdlp;

use state::AppState;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let app_state = Arc::new(AppState::new(app.handle().clone()));
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_ytdlp,
            commands::install_ytdlp,
            commands::install_ffmpeg,
            commands::check_update,
            commands::install_update,
            commands::probe_url,
            commands::start_download,
            commands::cancel_download,
            commands::list_tasks,
            commands::get_history,
            commands::clear_history,
            commands::delete_history_item,
            commands::get_settings,
            commands::save_settings,
            commands::test_cookies,
            commands::detect_proxy,
            commands::pick_directory,
            commands::reveal_in_finder,
            commands::open_file,
            commands::open_external,
            commands::default_download_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running 飞鸟下载器");
}
