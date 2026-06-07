use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::actor::DeptLogEntry;
use crate::commands::friendly_error::friendly_error;
use crate::config::RuntimeConfig;
use crate::models::chat::ChatMessage;
use crate::models::project::{OverallStatus, Project, ProjectSummary};
use crate::storage::shuji_dir::ShujiDir;

pub struct AppState {
    pub current_project: Arc<Mutex<Option<Project>>>,
    pub current_dir: Arc<Mutex<Option<String>>>,
    pub cancel_flag: Arc<AtomicBool>,
    pub actor_system: Arc<tokio::sync::Mutex<Option<crate::actor::ActorSystem>>>,
    pub chat_history: Arc<Mutex<Vec<ChatMessage>>>,
    pub dept_log_history: Arc<Mutex<Vec<DeptLogEntry>>>,
    pub runtime_config: Arc<RuntimeConfig>,
    pub compacting_roles: Arc<Mutex<HashSet<String>>>,
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    name: String,
    goal: String,
    working_dir: String,
) -> Result<Project, String> {
    let id = format!("{}_{}", Uuid::new_v4(), name.replace(' ', "_"));
    let project = Project {
        id: id.clone(),
        name,
        goal,
        working_dir: working_dir.clone(),
        state: "GoalReceived".to_string(),
        overall: OverallStatus::NotStarted,
        phases: vec![],
        phase_count: 3,
        created_at: chrono::Local::now().to_rfc3339(),
        updated_at: chrono::Local::now().to_rfc3339(),
        last_neige_msg: String::new(),
        summary: String::new(),
        talk: String::new(),
        task: String::new(),
        resume: String::new(),
        summary_prompt: String::new(),
    };

    let shuji_dir = ShujiDir::new(&working_dir);
    shuji_dir.init().await.map_err(friendly_error)?;
    shuji_dir
        .save_project(&project)
        .await
        .map_err(friendly_error)?;

    // Update state
    let mut current = state.current_project.lock().await;
    *current = Some(project.clone());
    let mut dir = state.current_dir.lock().await;
    *dir = Some(working_dir);

    Ok(project)
}

#[tauri::command]
pub async fn load_project(
    state: State<'_, AppState>,
    working_dir: String,
) -> Result<Project, String> {
    // ── Teardown old actor system before switching projects ──
    // This prevents old project actors from emitting events to the new project's UI.
    {
        let mut sys_lock = state.actor_system.lock().await;
        *sys_lock = None; // Drop triggers ActorSystem::drop() → cancel all actors
    }

    let shuji_dir = ShujiDir::new(&working_dir);
    let storage_path = Path::new(&working_dir)
        .join(".shuji")
        .join("token_records.json");
    crate::token_tracker::init(&storage_path);

    // Auto-create project if not exists
    let project = match shuji_dir.load_project().await.map_err(friendly_error)? {
        Some(p) => p,
        None => {
            let dir_name = std::path::Path::new(&working_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "项目".to_string());
            let id = format!("{}_{}", Uuid::new_v4(), dir_name.replace(' ', "_"));
            let project = Project {
                id,
                name: dir_name,
                goal: String::new(),
                working_dir: working_dir.clone(),
                state: "GoalReceived".to_string(),
                overall: OverallStatus::NotStarted,
                phases: vec![],
                phase_count: 3,
                created_at: chrono::Local::now().to_rfc3339(),
                updated_at: chrono::Local::now().to_rfc3339(),
                last_neige_msg: String::new(),
                summary: String::new(),
                talk: String::new(),
                task: String::new(),
                resume: String::new(),
                summary_prompt: String::new(),
            };
            shuji_dir.init().await.map_err(friendly_error)?;
            shuji_dir
                .save_project(&project)
                .await
                .map_err(friendly_error)?;
            project
        }
    };

    let mut current = state.current_project.lock().await;
    *current = Some(project.clone());
    let mut dir = state.current_dir.lock().await;
    *dir = Some(working_dir.clone());

    // Track in recent dirs (moved to front, persisted to ~/.shuji/recent_dirs.json)
    crate::commands::workflow::add_recent_dir(&working_dir);

    // Restore persisted chat history into buffer
    {
        let mut chat_hist = state.chat_history.lock().await;
        chat_hist.clear();
        let chat_path = Path::new(&working_dir).join(".shuji").join("chat.jsonl");
        if let Ok(data) = tokio::fs::read_to_string(&chat_path).await {
            for line in data.lines() {
                if let Ok(msg) = serde_json::from_str::<ChatMessage>(line) {
                    chat_hist.push(msg);
                }
            }
        }
    }

    // Restore persisted dept-log history into buffer
    {
        let mut dept_hist = state.dept_log_history.lock().await;
        dept_hist.clear();
        let dept_path = Path::new(&working_dir)
            .join(".shuji")
            .join("dept-log.jsonl");
        if let Ok(data) = tokio::fs::read_to_string(&dept_path).await {
            for line in data.lines() {
                if let Ok(entry) = serde_json::from_str::<DeptLogEntry>(line) {
                    dept_hist.push(entry);
                }
            }
        }
    }

    Ok(project)
}

#[tauri::command]
pub async fn get_project(state: State<'_, AppState>) -> Result<Option<Project>, String> {
    let current = state.current_project.lock().await;
    Ok(current.clone())
}

/// List projects by scanning directories that contain .shuji/state.json
/// Note: In the current architecture, ShuJi opens one directory at a time.
/// This command returns the currently loaded project as a summary list.
#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectSummary>, String> {
    let current = state.current_project.lock().await;
    Ok(match current.as_ref() {
        Some(p) => vec![ProjectSummary {
            id: p.id.clone(),
            name: p.name.clone(),
            goal: p.goal.clone(),
            working_dir: p.working_dir.clone(),
            created_at: p.created_at.clone(),
            overall_status: p.overall.label().to_string(),
            phases_status: format!("{}个阶段", p.phases.len()),
        }],
        None => vec![],
    })
}
