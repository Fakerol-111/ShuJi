/// Synchronized console logging macro.
/// Holds CONSOLE_LOCK to prevent interleaved output from concurrent actors.
/// Uses write! with explicit \n to avoid Windows CRLF pipe corruption.
#[macro_export]
macro_rules! log_console {
    ($($arg:tt)*) => {{
        let _lock = $crate::logging::logger::CONSOLE_LOCK.lock().unwrap();
        use std::io::Write;
        let _ = write!(std::io::stderr(), "{}\n", format!($($arg)*));
    }};
}

mod models;
mod agent;
mod storage;
mod logging;
mod api;
mod commands;
mod token_tracker;
mod actor;
pub mod tool;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;

use commands::project::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            current_project: Arc::new(Mutex::new(None)),
            current_dir: Arc::new(Mutex::new(None)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            actor_system: Arc::new(tokio::sync::Mutex::new(None)),
            chat_history: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            dept_log_history: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        })
        .invoke_handler(tauri::generate_handler![
            commands::project::create_project,
            commands::project::load_project,
            commands::project::get_project,
            commands::project::list_projects,
            commands::workflow::send_message,
            commands::workflow::discuss_with_cabinet,
            commands::workflow::get_snapshot,
            commands::workflow::read_document,
            commands::workflow::list_documents,
            commands::workflow::list_log_files,
            commands::workflow::read_log_file,
            commands::workflow::get_recent_dirs,
            commands::workflow::get_token_stats,
            commands::workflow::cancel_processing,
            commands::workflow::get_chat_history,
            commands::workflow::get_dept_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
