//! Workflow preset and deprecated workflow config commands.

use crate::commands::friendly_error::friendly_error;

use super::paths::workflow_preset_path;

/// Read the current workflow preset. Returns "standard" if not set.
#[tauri::command]
pub async fn get_workflow_preset(
    state: tauri::State<'_, crate::commands::project::AppState>,
) -> Result<String, String> {
    let dir = state
        .current_dir
        .lock()
        .await
        .clone()
        .ok_or("no open project")?;
    let path = workflow_preset_path(&dir);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => {
            let val: serde_json::Value = serde_json::from_str(&content).map_err(friendly_error)?;
            Ok(val["preset"].as_str().unwrap_or("standard").to_string())
        }
        Err(_) => Ok("standard".to_string()),
    }
}

/// Set the workflow preset. Valid values: full, standard, fast, audit.
#[tauri::command]
pub async fn set_workflow_preset(
    state: tauri::State<'_, crate::commands::project::AppState>,
    preset: String,
) -> Result<(), String> {
    if !matches!(preset.as_str(), "full" | "standard" | "fast" | "audit") {
        return Err(format!(
            "invalid preset: {} (options: full, standard, fast, audit)",
            preset
        ));
    }
    let dir = state
        .current_dir
        .lock()
        .await
        .clone()
        .ok_or("no open project")?;
    let path = workflow_preset_path(&dir);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(friendly_error)?;
    }
    let content = serde_json::json!({ "preset": preset });
    tokio::fs::write(
        &path,
        serde_json::to_string_pretty(&content).map_err(friendly_error)?,
    )
    .await
    .map_err(friendly_error)?;
    log_console!("[settings] workflow preset set to: {}", preset);
    Ok(())
}

/// Read the current workflow config from the project.
#[tauri::command]
pub async fn get_workflow_config(
    _state: tauri::State<'_, crate::commands::project::AppState>,
) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "deprecated": true,
        "message": "Workflow configuration has been replaced by the Pipeline engine. 内阁 now plans execution steps autonomously."
    }))
}

/// Save workflow config to the project.
#[tauri::command]
pub async fn set_workflow_config() -> Result<String, String> {
    Err("Workflow configuration has been replaced by the Pipeline engine, manual configuration is no longer supported.".to_string())
}
