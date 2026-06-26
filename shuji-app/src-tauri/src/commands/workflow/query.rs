use std::path::Path;

use tauri::State;

use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::models::chat::ChatMessage;
use crate::models::project::ProjectSnapshot;

/// Get current snapshot (for UI refresh).
#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<ProjectSnapshot, String> {
    let project_opt = state.current_project.lock().await;
    let project = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?;
    Ok(project.snapshot())
}

#[tauri::command]
pub async fn read_document(
    state: State<'_, AppState>,
    subdir: String,
    filename: String,
) -> Result<Option<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?
        .working_dir
        .clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir
        .read_document(&subdir, &filename)
        .await
        .map_err(friendly_error)
}

#[tauri::command]
pub async fn list_documents(
    state: State<'_, AppState>,
    subdir: String,
) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?
        .working_dir
        .clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir
        .list_documents(&subdir)
        .await
        .map_err(friendly_error)
}

#[tauri::command]
pub async fn list_log_files(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?
        .working_dir
        .clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir.list_log_files().await.map_err(friendly_error)
}

#[tauri::command]
pub async fn read_log_file(
    state: State<'_, AppState>,
    filename: String,
) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?
        .working_dir
        .clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir
        .read_log_file(&filename)
        .await
        .map_err(friendly_error)
}

// ── Recent directories ─────────────────────────────────────

fn recent_dirs_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".shuji")
        .join("recent_dirs.json")
}

fn load_recent_dirs() -> Vec<String> {
    let path = recent_dirs_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_recent_dirs(dirs: &[String]) {
    let path = recent_dirs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(dirs) {
        let _ = std::fs::write(&path, json);
    }
}

pub fn add_recent_dir(working_dir: &str) {
    let mut dirs = load_recent_dirs();
    dirs.retain(|d| d != working_dir);
    dirs.insert(0, working_dir.to_string());
    dirs.truncate(20);
    save_recent_dirs(&dirs);
}

#[tauri::command]
pub async fn get_recent_dirs() -> Result<Vec<String>, String> {
    Ok(load_recent_dirs())
}

/// Read tool-call log for a specific department from `.shuji/logs/tool-calls/{dept}.jsonl`.
/// Returns up to `limit` most recent entries.
#[tauri::command]
pub async fn get_tool_logs(
    state: State<'_, AppState>,
    dept: String,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?
            .working_dir
            .clone()
    };
    let log_path = std::path::Path::new(&working_dir)
        .join(".shuji")
        .join("logs")
        .join("tool-calls")
        .join(format!("{}.jsonl", dept));

    let limit = limit.unwrap_or(100);
    match tokio::fs::read_to_string(&log_path).await {
        Ok(content) => {
            let entries: Vec<serde_json::Value> = content
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect();
            let start = if entries.len() > limit {
                entries.len() - limit
            } else {
                0
            };
            Ok(entries[start..].to_vec())
        }
        Err(_) => Ok(vec![]),
    }
}

// ── Chat and log history ───────────────────────────────────

/// Get buffered chat message history (for re-sync after page navigation).
#[tauri::command]
pub async fn get_chat_history(state: State<'_, AppState>) -> Result<Vec<ChatMessage>, String> {
    let hist = state.chat_history.lock().await;
    Ok(hist.clone())
}

/// Get buffered department log history.
#[tauri::command]
pub async fn get_dept_logs(
    state: State<'_, AppState>,
) -> Result<Vec<crate::actor::DeptLogEntry>, String> {
    let hist = state.dept_log_history.lock().await;
    Ok(hist.clone())
}

// ── Approvals and metrics ──────────────────────────────────

/// Get the list of document IDs pending emperor approval (朱批).
#[tauri::command]
pub async fn get_pending_approvals(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?
            .working_dir
            .clone()
    };
    let path = std::path::Path::new(&working_dir);
    crate::tool::documents::sync_pending_approvals_cache(path)
        .await
        .map_err(|e| e.to_string())
}

/// Get the current round metrics.
#[tauri::command]
pub async fn get_round_metrics() -> Result<Option<crate::round_metrics::RoundMetricState>, String> {
    Ok(crate::round_metrics::snapshot())
}

/// Get the list of currently active departments.
#[tauri::command]
pub fn get_active_roles() -> Vec<String> {
    crate::round_metrics::get_active_roles()
}

// ── Workflow state ─────────────────────────────────────────

#[tauri::command]
pub async fn get_workflow_state(
    state: State<'_, AppState>,
) -> Result<Option<crate::workflow::WorkflowState>, String> {
    let dir = {
        let d = state.current_dir.lock().await;
        d.clone().ok_or("没有打开的项目")?
    };
    Ok(crate::workflow::WorkflowState::load_from(Path::new(&dir)).await)
}

#[tauri::command]
pub async fn get_workflow_graph(
    state: State<'_, AppState>,
) -> Result<Option<crate::workflow::WorkflowGraph>, String> {
    let dir = {
        let d = state.current_dir.lock().await;
        d.clone().ok_or("没有打开的项目")?
    };
    Ok(crate::workflow::WorkflowGraph::load_from(Path::new(&dir)).await)
}

#[tauri::command]
pub async fn list_workflow_archives(
    state: State<'_, AppState>,
) -> Result<Vec<Vec<String>>, String> {
    let dir = {
        let d = state.current_dir.lock().await;
        d.clone().ok_or("没有打开的项目")?
    };
    let archives = crate::workflow::WorkflowGraph::list_archives(Path::new(&dir)).await;
    Ok(archives.into_iter().map(|(f, l)| vec![f, l]).collect())
}

#[tauri::command]
pub async fn load_workflow_archive(
    state: State<'_, AppState>,
    filename: String,
) -> Result<Option<crate::workflow::WorkflowGraph>, String> {
    let dir = {
        let d = state.current_dir.lock().await;
        d.clone().ok_or("没有打开的项目")?
    };
    Ok(crate::workflow::WorkflowGraph::load_archive(Path::new(&dir), &filename).await)
}

/// Get the current pipeline execution status (null if no active pipeline).
#[tauri::command]
pub async fn get_pipeline_status(
    state: State<'_, AppState>,
) -> Result<Option<crate::pipeline::PlanRuntime>, String> {
    let dir = {
        let d = state.current_dir.lock().await;
        d.clone().ok_or("没有打开的项目")?
    };
    Ok(crate::pipeline::PlanRuntime::load_from(std::path::Path::new(&dir)).await)
}
