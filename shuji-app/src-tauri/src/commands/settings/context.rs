//! Context window config (get/save/reset).

use std::collections::HashMap;

use crate::commands::friendly_error::friendly_error;
use crate::config::RoleContextConfig;
use serde::{Deserialize, Serialize};

use super::paths::context_config_path;

/// Per-role context window overrides, persisted as `context_config.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContextWindowConfig {
    pub roles: HashMap<String, RoleContextConfig>,
}

/// Read per-role context window config from `context_config.json`.
#[tauri::command]
pub async fn get_context_config() -> Result<ContextWindowConfig, String> {
    let path = context_config_path();
    if let Ok(content) = tokio::fs::read_to_string(&path).await {
        if let Ok(config) = serde_json::from_str::<ContextWindowConfig>(&content) {
            return Ok(config);
        }
    }
    Ok(ContextWindowConfig::default())
}

/// Save per-role context window config to `context_config.json`.
#[tauri::command]
pub async fn save_context_config(config: ContextWindowConfig) -> Result<(), String> {
    let path = context_config_path();
    let content = serde_json::to_string_pretty(&config).map_err(friendly_error)?;
    tokio::fs::write(&path, &content)
        .await
        .map_err(friendly_error)?;
    Ok(())
}

/// Reset context_config.json to default (empty overrides).
#[tauri::command]
pub async fn reset_context_config() -> Result<(), String> {
    let path = context_config_path();
    tokio::fs::write(&path, "{}")
        .await
        .map_err(friendly_error)?;
    log_console!("[debug] reset context_config to defaults");
    Ok(())
}
