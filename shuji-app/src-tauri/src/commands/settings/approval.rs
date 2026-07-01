//! Approval mode config (get/set).

use crate::commands::friendly_error::friendly_error;
use serde::{Deserialize, Serialize};

/// Frontend-facing approval config (persisted in `config.local.toml`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalConfigDto {
    pub mode: String,
    pub auto_retries: u32,
}

/// Read current approval mode from in-memory runtime config.
#[tauri::command]
pub async fn get_approval_config(
    state: tauri::State<'_, crate::commands::project::AppState>,
) -> Result<ApprovalConfigDto, String> {
    let cfg = state.runtime_config.read().map_err(friendly_error)?;
    Ok(ApprovalConfigDto {
        mode: match cfg.approval.mode {
            crate::config::ApprovalMode::Manual => "manual".to_string(),
            crate::config::ApprovalMode::Auto => "auto".to_string(),
        },
        auto_retries: cfg.approval.auto_retries,
    })
}

/// Save approval mode to `config.local.toml` and update in-memory config.
#[tauri::command]
pub async fn set_approval_config(
    state: tauri::State<'_, crate::commands::project::AppState>,
    config: ApprovalConfigDto,
) -> Result<(), String> {
    let mode = match config.mode.as_str() {
        "auto" => crate::config::ApprovalMode::Auto,
        _ => crate::config::ApprovalMode::Manual,
    };
    let auto_retries = config.auto_retries.max(1);
    let approval = crate::config::ApprovalConfig { mode, auto_retries };

    {
        let mut cfg = state.runtime_config.write().map_err(friendly_error)?;
        cfg.approval = approval.clone();
    }

    crate::config::RuntimeConfig::save_approval_to_local(
        &crate::config::RuntimeConfig::config_toml_path(),
        &approval,
    )
    .map_err(friendly_error)?;

    log_console!(
        "[settings] approval mode set to {:?}, auto_retries={}",
        mode,
        auto_retries
    );
    Ok(())
}
