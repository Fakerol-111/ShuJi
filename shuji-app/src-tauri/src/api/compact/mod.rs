use std::path::Path;

use crate::api::client::AnthropicClient;
use crate::api::session::PersistedContext;
use crate::api::token_count;
use crate::config::CompactThresholds;
use crate::models::message::Message;

/// Build a flat text representation of messages for the summarizer.
pub fn messages_to_text(msgs: &[serde_json::Value]) -> String {
    let mut lines = Vec::new();
    for m in msgs {
        let role = m["role"].as_str().unwrap_or("");
        if let Some(content) = m["content"].as_str() {
            if !content.is_empty() {
                lines.push(format!("[{}]: {}", role, content));
            }
        }
        if let Some(tool_calls) = m["tool_calls"].as_array() {
            for tc in tool_calls {
                let name = tc["function"]["name"].as_str().unwrap_or("?");
                lines.push(format!("[{} → {}]", role, name));
            }
        }
        if role == "tool" {
            lines.push("[tool_result]".to_string());
        }
    }
    lines.join("\n")
}

/// Internal compaction function with configurable prompt.
/// Returns the new context_messages with old messages replaced by a summary,
/// or None if no compression was needed.
async fn maybe_compact_with_prompt(
    client: &AnthropicClient,
    model: &str,
    context_messages: &[serde_json::Value],
    thresholds: &CompactThresholds,
    compact_prompt: &str,
    tag: &str,
) -> Option<Vec<serde_json::Value>> {
    let total_tokens = token_count::count_messages_tokens(context_messages);

    if total_tokens < thresholds.token_threshold {
        return None;
    }

    // Strip skill messages so they never enter the compressible batch.
    // They will be re-appended to the keep zone after compression.
    let skill_msgs: Vec<serde_json::Value> = context_messages
        .iter()
        .filter(|m| crate::api::session::is_skill_message(m))
        .cloned()
        .collect();

    // Work with non-skill messages only
    let mut non_skill = context_messages.to_vec();
    crate::api::session::strip_skill_messages(&mut non_skill);

    let split_at = non_skill
        .len()
        .saturating_sub(thresholds.keep_recent_count);
    if split_at == 0 {
        return None;
    }
    let old_messages = non_skill[..split_at].to_vec();
    let kept_context = non_skill[split_at..].to_vec();

    let input_text = messages_to_text(&old_messages);

    let msgs = vec![Message::user(&input_text)];

    log_console!(
        "[compact:{}] compressing {} messages ({} tokens) → target ≤500 token summary",
        tag,
        old_messages.len(),
        token_count::count_messages_tokens(&old_messages)
    );

    match client.send_message(compact_prompt, &msgs, model).await {
        Ok(summary) => {
            let trimmed = summary.trim();
            if trimmed.is_empty() || trimmed.len() < 20 {
                log_console!(
                    "[compact:{}] returned empty or too-short summary, skipping",
                    tag
                );
                return None;
            }
            let tagged = if trimmed.starts_with("[对话摘要]") {
                trimmed.to_string()
            } else {
                format!("[对话摘要] {}", trimmed)
            };
            let kept_count = kept_context.len();
            let skill_count = skill_msgs.len();
            let summary_msg = serde_json::json!({"role": "system", "content": tagged});
            let mut new_context = vec![summary_msg];
            new_context.extend(kept_context);
            // Re-append skill messages at the end so they stay in the keep zone
            new_context.extend(skill_msgs);
            log_console!(
                "[compact:{}] done — summary msg + {} recent msgs + {} skill msgs",
                tag,
                kept_count,
                skill_count
            );
            Some(new_context)
        }
        Err(e) => {
            log_console!("[compact:{}] summarization failed: {}", tag, e);
            None
        }
    }
}

/// Check whether compaction is needed and, if so, run it (内阁 version).
pub async fn maybe_compact(
    client: &AnthropicClient,
    model: &str,
    context_messages: &[serde_json::Value],
    thresholds: &CompactThresholds,
) -> Option<Vec<serde_json::Value>> {
    maybe_compact_with_prompt(
        client,
        model,
        context_messages,
        thresholds,
        include_str!("prompt.md"),
        "cabinet",
    )
    .await
}

/// Check whether compaction is needed and, if so, run it (department version).
pub async fn maybe_compact_dept(
    client: &AnthropicClient,
    model: &str,
    context_messages: &[serde_json::Value],
    thresholds: &CompactThresholds,
) -> Option<Vec<serde_json::Value>> {
    maybe_compact_with_prompt(
        client,
        model,
        context_messages,
        thresholds,
        include_str!("dept_prompt.md"),
        "dept",
    )
    .await
}

/// Run compaction on a loaded PersistedContext.
/// Compresses old context_messages into a summary message prepended to context.
/// Returns `true` if any compaction was performed.
pub async fn run_compaction_loop(
    client: &AnthropicClient,
    model: &str,
    ctx: &mut PersistedContext,
    thresholds: &CompactThresholds,
    is_cabinet: bool,
    working_dir: &Path,
    role: &str,
) -> bool {
    let compact = if is_cabinet {
        maybe_compact(client, model, &ctx.context_messages, thresholds).await
    } else {
        maybe_compact_dept(client, model, &ctx.context_messages, thresholds).await
    };

    if let Some(new_ctx) = compact {
        ctx.context_messages = new_ctx;
        ctx.save_to(working_dir, role).await;
        true
    } else {
        false
    }
}

/// High-level helper: compact context, trim tool results, and save.
/// Intended for use in agent mid-run compact handlers and reload paths.
pub async fn compact_and_save(
    client: &AnthropicClient,
    model: &str,
    ctx: &mut PersistedContext,
    thresholds: &CompactThresholds,
    is_cabinet: bool,
    working_dir: &Path,
    role: &str,
) -> bool {
    let compact = if is_cabinet {
        maybe_compact(client, model, &ctx.context_messages, thresholds).await
    } else {
        maybe_compact_dept(client, model, &ctx.context_messages, thresholds).await
    };

    if let Some(new_ctx) = compact {
        ctx.context_messages = new_ctx;
        ctx.trim_tool_results(2000);
        ctx.save_to(working_dir, role).await;
        true
    } else {
        false
    }
}
