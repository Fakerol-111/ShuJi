use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::PathBuf;

/// Simple JSON config stored next to the app
fn config_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    path.push("shuji_config.json");
    path
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub api_key: String,
    pub model: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "claude-sonnet-4-20250514".into(),
        }
    }
}

#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    let path = config_path();
    if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(AppConfig::default());
    }
    let data = tokio::fs::read_to_string(&path).await.map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    let path = config_path();
    let data = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, &data).await.map_err(|e| e.to_string())?;
    Ok(())
}
