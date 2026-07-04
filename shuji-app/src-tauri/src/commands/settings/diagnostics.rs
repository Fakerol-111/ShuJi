//! Diagnostics commands — export effective configuration for debugging.
//!
//! Provides `export_effective_config` Tauri command that returns the merged
//! RuntimeConfig along with metadata about which config files exist.

use serde::Serialize;
use tauri::State;

use crate::commands::project::snapshot_runtime_config;
use crate::commands::project::AppState;
use crate::config::RuntimeConfig;

/// 合并后的有效配置 + 配置来源标注。
#[derive(Debug, Serialize)]
pub struct EffectiveConfig {
    /// 合并后的完整 RuntimeConfig
    pub runtime: RuntimeConfig,
    /// 配置来源标注
    pub sources: ConfigSources,
}

/// 配置文件存在性 + 覆盖信息。
#[derive(Debug, Serialize)]
pub struct ConfigSources {
    pub config_toml_exists: bool,
    pub config_local_toml_exists: bool,
    pub api_config_json_exists: bool,
    pub context_config_json_exists: bool,
    pub dotenv_exists: bool,
    pub config_toml_path: String,
    pub config_local_toml_path: String,
}

/// 导出当前有效配置（合并后）及配置文件来源信息。
#[tauri::command]
pub async fn export_effective_config(
    state: State<'_, AppState>,
) -> Result<EffectiveConfig, String> {
    let config = snapshot_runtime_config(&state.runtime_config);

    let config_toml_path = RuntimeConfig::config_toml_path();
    let config_local_toml_path = config_toml_path.with_file_name("config.local.toml");

    let api_config_path = crate::commands::settings::paths::api_config_path();
    let context_config_path = crate::commands::settings::paths::context_config_path();
    let dotenv_path = crate::commands::settings::paths::dotenv_path();

    let sources = ConfigSources {
        config_toml_exists: config_toml_path.exists(),
        config_local_toml_exists: config_local_toml_path.exists(),
        api_config_json_exists: api_config_path.exists(),
        context_config_json_exists: context_config_path.exists(),
        dotenv_exists: dotenv_path.exists(),
        config_toml_path: config_toml_path.display().to_string(),
        config_local_toml_path: config_local_toml_path.display().to_string(),
    };

    Ok(EffectiveConfig {
        runtime: (*config).clone(),
        sources,
    })
}
