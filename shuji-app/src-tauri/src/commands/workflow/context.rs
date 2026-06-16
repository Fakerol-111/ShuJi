use std::collections::HashMap;
use std::path::Path;

use tauri::State;

use crate::api::client::AnthropicClient;
use crate::api::session::PersistedContext;
use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::commands::settings::ContextWindowConfig;
use crate::commands::workflow::bootstrap::ContextStats;

/// Get per-role context usage statistics.
#[tauri::command]
pub async fn get_context_stats(
    state: State<'_, AppState>,
) -> Result<HashMap<String, ContextStats>, String> {
    let dir = match state.current_dir.lock().await.as_ref() {
        Some(d) => d.clone(),
        None => return Ok(HashMap::new()),
    };
    let config = &state.runtime_config;

    let role_overrides: HashMap<String, crate::config::RoleContextConfig> = {
        let path = std::path::Path::new(&dir).join("context_config.json");
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => match serde_json::from_str::<ContextWindowConfig>(&content) {
                Ok(cfg) => cfg.roles,
                Err(_) => HashMap::new(),
            },
            Err(_) => HashMap::new(),
        }
    };

    let ctx_dir = std::path::Path::new(&dir).join(".shuji/context");
    let mut entries = match tokio::fs::read_dir(&ctx_dir).await {
        Ok(e) => e,
        Err(_) => return Ok(HashMap::new()),
    };

    let mut result = HashMap::new();

    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let role = match path.file_stem().and_then(|s| s.to_str()) {
            Some(r) if !r.is_empty() => r.to_string(),
            _ => continue,
        };

        let data = match tokio::fs::read_to_string(&path).await {
            Ok(d) => d,
            _ => continue,
        };
        let ctx: PersistedContext = match serde_json::from_str(&data) {
            Ok(c) => c,
            _ => continue,
        };

        let token_count = crate::api::token_count::count_messages_tokens(&ctx.context_messages);
        let thresholds = config.resolve_compact_thresholds(&role, role_overrides.get(&role));

        result.insert(
            role,
            ContextStats {
                message_count: ctx.context_messages.len(),
                token_count,
                token_threshold: thresholds.token_threshold,
                compressed: ctx.context_messages.iter().any(|m| {
                    m["role"].as_str() == Some("system")
                        && m["content"]
                            .as_str()
                            .is_some_and(|c| c.starts_with("[对话摘要]"))
                }),
                skill_count: crate::api::session::count_skill_messages(&ctx.context_messages),
            },
        );
    }

    Ok(result)
}

/// Get token usage statistics for all roles.
#[tauri::command]
pub async fn get_token_stats() -> Result<
    std::collections::HashMap<
        String,
        std::collections::HashMap<String, crate::token_tracker::TokenUsage>,
    >,
    String,
> {
    Ok(crate::token_tracker::snapshot_grouped())
}

/// Manually trigger context compaction for a specific role.
#[tauri::command]
pub async fn compact_context(state: State<'_, AppState>, role: String) -> Result<String, String> {
    let dir = state
        .current_dir
        .lock()
        .await
        .clone()
        .ok_or_else(|| friendly_error("no open project"))?;
    let working_dir = std::path::Path::new(&dir);

    if crate::round_metrics::is_active(&role) {
        return Err(friendly_error(format!(
            "role {} is currently executing, please wait for completion before compacting",
            role
        )));
    }

    {
        let mut compacting = state.compacting_roles.lock().await;
        if !compacting.insert(role.clone()) {
            return Err(friendly_error(format!(
                "role {} is already being compacted, please do not repeat the operation",
                role
            )));
        }
    }

    let result = compact_impl(working_dir, &role, &state).await;
    state.compacting_roles.lock().await.remove(&role);
    result
}

async fn compact_impl(
    working_dir: &Path,
    role: &str,
    state: &State<'_, AppState>,
) -> Result<String, String> {
    let role_overrides: HashMap<String, crate::config::RoleContextConfig> = {
        let path = working_dir.join("context_config.json");
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str::<ContextWindowConfig>(&content)
                .ok()
                .map(|c| c.roles)
                .unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    };

    let mut ctx = PersistedContext::load_from(working_dir, role)
        .await
        .ok_or_else(|| friendly_error(format!("no context file found for role {}", role)))?;

    let thresholds = state
        .runtime_config
        .resolve_compact_thresholds(role, role_overrides.get(role));

    let total_tokens = crate::api::token_count::count_messages_tokens(&ctx.context_messages);

    let force_thresholds = crate::config::CompactThresholds {
        token_threshold: 0,
        keep_recent_count: thresholds.keep_recent_count,
        mid_run_compact: false,
    };

    let config = crate::commands::settings::get_config()
        .await
        .map_err(friendly_error)?;
    let ep = config.for_role(role);

    if ep.api_key.is_empty() {
        return Err(friendly_error(format!(
            "role {} has no API key configured, please configure in settings",
            role
        )));
    }
    if ep.api_url.is_empty() {
        return Err(friendly_error(format!(
            "role {} has no API URL configured",
            role
        )));
    }

    let client = AnthropicClient::new(ep.api_key, ep.api_url);
    let model = ep.model;
    let is_cabinet = role == "neige";

    log_console!(
        "[compact:manual] starting compaction for {} (cabinet={}, tokens={})",
        role,
        is_cabinet,
        total_tokens,
    );

    let performed = crate::api::compact::run_compaction_loop(
        &client,
        &model,
        &mut ctx,
        &force_thresholds,
        is_cabinet,
        working_dir,
        role,
    )
    .await;

    if performed {
        log_console!("[compact:manual] compaction completed for {}", role);
        Ok(format!(
            "Compaction complete (role: {}, original {} tokens -> summary + {} recent messages)",
            role, total_tokens, thresholds.keep_recent_count,
        ))
    } else {
        Err(friendly_error(format!(
            "Compaction failed for role {} -- API call did not return a valid summary. Please check API config and retry",
            role
        )))
    }
}
