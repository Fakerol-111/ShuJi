use tauri::State;
use std::collections::HashMap;

use crate::commands::project::AppState;
use crate::orchestrator::engine::{WorkflowEngine, ProjectSnapshot};
use crate::models::chat::{ChatMessage, ChatResponse};
use crate::agent::mock::MockAgent;
use crate::agent::r#trait::Agent;
use crate::models::role::Role;
use crate::storage::shuji_dir::ShujiDir;

fn build_mock_agents() -> HashMap<Role, Box<dyn Agent>> {
    let mut agents: HashMap<Role, Box<dyn Agent>> = HashMap::new();
    agents.insert(Role::Zhongshu, Box::new(MockAgent::new(Role::Zhongshu)));
    agents.insert(Role::Menxia, Box::new(MockAgent::new(Role::Menxia).with_rates(0.3, 0.0)));
    agents.insert(Role::Neige, Box::new(MockAgent::new(Role::Neige)));
    agents.insert(Role::Shangshu, Box::new(MockAgent::new(Role::Shangshu)));
    agents.insert(Role::LiBuP, Box::new(MockAgent::new(Role::LiBuP)));
    agents.insert(Role::Hubu, Box::new(MockAgent::new(Role::Hubu)));
    agents.insert(Role::LiBuR, Box::new(MockAgent::new(Role::LiBuR)));
    agents.insert(Role::Bingbu, Box::new(MockAgent::new(Role::Bingbu).with_rates(0.0, 0.15)));
    agents.insert(Role::Xingbu, Box::new(MockAgent::new(Role::Xingbu)));
    agents.insert(Role::Gongbu, Box::new(MockAgent::new(Role::Gongbu)));
    agents.insert(Role::Zhisi, Box::new(MockAgent::new(Role::Zhisi)));
    agents
}

/// Single entry point for all emperor interactions.
/// Input: emperor's message (a new goal, a decision choice, or a question).
/// Output: conversation messages + project snapshot.
#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    message: String,
) -> Result<ChatResponse, String> {
    // Lock in consistent order: engine → project
    let mut engine = state.engine.lock().await;
    let mut project_opt = state.current_project.lock().await;
    let project = project_opt.as_mut().ok_or("没有加载项目")?;
    let working_dir = project.working_dir.clone();

    // Rebuild engine with fresh agents + shuji_dir
    let agents = build_mock_agents();
    let shuji_dir = ShujiDir::new(&working_dir);
    *engine = WorkflowEngine::new(agents, shuji_dir, project.phase_count);

    let chat_messages = engine.process_and_advance(project, &message)
        .await
        .map_err(|e| e.to_string())?;

    let snapshot = engine.snapshot(project);

    // Save project after processing
    let s = ShujiDir::new(&working_dir);
    let _ = s.save_project(project).await;

    Ok(ChatResponse { messages: chat_messages, snapshot })
}

/// Get current snapshot (for UI refresh).
#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<ProjectSnapshot, String> {
    let engine = state.engine.lock().await;
    let project_opt = state.current_project.lock().await;
    let project = project_opt.as_ref().ok_or("没有加载项目")?;
    Ok(engine.snapshot(project))
}

// Keep the rest unchanged
#[tauri::command]
pub async fn read_document(
    state: State<'_, AppState>,
    subdir: String,
    filename: String,
) -> Result<Option<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt.as_ref().ok_or("没有加载项目")?.working_dir.clone();
    drop(project_opt);
    let shuji_dir = ShujiDir::new(&working_dir);
    shuji_dir.read_document(&subdir, &filename).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_documents(
    state: State<'_, AppState>,
    subdir: String,
) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt.as_ref().ok_or("没有加载项目")?.working_dir.clone();
    drop(project_opt);
    let shuji_dir = ShujiDir::new(&working_dir);
    shuji_dir.list_documents(&subdir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_log_files(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt.as_ref().ok_or("没有加载项目")?.working_dir.clone();
    drop(project_opt);
    let shuji_dir = ShujiDir::new(&working_dir);
    shuji_dir.list_log_files().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_log_file(state: State<'_, AppState>, filename: String) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt.as_ref().ok_or("没有加载项目")?.working_dir.clone();
    drop(project_opt);
    let shuji_dir = ShujiDir::new(&working_dir);
    shuji_dir.read_log_file(&filename).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recent_dirs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let dir = state.current_dir.lock().await;
    Ok(match dir.as_ref() {
        Some(d) => vec![d.clone()],
        None => vec![],
    })
}
