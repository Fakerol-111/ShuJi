//! Global learning candidate and config commands.

#[tauri::command]
pub async fn list_global_learning_candidates() -> Result<Vec<crate::learning::LearningEntry>, String>
{
    crate::learning::SoulStore::list_global_candidates().await
}

#[tauri::command]
pub async fn approve_global_learning(candidate_id: String) -> Result<(), String> {
    crate::learning::SoulStore::approve_global_candidate(&candidate_id).await
}

#[tauri::command]
pub async fn reject_global_learning(candidate_id: String) -> Result<(), String> {
    crate::learning::SoulStore::reject_global_candidate(&candidate_id).await
}

#[tauri::command]
pub async fn get_learning_config() -> Result<serde_json::Value, String> {
    let cfg = crate::learning::SoulStore::config();
    Ok(serde_json::json!({
        "project_enabled": cfg.project_enabled,
        "global_enabled": cfg.global_enabled,
        "max_injected_chars_per_role": cfg.max_injected_chars_per_role,
        "auto_extract": cfg.auto_extract,
        "global_requires_approval": cfg.global_requires_approval,
    }))
}

#[tauri::command]
pub async fn set_learning_global_enabled(enabled: bool) -> Result<(), String> {
    crate::learning::set_global_enabled(enabled)?;
    log_console!("[settings] global learning enabled={}", enabled);
    Ok(())
}
