use std::collections::HashMap;

use crate::commands::friendly_error::friendly_error;

use serde::{Deserialize, Serialize};

fn dotenv_path() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".env")
}

fn dotenv_path_parent() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join(".env")
}

/// Read .env file — supports KEY=VALUE, # comments, skips blanks.
/// Checks CWD first, then parent directory.
fn load_dotenv() -> HashMap<String, String> {
    let paths = [dotenv_path(), dotenv_path_parent()];
    for path in &paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let mut vars = HashMap::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some(eq) = trimmed.find('=') {
                    let key = trimmed[..eq].trim().to_string();
                    let val = trimmed[eq + 1..].trim().to_string();
                    vars.insert(key, val);
                }
            }
            log_console!("[debug] loaded .env from {} ({} vars)", path.display(), vars.len());
            return vars;
        }
    }
    log_console!("[debug] no .env found (cwd={})",
        std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default());
    HashMap::new()
}

/// A single role's API endpoint configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleEndpoint {
    pub api_key: String,
    pub api_url: String,
    pub model: String,
}

/// Top-level config loaded entirely from .env.
/// Each role (including "default") has its own endpoint.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub roles: HashMap<String, RoleEndpoint>,
}

impl AppConfig {
    /// Get config for a specific role, falling back to "default", then a hardcoded default.
    pub fn for_role(&self, name: &str) -> RoleEndpoint {
        self.roles.get(name)
            .cloned()
            .or_else(|| self.roles.get("default").cloned())
            .unwrap_or_else(|| RoleEndpoint {
                api_key: String::new(),
                api_url: "https://api.anthropic.com/v1/messages".into(),
                model: "claude-sonnet-4-20250514".into(),
            })
    }

    /// Build AppConfig from parsed .env vars.
    fn from_dotenv(vars: &HashMap<String, String>) -> Self {
        let mut roles = HashMap::new();

        // All known roles + default
        let all_roles = ["default", "menxiashizhong", "zhongshuling", "neige", "shangshuling",
                         "libushangshu", "hubu", "liburshangshu", "bingbushangshu", "xingbushangshu", "gongbushangshu", "zhisi"];

        for role in all_roles {
            let prefix = role.to_uppercase();
            let key = vars.get(&format!("{}_API_KEY", prefix));
            let url = vars.get(&format!("{}_API_URL", prefix));
            let model = vars.get(&format!("{}_MODEL", prefix));

            // Only insert if at least one field is present
            if key.or(url).or(model).is_some() {
                roles.insert(role.to_string(), RoleEndpoint {
                    api_key: key.cloned().unwrap_or_default(),
                    api_url: url.cloned().unwrap_or_default(),
                    model: model.cloned().unwrap_or_default(),
                });
            }
        }

        Self { roles }
    }

    /// Serialize back to .env format lines.
    pub fn to_dotenv_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        // Sort keys for deterministic output
        let mut keys: Vec<&String> = self.roles.keys().collect();
        keys.sort();

        for role in keys {
            if let Some(ep) = self.roles.get(role) {
                let prefix = role.to_uppercase();
                lines.push(format!("# {}", role));
                lines.push(format!("{}_API_KEY={}", prefix, ep.api_key));
                lines.push(format!("{}_API_URL={}", prefix, ep.api_url));
                lines.push(format!("{}_MODEL={}", prefix, ep.model));
                lines.push(String::new());
            }
        }
        lines
    }
}

/// Read config from .env in the working directory (or parent).
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    let vars = load_dotenv();
    Ok(AppConfig::from_dotenv(&vars))
}

#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    let path = dotenv_path();
    let content = config.to_dotenv_lines().join("\n");
    tokio::fs::write(&path, &content).await.map_err(friendly_error)?;
    Ok(())
}

/// Set a single .env key. Updates existing key or appends new line.
#[tauri::command]
pub async fn set_dotenv_key(key: String, value: String) -> Result<(), String> {
    let path = dotenv_path();
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
        // Remove any trailing blank lines, append, then add one blank
        while lines.last().map_or(false, |l| l.trim().is_empty()) {
            lines.pop();
        }
        lines.push(format!("{}={}", key, value));
        lines.push(String::new());
    }
    tokio::fs::write(&path, lines.join("\n")).await.map_err(friendly_error)?;
    Ok(())
}
