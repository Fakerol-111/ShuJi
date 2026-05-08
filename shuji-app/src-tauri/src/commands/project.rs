use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::project::{Project, ProjectSummary, OverallStatus};
use crate::state_machine::states::ProjectState;
use crate::storage::shuji_dir::ShujiDir;
use crate::orchestrator::engine::WorkflowEngine;

pub struct AppState {
    pub engine: Arc<Mutex<WorkflowEngine>>,
    pub current_project: Arc<Mutex<Option<Project>>>,
    pub current_dir: Arc<Mutex<Option<String>>>,
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
        state: ProjectState::GoalReceived,
        overall: OverallStatus::NotStarted,
        phases: vec![],
        phase_count: 3,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let shuji_dir = ShujiDir::new(&working_dir);
    shuji_dir.init().await.map_err(|e| e.to_string())?;
    shuji_dir.save_project(&project).await.map_err(|e| e.to_string())?;

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
    let shuji_dir = ShujiDir::new(&working_dir);

    // Auto-create project if not exists
    let project = match shuji_dir.load_project().await.map_err(|e| e.to_string())? {
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
                state: ProjectState::GoalReceived,
                overall: OverallStatus::NotStarted,
                phases: vec![],
                phase_count: 3,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            shuji_dir.init().await.map_err(|e| e.to_string())?;
            shuji_dir.save_project(&project).await.map_err(|e| e.to_string())?;
            project
        }
    };

    // Update engine's shuji_dir
    {
        let mut engine = state.engine.lock().await;
        let agents = std::collections::HashMap::new();
        *engine = WorkflowEngine::new(agents, shuji_dir, 3);
    }

    let mut current = state.current_project.lock().await;
    *current = Some(project.clone());
    let mut dir = state.current_dir.lock().await;
    *dir = Some(working_dir);

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
