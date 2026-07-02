//! API config (AppConfig, dotenv migration, get_config, save_config).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::commands::friendly_error::friendly_error;
use crate::util::lock::lock_or_recover;
use serde::{Deserialize, Serialize};

use super::paths::{api_config_path, load_dotenv, prefix_for_role, ROLE_PREFIXES};

/// In-memory cache for AppConfig read from `api_config.json`.
static APP_CONFIG_CACHE: Mutex<Option<AppConfig>> = Mutex::new(None);

/// A single role's API endpoint configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleEndpoint {
    pub api_key: String,
    pub api_url: String,
    pub model: String,
}

/// Top-level API configuration.
/// Each role (including "default") has its own endpoint.
/// Persisted as `api_config.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    /// Model preset: "balanced" (default), "economy", "quality", or "custom".
    #[serde(default)]
    pub preset: Option<String>,
    pub roles: HashMap<String, RoleEndpoint>,
}

impl AppConfig {
    /// Get config for a specific role, falling back to "default", then a hardcoded default.
    pub fn for_role(&self, name: &str) -> RoleEndpoint {
        self.roles
            .get(name)
            .cloned()
            .or_else(|| self.roles.get("default").cloned())
            .unwrap_or_else(|| RoleEndpoint {
                api_key: String::new(),
                api_url: "https://api.anthropic.com/v1/messages".into(),
                model: "claude-sonnet-4-20250514".into(),
            })
    }
}

/// Build AppConfig from parsed .env vars using the short-prefix mapping.
fn app_config_from_dotenv(vars: &HashMap<String, String>) -> AppConfig {
    let mut roles = HashMap::new();
    for (short_prefix, role_name) in ROLE_PREFIXES {
        let api_key = vars.get(&format!("{}_API_KEY", short_prefix));
        let api_url = vars.get(&format!("{}_API_URL", short_prefix));
        let model = vars.get(&format!("{}_MODEL", short_prefix));
        if api_key.or(api_url).or(model).is_some() {
            roles.insert(
                role_name.to_string(),
                RoleEndpoint {
                    api_key: api_key.cloned().unwrap_or_default(),
                    api_url: api_url.cloned().unwrap_or_default(),
                    model: model.cloned().unwrap_or_default(),
                },
            );
        }
    }
    AppConfig {
        preset: Some("balanced".to_string()),
        roles,
    }
}

/// Serialize back to .env format lines (backward compat).
#[allow(dead_code)]
pub fn to_dotenv_lines(config: &AppConfig) -> Vec<String> {
    let mut lines = Vec::new();
    let mut keys: Vec<&String> = config.roles.keys().collect();
    keys.sort();
    for role in keys {
        if let Some(ep) = config.roles.get(role) {
            let prefix = prefix_for_role(role);
            lines.push(format!("# {}", role));
            lines.push(format!("{}_API_KEY={}", prefix, ep.api_key));
            lines.push(format!("{}_API_URL={}", prefix, ep.api_url));
            lines.push(format!("{}_MODEL={}", prefix, ep.model));
            lines.push(String::new());
        }
    }
    lines
}

/// Read config from `api_config.json`, falling back to `.env` with auto-migration.
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    if let Ok(cache) = lock_or_recover(&APP_CONFIG_CACHE) {
        if let Some(cached) = cache.as_ref() {
            return Ok(cached.clone());
        }
    }

    let json_path = api_config_path();

    if let Ok(content) = tokio::fs::read_to_string(&json_path).await {
        if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
            log_console!("[debug] loaded api_config from {}", json_path.display());
            if let Ok(mut cache) = lock_or_recover(&APP_CONFIG_CACHE) {
                *cache = Some(config.clone());
            }
            return Ok(config);
        }
        log_console!("[warn] corrupted api_config.json, falling back to .env");
    }

    let vars = load_dotenv();
    let config = app_config_from_dotenv(&vars);

    let has_role_data = config.roles.iter().any(|(k, v)| {
        k != "default" && (!v.api_key.is_empty() || !v.api_url.is_empty() || !v.model.is_empty())
    });
    if has_role_data {
        let json = serde_json::to_string_pretty(&config).map_err(friendly_error)?;
        let _ = tokio::fs::write(&json_path, &json).await;
        log_console!("[debug] migrated .env → {}", json_path.display());
    }

    if let Ok(mut cache) = lock_or_recover(&APP_CONFIG_CACHE) {
        *cache = Some(config.clone());
    }
    Ok(config)
}

/// Save config to `api_config.json` (does NOT touch `.env`).
#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    let path = api_config_path();
    let mut final_config = config;
    if final_config.preset.is_none() {
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            if let Ok(existing) = serde_json::from_str::<AppConfig>(&content) {
                final_config.preset = existing.preset;
            }
        }
    }
    let content = serde_json::to_string_pretty(&final_config).map_err(friendly_error)?;
    tokio::fs::write(&path, &content)
        .await
        .map_err(friendly_error)?;
    log_console!("[debug] saved api_config to {}", path.display());
    if let Ok(mut cache) = lock_or_recover(&APP_CONFIG_CACHE) {
        *cache = Some(final_config);
    }
    Ok(())
}

/// Set a single .env key. Updates existing key or appends new line.
#[tauri::command]
pub async fn set_dotenv_key(key: String, value: String) -> Result<(), String> {
    let path = super::paths::dotenv_path();
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let target = format!("{}=", key);
    let mut found = false;
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if trimmed.starts_with(&target) || trimmed == target.trim_end_matches('=') {
            *line = format!("{}={}", key, value);
            found = true;
            break;
        }
    }
    if !found {
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        lines.push(format!("{}={}", key, value));
        lines.push(String::new());
    }
    tokio::fs::write(&path, lines.join("\n"))
        .await
        .map_err(friendly_error)?;
    Ok(())
}

/// Update the in-memory config cache (called by model_preset after applying preset).
pub(crate) fn update_cache(config: AppConfig) {
    if let Ok(mut cache) = lock_or_recover(&APP_CONFIG_CACHE) {
        *cache = Some(config);
    }
}
