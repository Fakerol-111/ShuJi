// 存量 clippy warning 允许项 — 逐文件消解（已全部消解完毕）

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
pub mod audit;
mod commands;
pub mod config;
mod logging;
pub mod models;
mod round_metrics;
mod storage;
mod token_tracker;
pub mod tool;
pub mod workflow;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared type alias for the complex cancel-flag map used by agents and tools.
/// 内阁 uses this to interrupt other agents via the `cancel_agent` tool.
pub type CancelMap = Arc<std::sync::Mutex<HashMap<crate::models::role::Role, Arc<AtomicBool>>>>;
pub type FastTxMap = Arc<
    HashMap<
        crate::models::role::Role,
        tokio::sync::mpsc::UnboundedSender<crate::actor::FastMessage>,
    >,
>;

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
            compacting_roles: Arc::new(Mutex::new(HashSet::new())),
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
            commands::workflow::compact_context,
            commands::workflow::cancel_processing,
            commands::workflow::get_chat_history,
            commands::workflow::get_dept_logs,
            commands::workflow::get_round_metrics,
            commands::workflow::get_active_roles,
            commands::workflow::get_pending_approvals,
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
            commands::settings::get_workflow_config,
            commands::settings::set_workflow_config,
            commands::settings::get_soul_content,
            commands::settings::clear_soul,
            commands::settings::get_model_preset,
            commands::settings::set_model_preset,
            commands::shuji_docs::list_shuji_tree,
            commands::shuji_docs::read_shuji_doc,
            commands::shuji_docs::get_document_diff,
            commands::checkpoint::list_checkpoints,
            commands::checkpoint::restore_checkpoint,
            commands::workflow::get_document_lineage,
            commands::workflow::get_audit_timeline,
            commands::workflow::generate_delivery_report,
            commands::workflow::get_document_diffs,
            commands::workflow::read_document_diff,
            commands::workflow::trace_document,
            commands::workflow::get_workflow_state,
            commands::workflow::get_workflow_graph,
            commands::workflow::list_workflow_archives,
            commands::workflow::load_workflow_archive,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
