//! Model preset system (derive_cheap_strong, apply, get/set).

use crate::commands::friendly_error::{friendly_error, friendly_error_plain};

use super::api_config::{update_cache, AppConfig, RoleEndpoint};
use super::paths::api_config_path;

/// Derive "cheap" and "strong" model names from the user's default model.
fn derive_cheap_strong(default_model: &str) -> (String, String) {
    match default_model {
        "deepseek-v4-flash" => ("deepseek-v4-flash".into(), "deepseek-4-pro".into()),
        "deepseek-4-pro" => ("deepseek-v4-flash".into(), "deepseek-4-pro".into()),
        "claude-sonnet-4-20250514" => {
            ("claude-haiku-4-5-20251001".into(), "claude-opus-4-7".into())
        }
        "claude-haiku-4-5-20251001" => (
            "claude-haiku-4-5-20251001".into(),
            "claude-sonnet-4-20250514".into(),
        ),
        "gpt-4o" => ("gpt-4o-mini".into(), "gpt-4o".into()),
        "gpt-4o-mini" => ("gpt-4o-mini".into(), "gpt-4o".into()),
        _ => (default_model.into(), default_model.into()),
    }
}

const ECONOMY_ROLES: &[(&str, &str)] = &[
    ("menxiashizhong", "cheap"),
    ("xingbushangshu", "cheap"),
    ("liburshangshu", "cheap"),
    ("zhongshuling", "default"),
    ("gongbushangshu", "default"),
    ("libushangshu", "default"),
];

const QUALITY_ROLES: &[(&str, &str)] = &[
    ("zhongshuling", "strong"),
    ("gongbushangshu", "strong"),
    ("libushangshu", "strong"),
];

/// Apply a model preset to `config`, updating per-role model fields.
pub fn apply_model_preset(config: &mut AppConfig, preset: &str) {
    let default = match config.roles.get("default") {
        Some(d) => d.clone(),
        None => return,
    };
    if default.model.is_empty() {
        return;
    }

    let (cheap, strong) = derive_cheap_strong(&default.model);

    let map: &[(&str, &str)] = match preset {
        "economy" => ECONOMY_ROLES,
        "quality" => QUALITY_ROLES,
        _ => &[],
    };

    if preset == "balanced" {
        config.roles.retain(|k, v| {
            if k == "default" {
                return true;
            }
            if v.api_key == default.api_key && v.api_url == default.api_url {
                return false;
            }
            v.model = default.model.clone();
            true
        });
    } else {
        for (role_name, tier) in map {
            let model = match *tier {
                "cheap" => cheap.clone(),
                "strong" => strong.clone(),
                _ => default.model.clone(),
            };
            config
                .roles
                .entry(role_name.to_string())
                .and_modify(|r| r.model = model.clone())
                .or_insert(RoleEndpoint {
                    api_key: default.api_key.clone(),
                    api_url: default.api_url.clone(),
                    model,
                });
        }
    }

    config.preset = Some(preset.to_string());
}

/// Get the current model preset name.
#[tauri::command]
pub async fn get_model_preset() -> Result<String, String> {
    let config = super::api_config::get_config().await?;
    Ok(config.preset.unwrap_or_else(|| "balanced".to_string()))
}

/// Set the model preset and apply role model mapping.
#[tauri::command]
pub async fn set_model_preset(preset: String) -> Result<(), String> {
    if !["economy", "balanced", "quality", "custom"].contains(&preset.as_str()) {
        return Err(friendly_error_plain(format!(
            "invalid preset: {}. Options: economy, balanced, quality",
            preset
        )));
    }

    let mut config = super::api_config::get_config().await?;
    apply_model_preset(&mut config, &preset);

    let path = api_config_path();
    let content = serde_json::to_string_pretty(&config).map_err(friendly_error)?;
    tokio::fs::write(&path, &content)
        .await
        .map_err(friendly_error)?;
    log_console!("[settings] model preset applied: {}", preset);
    update_cache(config);
    Ok(())
}
