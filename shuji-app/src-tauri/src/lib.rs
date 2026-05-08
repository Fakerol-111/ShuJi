mod models;
mod state_machine;
mod agent;
mod orchestrator;
mod storage;
mod logging;
mod context;
mod api;
mod commands;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use commands::project::AppState;
use storage::shuji_dir::ShujiDir;
use orchestrator::engine::WorkflowEngine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Build initial engine with mock agents (will be replaced on project load)
    let agents = HashMap::new();
    let shuji_dir = ShujiDir::new(".");
    let engine = WorkflowEngine::new(agents, shuji_dir, 3);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            engine: Arc::new(Mutex::new(engine)),
            current_project: Arc::new(Mutex::new(None)),
            current_dir: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            commands::project::create_project,
            commands::project::load_project,
            commands::project::get_project,
            commands::project::list_projects,
            commands::workflow::send_message,
            commands::workflow::get_snapshot,
            commands::workflow::read_document,
            commands::workflow::list_documents,
            commands::workflow::list_log_files,
            commands::workflow::read_log_file,
            commands::workflow::get_recent_dirs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
