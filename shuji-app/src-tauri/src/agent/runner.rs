//! Shared agent execution helpers.
//!
//! Extracts the compact handler, checkpoint handler, and session setup logic
//! that is duplicated across all non-内阁 agent implementations.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::api::client::AnthropicClient;
use crate::api::compact::compact_and_save;
use crate::api::control::{CheckpointFn, CompactFn};
use crate::api::session::{PersistedContext, Session, SessionSnapshot};
use crate::config::{CompactThresholds, RuntimeConfig};

/// Build a mid-run compaction handler (shared by all agents).
///
/// Reads context_config.json for live threshold updates, compacts the
/// session messages, and persists the compressed context to disk.
/// The running session is NOT modified — the compressed context is
/// loaded automatically on the next execute() call.
pub fn build_compact_handler(
    client: AnthropicClient,
    model: String,
    working_dir: PathBuf,
    role_name: String,
    runtime_config: Arc<RuntimeConfig>,
    is_cabinet: bool,
    context_window_config: Arc<std::collections::HashMap<String, crate::config::RoleContextConfig>>,
) -> (CompactFn, u32) {
    let cb: CompactFn = Box::new(move |messages: Vec<serde_json::Value>| {
        let client = client.clone();
        let model = model.clone();
        let wd = working_dir.clone();
        let role = role_name.clone();
        let cfg = runtime_config.clone();
        let ctx_roles = context_window_config.clone();
        Box::pin(async move {
            let thresholds = cfg.resolve_compact_thresholds(&role, ctx_roles.get(&role));

            let mut ctx = PersistedContext::from_messages(&messages);
            compact_and_save(
                &client,
                &model,
                &mut ctx,
                &thresholds,
                is_cabinet,
                &wd,
                &role,
            )
            .await;
        })
    });
    (cb, 40)
}

/// Build a periodic checkpoint handler (shared by all agents).
///
/// Saves a git commit + session snapshot under `.shuji/checkpoints/`.
pub fn build_checkpoint_handler(
    working_dir: PathBuf,
    role_name: String,
    task_description: String,
) -> CheckpointFn {
    Box::new(move |snap: SessionSnapshot| {
        let wd = working_dir.clone();
        let role = role_name.clone();
        let desc = task_description.clone();
        Box::pin(async move {
            crate::storage::checkpoint::save(&wd, &role, &desc, &snap).await;
        })
    })
}

/// Load persisted context for the role, compact if needed, and restore into the session.
///
/// Returns true if context was loaded + restored (i.e. this is a continuation),
/// false if no persisted context exists (fresh start).
#[allow(clippy::too_many_arguments)]
pub async fn load_and_compact_context(
    client: &AnthropicClient,
    model: &str,
    working_dir: &Path,
    role_name: &str,
    task_description: &str,
    session: &mut Session,
    thresholds: &CompactThresholds,
    is_cabinet: bool,
) -> bool {
    if let Some(mut ctx) = PersistedContext::load_from(working_dir, role_name).await {
        compact_and_save(
            client,
            model,
            &mut ctx,
            thresholds,
            is_cabinet,
            working_dir,
            role_name,
        )
        .await;

        let mut msgs = ctx.to_messages();
        // 防止双份注入：检查末尾是否已有相同 task
        let last_is_task = msgs
            .last()
            .filter(|m| m["role"].as_str() == Some("user"))
            .and_then(|m| m["content"].as_str())
            .map(|c| c.contains(task_description))
            .unwrap_or(false);
        if !last_is_task {
            msgs.push(serde_json::json!({"role": "user", "content": task_description}));
        }
        let snap = SessionSnapshot::from_messages(msgs);
        session.restore(&snap);
        true
    } else {
        false
    }
}

/// Save the current session state as persisted context.
pub async fn save_context(session: &Session, working_dir: &Path, role_name: &str) {
    let snap = session.snapshot();
    let ctx = PersistedContext::from_messages(&snap.messages);
    ctx.save_to(working_dir, role_name).await;
}
