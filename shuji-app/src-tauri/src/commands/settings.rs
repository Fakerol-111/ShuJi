use std::collections::HashMap;

use crate::commands::friendly_error::friendly_error;

use serde::{Deserialize, Serialize};

// ── Path helpers ───────────────────────────────────────────

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

fn api_config_path() -> std::path::PathBuf {
    // Same resolution logic as dotenv_path: check CWD then parent
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let candidate = cwd.join("api_config.json");
    if candidate.exists() {
        return candidate;
    }
    if let Some(parent) = cwd.parent() {
        let parent_candidate = parent.join("api_config.json");
        if parent_candidate.exists() {
            return parent_candidate;
        }
    }
    // Default: write to CWD
    cwd.join("api_config.json")
}

// ── Mappings: short env prefix ↔ canonical role name ──────

/// (short_prefix, canonical_role_name) — maps .env.template short names
/// to the canonical names used by `for_role()` and `build_agents()`.
const ROLE_PREFIXES: &[(&str, &str)] = &[
    ("DEFAULT",  "default"),
    ("MENXIA",   "menxiashizhong"),
    ("ZHONGSHU", "zhongshuling"),
    ("NEIGE",    "neige"),
    ("SHANGSHU", "shangshuling"),
    ("LIBUP",    "libushangshu"),
    ("HUBU",     "hubu"),
    ("LIBUR",    "liburshangshu"),
    ("BINGBU",   "bingbushangshu"),
    ("XINGBU",   "xingbushangshu"),
    ("GONGBU",   "gongbushangshu"),
    ("ZHISI",    "zhisi"),
];

fn prefix_for_role(role_name: &str) -> &str {
    for (prefix, name) in ROLE_PREFIXES {
        if *name == role_name { return prefix; }
    }
    role_name // fallback: use as-is
}

// ── Data structures ───────────────────────────────────────

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
}

// ── Dotenv parsing (backward compatibility) ────────────────

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

/// Build AppConfig from parsed .env vars using the short-prefix mapping.
fn app_config_from_dotenv(vars: &HashMap<String, String>) -> AppConfig {
    let mut roles = HashMap::new();
    for (short_prefix, role_name) in ROLE_PREFIXES {
        let api_key = vars.get(&format!("{}_API_KEY", short_prefix));
        let api_url = vars.get(&format!("{}_API_URL", short_prefix));
        let model = vars.get(&format!("{}_MODEL", short_prefix));
        if api_key.or(api_url).or(model).is_some() {
            roles.insert(role_name.to_string(), RoleEndpoint {
                api_key: api_key.cloned().unwrap_or_default(),
                api_url: api_url.cloned().unwrap_or_default(),
                model: model.cloned().unwrap_or_default(),
            });
        }
    }
    AppConfig { roles }
}

/// Serialize back to .env format lines (backward compat, used by set_dotenv_key area).
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

// ── Tauri commands ────────────────────────────────────────

/// Read config from `api_config.json`, falling back to `.env` with auto-migration.
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    let json_path = api_config_path();

    // 1. Try JSON first
    if let Ok(content) = tokio::fs::read_to_string(&json_path).await {
        if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
            log_console!("[debug] loaded api_config from {}", json_path.display());
            return Ok(config);
        }
        log_console!("[warn] corrupted api_config.json, falling back to .env");
    }

    // 2. Fall back to .env
    let vars = load_dotenv();
    let config = app_config_from_dotenv(&vars);

    // 3. Auto-migrate to JSON if there's meaningful per-role data
    let has_role_data = config.roles.iter().any(|(k, v)| {
        k != "default" && (!v.api_key.is_empty() || !v.api_url.is_empty() || !v.model.is_empty())
    });
    if has_role_data {
        let json = serde_json::to_string_pretty(&config).map_err(friendly_error)?;
        let _ = tokio::fs::write(&json_path, &json).await;
        log_console!("[debug] migrated .env → {}", json_path.display());
    }

    Ok(config)
}

/// Save config to `api_config.json` (does NOT touch `.env`).
#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    let path = api_config_path();
    let content = serde_json::to_string_pretty(&config).map_err(friendly_error)?;
    tokio::fs::write(&path, &content).await.map_err(friendly_error)?;
    log_console!("[debug] saved api_config to {}", path.display());
    Ok(())
}

/// Set a single .env key. Updates existing key or appends new line.
/// Still writes to `.env` (for PARTICIPATION_LEVEL etc., not API config).
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
        while lines.last().map_or(false, |l| l.trim().is_empty()) {
            lines.pop();
        }
        lines.push(format!("{}={}", key, value));
        lines.push(String::new());
    }
    tokio::fs::write(&path, lines.join("\n")).await.map_err(friendly_error)?;
    Ok(())
}
