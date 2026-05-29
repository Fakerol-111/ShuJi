/// Console logging via a dedicated writer task.
/// Sends formatted lines through an mpsc channel; a single background
/// task drains and writes to stderr sequentially, preventing interleaving.
#[macro_export]
macro_rules! log_console {
    ($($arg:tt)*) => {{
        $crate::logging::logger::console_send(format!($($arg)*));
    }};
}

mod models;
pub mod agent;
mod storage;
mod logging;
pub mod api;
mod commands;
mod token_tracker;
pub mod actor;
pub mod tool;
pub mod config;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;

use commands::project::AppState;
use config::RuntimeConfig;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime_config = RuntimeConfig::load_or_default("config.toml");
    
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
            runtime_config: Arc::new(runtime_config),
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
            commands::workflow::get_context_stats,
            commands::workflow::cancel_processing,
            commands::workflow::get_chat_history,
            commands::workflow::get_dept_logs,
            commands::settings::get_config,
            commands::settings::save_config,
            commands::settings::set_dotenv_key,
            commands::shuji_docs::list_shuji_tree,
            commands::shuji_docs::read_shuji_doc,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
