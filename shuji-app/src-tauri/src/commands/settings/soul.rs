//! Soul management commands.

use crate::commands::friendly_error::friendly_error;

async fn resolve_project_dir(
    state: &tauri::State<'_, crate::commands::project::AppState>,
) -> Result<String, String> {
    state
        .current_dir
        .lock()
        .await
        .clone()
        .ok_or_else(|| "no open project".to_string())
}

/// Read soul content for a role and scope (`project` default).
#[tauri::command]
pub async fn get_soul_content(
    state: tauri::State<'_, crate::commands::project::AppState>,
    role: Option<String>,
    scope: Option<String>,
) -> Result<String, String> {
    let dir = resolve_project_dir(&state).await?;
    let role_name = crate::learning::normalize_role_name(role.as_deref())?;
    let content = match scope.as_deref().unwrap_or("project") {
        "global" => crate::learning::SoulStore::load_global_markdown(&role_name)
            .await
            .unwrap_or_default(),
        _ => {
            crate::learning::SoulStore::read_project_soul(std::path::Path::new(&dir), &role_name)
                .await
        }
    };
    Ok(content)
}

/// Reset a role's project soul to the default template.
#[tauri::command]
pub async fn clear_soul(
    state: tauri::State<'_, crate::commands::project::AppState>,
    role: Option<String>,
    scope: Option<String>,
) -> Result<(), String> {
    let dir = resolve_project_dir(&state).await?;
    let role_name = crate::learning::normalize_role_name(role.as_deref())?;
    match scope.as_deref().unwrap_or("project") {
        "global" => {
            if let Some(path) = crate::learning::SoulStore::global_soul_path(&role_name) {
                let default = if role_name == "Neige" {
                    include_str!("../../agent/neige/soul.md")
                } else {
                    "## Experience\n\n## Lessons\n\n## Preferences\n"
                };
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(friendly_error)?;
                }
                tokio::fs::write(&path, default)
                    .await
                    .map_err(friendly_error)?;
            }
        }
        _ => {
            crate::learning::SoulStore::clear_project_soul(std::path::Path::new(&dir), &role_name)
                .await?;
        }
    }
    log_console!("[settings] soul cleared for {} ({:?})", role_name, scope);
    Ok(())
}

#[tauri::command]
pub async fn list_soul_roles() -> Result<Vec<String>, String> {
    Ok(crate::learning::SoulStore::list_soul_roles())
}
