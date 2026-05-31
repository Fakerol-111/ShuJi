// 存量 clippy warning 允许项 — 逐文件消解
#![allow(clippy::type_complexity, clippy::too_many_arguments)]
#![allow(clippy::new_without_default, clippy::derivable_impls)]
#![allow(clippy::doc_lazy_continuation)]

/// Console logging via a dedicated writer task.
/// Sends formatted lines through an mpsc channel; a single background
/// task drains and writes to stderr sequentially, preventing interleaving.
#[macro_export]
macro_rules! log_console {
    ($($arg:tt)*) => {{
        $crate::logging::logger::console_send(format!($($arg)*));
    }};
}

pub mod actor;
pub mod agent;
pub mod api;
mod commands;
pub mod config;
mod logging;
pub mod models;
mod round_metrics;
mod storage;
mod token_tracker;
pub mod tool;

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
            commands::workflow::get_round_metrics,
            commands::workflow::set_document_status,
            commands::demo::create_demo_project,
            commands::settings::get_config,
            commands::settings::save_config,
            commands::settings::set_dotenv_key,
            commands::settings::get_context_config,
            commands::settings::save_context_config,
            commands::settings::reset_context_config,
            commands::settings::check_api_connection,
            commands::settings::get_workflow_preset,
            commands::settings::set_workflow_preset,
            commands::settings::get_soul_content,
            commands::settings::clear_soul,
            commands::shuji_docs::list_shuji_tree,
            commands::shuji_docs::read_shuji_doc,
            commands::checkpoint::list_checkpoints,
            commands::checkpoint::restore_checkpoint,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
