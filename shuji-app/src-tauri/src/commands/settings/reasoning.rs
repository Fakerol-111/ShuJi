//! Reasoning config (get/set, per-role overrides).

use std::collections::HashMap;

use crate::commands::friendly_error::friendly_error;
use crate::config::ReasoningEffort;
use serde::{Deserialize, Serialize};

/// Frontend-facing reasoning config DTO (persisted in `config.local.toml`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningConfigDto {
    pub enabled: bool,
    pub effort: String,
    pub budget_tokens: u32,
    pub roles: HashMap<String, RoleReasoningDto>,
}

/// Single role reasoning override DTO.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleReasoningDto {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub budget_tokens: Option<u32>,
}

impl From<&crate::config::ReasoningConfig> for ReasoningConfigDto {
    fn from(cfg: &crate::config::ReasoningConfig) -> Self {
        Self {
            enabled: cfg.enabled,
            effort: cfg.effort.to_string(),
            budget_tokens: cfg.budget_tokens,
            roles: cfg
                .roles
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        RoleReasoningDto {
                            enabled: v.enabled,
                            effort: v.effort.map(|e| e.to_string()),
                            budget_tokens: v.budget_tokens,
                        },
                    )
                })
                .collect(),
        }
    }
}

/// Read current reasoning config from in-memory runtime config.
#[tauri::command]
pub async fn get_reasoning_config(
    state: tauri::State<'_, crate::commands::project::AppState>,
) -> Result<ReasoningConfigDto, String> {
    let cfg = state.runtime_config.read().map_err(friendly_error)?;
    Ok(ReasoningConfigDto::from(&cfg.api.reasoning))
}

/// Save reasoning config to `config.local.toml` and update in-memory config.
#[tauri::command]
pub async fn set_reasoning_config(
    state: tauri::State<'_, crate::commands::project::AppState>,
    config: ReasoningConfigDto,
) -> Result<(), String> {
    let effort = match config.effort.as_str() {
        "none" => ReasoningEffort::None,
        "low" => ReasoningEffort::Low,
        "high" => ReasoningEffort::High,
        _ => ReasoningEffort::Medium,
    };

    let roles: HashMap<String, crate::config::RoleReasoningConfig> = config
        .roles
        .into_iter()
        .filter(|(k, _)| k.is_ascii())
        .map(|(k, v)| {
            let e = v.effort.as_deref().map(|s| match s {
                "none" => ReasoningEffort::None,
                "low" => ReasoningEffort::Low,
                "high" => ReasoningEffort::High,
                _ => ReasoningEffort::Medium,
            });
            (
                k,
                crate::config::RoleReasoningConfig {
                    enabled: v.enabled,
                    effort: e,
                    budget_tokens: v.budget_tokens,
                },
            )
        })
        .collect();

    let reasoning = crate::config::ReasoningConfig {
        enabled: config.enabled,
        effort,
        budget_tokens: config.budget_tokens,
        roles,
    };

    {
        let mut cfg = state.runtime_config.write().map_err(friendly_error)?;
        cfg.api.reasoning = reasoning.clone();
    }

    save_reasoning_to_local(&reasoning).map_err(friendly_error)?;

    let role_summary: Vec<String> = reasoning
        .roles
        .iter()
        .map(|(k, v)| {
            format!(
                "{}(enabled={},effort={})",
                k,
                v.enabled.unwrap_or(reasoning.enabled),
                v.effort
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| reasoning.effort.to_string()),
            )
        })
        .collect();
    log_console!(
        "[settings] reasoning: enabled={}, effort={}, roles={:?}",
        reasoning.enabled,
        reasoning.effort,
        role_summary,
    );
    Ok(())
}

/// Save reasoning config to `config.local.toml` (merge `[api.reasoning]` section only).
fn save_reasoning_to_local(reasoning: &crate::config::ReasoningConfig) -> anyhow::Result<()> {
    let local_path =
        crate::config::RuntimeConfig::config_toml_path().with_file_name("config.local.toml");
    let mut doc: toml::Value = if local_path.exists() {
        let content = std::fs::read_to_string(&local_path)?;
        toml::from_str(&content).unwrap_or(toml::Value::Table(toml::map::Map::new()))
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    let reasoning_value = toml::Value::try_from(reasoning.clone())?;

    if let toml::Value::Table(ref mut table) = doc {
        let api_table = table
            .entry("api".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if let toml::Value::Table(ref mut api) = api_table {
            api.insert("reasoning".to_string(), reasoning_value);
        }
    }
    std::fs::write(&local_path, toml::to_string_pretty(&doc)?)?;
    Ok(())
}
