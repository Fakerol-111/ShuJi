use std::collections::HashMap;
use std::sync::Mutex;

use crate::commands::friendly_error::friendly_error;
use crate::config::RoleContextConfig;

use serde::{Deserialize, Serialize};

/// In-memory cache for AppConfig read from `api_config.json`.
/// Populated on first `get_config()` call, updated on `save_config()` / `set_model_preset()`.
/// Ensures that simply opening the settings page (which reads config) does not cause
/// disk I/O that might interfere with a running workflow, and that config changes
/// only take effect for the NEXT actor system initialization (not the current run).
static APP_CONFIG_CACHE: Mutex<Option<AppConfig>> = Mutex::new(None);

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
    ("DEFAULT", "default"),
    ("MENXIA", "menxiashizhong"),
    ("ZHONGSHU", "zhongshuling"),
    ("NEIGE", "neige"),
    ("SHANGSHU", "shangshuling"),
    ("LIBUP", "libushangshu"),
    ("HUBU", "hubu"),
    ("LIBUR", "liburshangshu"),
    ("BINGBU", "bingbushangshu"),
    ("XINGBU", "xingbushangshu"),
    ("GONGBU", "gongbushangshu"),
];

fn prefix_for_role(role_name: &str) -> &str {
    for (prefix, name) in ROLE_PREFIXES {
        if *name == role_name {
            return prefix;
        }
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
            log_console!(
                "[debug] loaded .env from {} ({} vars)",
                path.display(),
                vars.len()
            );
            return vars;
        }
    }
    log_console!(
        "[debug] no .env found (cwd={})",
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
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
/// Results are cached in `APP_CONFIG_CACHE` so that repeated calls (e.g. settings page
/// mount, every `send_message`) do not repeatedly hit the filesystem.
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    // Return cached value if available
    if let Some(cached) = APP_CONFIG_CACHE.lock().unwrap().as_ref() {
        return Ok(cached.clone());
    }

    let json_path = api_config_path();

    // 1. Try JSON first
    if let Ok(content) = tokio::fs::read_to_string(&json_path).await {
        if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
            log_console!("[debug] loaded api_config from {}", json_path.display());
            *APP_CONFIG_CACHE.lock().unwrap() = Some(config.clone());
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

    *APP_CONFIG_CACHE.lock().unwrap() = Some(config.clone());
    Ok(config)
}

/// Save config to `api_config.json` (does NOT touch `.env`).
/// Preserves `preset` field from existing config if not set in incoming config.
/// Updates in-memory cache so subsequent `get_config()` calls return the new config
/// for the NEXT actor system initialization (existing running actors are unaffected).
#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    let path = api_config_path();
    let mut final_config = config;
    if final_config.preset.is_none() {
        // Preserve existing preset from disk
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
    *APP_CONFIG_CACHE.lock().unwrap() = Some(final_config);
    Ok(())
}

// ── Model preset system ────────────────────────────────────────

/// Derive "cheap" and "strong" model names from the user's default model.
/// Falls back to `default_model` for both when the model family is unknown.
fn derive_cheap_strong(default_model: &str) -> (String, String) {
    match default_model {
        // DeepSeek
        "deepseek-v4-flash" => ("deepseek-v4-flash".into(), "deepseek-4-pro".into()),
        "deepseek-4-pro" => ("deepseek-v4-flash".into(), "deepseek-4-pro".into()),
        // Anthropic
        "claude-sonnet-4-20250514" => {
            ("claude-haiku-4-5-20251001".into(), "claude-opus-4-7".into())
        }
        "claude-haiku-4-5-20251001" => (
            "claude-haiku-4-5-20251001".into(),
            "claude-sonnet-4-20250514".into(),
        ),
        // OpenAI
        "gpt-4o" => ("gpt-4o-mini".into(), "gpt-4o".into()),
        "gpt-4o-mini" => ("gpt-4o-mini".into(), "gpt-4o".into()),
        // Unknown family — cheap = strong = default
        _ => (default_model.into(), default_model.into()),
    }
}

/// Role-to-tier mapping for "economy" preset.
/// Roles not listed inherit the default model.
const ECONOMY_ROLES: &[(&str, &str)] = &[
    ("menxiashizhong", "cheap"),   // 审查
    ("xingbushangshu", "cheap"),   // 检查
    ("liburshangshu", "cheap"),    // 规范
    ("zhongshuling", "default"),   // 设计 — uses default model
    ("gongbushangshu", "default"), // 编码
    ("libushangshu", "default"),   // 详细设计
];

/// Role-to-tier mapping for "quality" preset.
const QUALITY_ROLES: &[(&str, &str)] = &[
    ("zhongshuling", "strong"),   // 方案设计
    ("gongbushangshu", "strong"), // 编码实现
    ("libushangshu", "strong"),   // 详细设计
];

/// Apply a model preset to `config`, updating per-role model fields.
///
/// The mapping table is the single source of truth for which roles
/// receive which model tier. The frontend only sends the preset name.
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
        _ => &[], // balanced: no role overrides
    };

    if preset == "balanced" {
        // Remove model-only overrides; reset model on retained custom-API roles
        config.roles.retain(|k, v| {
            if k == "default" {
                return true;
            }
            if v.api_key == default.api_key && v.api_url == default.api_url {
                return false; // Remove pure-model overrides
            }
            v.model = default.model.clone(); // Reset to default model
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
    let config = get_config().await?;
    Ok(config.preset.unwrap_or_else(|| "balanced".to_string()))
}

/// Set the model preset and apply role model mapping.
/// Valid presets: "economy", "balanced", "quality".
#[tauri::command]
pub async fn set_model_preset(preset: String) -> Result<(), String> {
    if !["economy", "balanced", "quality", "custom"].contains(&preset.as_str()) {
        return Err(friendly_error(format!(
            "invalid preset: {}. Options: economy, balanced, quality",
            preset
        )));
    }

    let mut config = get_config().await?;
    apply_model_preset(&mut config, &preset);

    let path = api_config_path();
    let content = serde_json::to_string_pretty(&config).map_err(friendly_error)?;
    tokio::fs::write(&path, &content)
        .await
        .map_err(friendly_error)?;
    log_console!("[settings] model preset applied: {}", preset);
    *APP_CONFIG_CACHE.lock().unwrap() = Some(config);
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

// ── Context window config ───────────────────────────────

fn context_config_path() -> std::path::PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let candidate = cwd.join("context_config.json");
    if candidate.exists() {
        return candidate;
    }
    if let Some(parent) = cwd.parent() {
        let parent_candidate = parent.join("context_config.json");
        if parent_candidate.exists() {
            return parent_candidate;
        }
    }
    cwd.join("context_config.json")
}

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

/// Probe the given API endpoint with a minimal chat request.
/// Returns "ok" on success, or a translated error on failure.
/// Timeout is 10 seconds — designed for Settings/SetupPage health check.
#[tauri::command]
pub async fn check_api_connection(
    api_key: String,
    api_url: String,
    model: String,
) -> Result<String, String> {
    use crate::api::client::AnthropicClient;
    use crate::models::message::Message;
    use std::time::Duration;
    use tokio::time::timeout;

    let client = AnthropicClient::new(api_key, api_url.clone());

    let msg = Message::user("ping");
    let result = timeout(
        Duration::from_secs(10),
        client.send_message("respond with pong", &[msg], &model),
    )
    .await;

    match result {
        Ok(Ok(_response)) => Ok("ok".into()),
        Ok(Err(e)) => Err(friendly_error(e)),
        Err(_) => Err(
            "connection timed out (10s), please check the API URL and network connection"
                .to_string(),
        ),
    }
}

// ── Workflow preset ─────────────────────────────────────────

/// Resolve the workflow_preset file path relative to a project directory.
fn workflow_preset_path(project_dir: &str) -> std::path::PathBuf {
    std::path::Path::new(project_dir)
        .join(".shuji")
        .join("workflow_preset.json")
}

/// Read the current workflow preset. Returns "standard" if not set.
#[tauri::command]
pub async fn get_workflow_preset(
    state: tauri::State<'_, crate::commands::project::AppState>,
) -> Result<String, String> {
    let dir = state
        .current_dir
        .lock()
        .await
        .clone()
        .ok_or("no open project")?;
    let path = workflow_preset_path(&dir);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => {
            let val: serde_json::Value = serde_json::from_str(&content).map_err(friendly_error)?;
            Ok(val["preset"].as_str().unwrap_or("standard").to_string())
        }
        Err(_) => Ok("standard".to_string()),
    }
}

/// Set the workflow preset. Valid values: full, standard, fast, audit.
#[tauri::command]
pub async fn set_workflow_preset(
    state: tauri::State<'_, crate::commands::project::AppState>,
    preset: String,
) -> Result<(), String> {
    if !matches!(preset.as_str(), "full" | "standard" | "fast" | "audit") {
        return Err(format!(
            "invalid preset: {} (options: full, standard, fast, audit)",
            preset
        ));
    }
    let dir = state
        .current_dir
        .lock()
        .await
        .clone()
        .ok_or("no open project")?;
    let path = workflow_preset_path(&dir);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(friendly_error)?;
    }
    let content = serde_json::json!({ "preset": preset });
    tokio::fs::write(
        &path,
        serde_json::to_string_pretty(&content).map_err(friendly_error)?,
    )
    .await
    .map_err(friendly_error)?;
    log_console!("[settings] workflow preset set to: {}", preset);
    Ok(())
}

// ── Workflow config (deprecated — pipeline engine replaces Intent × Governance) ──

/// Read the current workflow config from the project.
#[tauri::command]
pub async fn get_workflow_config(
    _state: tauri::State<'_, crate::commands::project::AppState>,
) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "deprecated": true,
        "message": "Workflow configuration has been replaced by the Pipeline engine. 内阁 now plans execution steps autonomously."
    }))
}

/// Save workflow config to the project.
#[tauri::command]
pub async fn set_workflow_config() -> Result<String, String> {
    Err("Workflow configuration has been replaced by the Pipeline engine, manual configuration is no longer supported.".to_string())
}

// ── Soul management ─────────────────────────────────────────────────

async fn resolve_project_dir(
    state: &tauri::State<'_, crate::commands::project::AppState>,
) -> Result<String, String> {
    state
        .current_dir
        .lock()
        .await
        .clone()
        .ok_or_else(|| "no open project".to_string())
}

/// Read soul content for a role and scope (`project` default).
#[tauri::command]
pub async fn get_soul_content(
    state: tauri::State<'_, crate::commands::project::AppState>,
    role: Option<String>,
    scope: Option<String>,
) -> Result<String, String> {
    let dir = resolve_project_dir(&state).await?;
    let role_name = crate::learning::normalize_role_name(role.as_deref())?;
    let content = match scope.as_deref().unwrap_or("project") {
        "global" => crate::learning::SoulStore::load_global_markdown(&role_name)
            .await
            .unwrap_or_default(),
        _ => {
            crate::learning::SoulStore::read_project_soul(std::path::Path::new(&dir), &role_name)
                .await
        }
    };
    Ok(content)
}

/// Reset a role's project soul to the default template.
#[tauri::command]
pub async fn clear_soul(
    state: tauri::State<'_, crate::commands::project::AppState>,
    role: Option<String>,
    scope: Option<String>,
) -> Result<(), String> {
    let dir = resolve_project_dir(&state).await?;
    let role_name = crate::learning::normalize_role_name(role.as_deref())?;
    match scope.as_deref().unwrap_or("project") {
        "global" => {
            if let Some(path) = crate::learning::SoulStore::global_soul_path(&role_name) {
                let default = if role_name == "Neige" {
                    include_str!("../agent/neige/soul.md")
                } else {
                    "## Experience\n\n## Lessons\n\n## Preferences\n"
                };
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(friendly_error)?;
                }
                tokio::fs::write(&path, default)
                    .await
                    .map_err(friendly_error)?;
            }
        }
        _ => {
            crate::learning::SoulStore::clear_project_soul(std::path::Path::new(&dir), &role_name)
                .await?;
        }
    }
    log_console!("[settings] soul cleared for {} ({:?})", role_name, scope);
    Ok(())
}

#[tauri::command]
pub async fn list_soul_roles() -> Result<Vec<String>, String> {
    Ok(crate::learning::SoulStore::list_soul_roles())
}

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

// ── Approval mode ───────────────────────────────────────────

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
