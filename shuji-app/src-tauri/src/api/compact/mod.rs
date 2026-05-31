use std::path::Path;

use crate::api::client::AnthropicClient;
use crate::api::session::PersistedContext;
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

/// Result of a compaction run.
pub struct CompactResult {
    pub new_history: String,
    pub kept_context: Vec<serde_json::Value>,
}

/// Internal compaction function with configurable prompt.
async fn maybe_compact_with_prompt(
    client: &AnthropicClient,
    model: &str,
    history_messages: &str,
    context_messages: &[serde_json::Value],
    thresholds: &CompactThresholds,
    compact_prompt: &str,
    tag: &str,
) -> Option<CompactResult> {
    let total_chars: usize = context_messages.iter().map(|m| m.to_string().len()).sum();

    if total_chars < thresholds.char_threshold {
        return None;
    }

    // Split: old messages to compress, recent messages to keep
    let split_at = context_messages
        .len()
        .saturating_sub(thresholds.keep_recent_count);
    if split_at == 0 {
        return None;
    }
    let old_messages = context_messages[..split_at].to_vec();
    let kept_context = context_messages[split_at..].to_vec();

    let existing_summary = if history_messages.is_empty() {
        String::new()
    } else {
        format!("\n\nExisting summary:\n{}", history_messages)
    };

    let input_text = format!(
        "{}\n\nConversation to summarize:\n\n{}",
        existing_summary,
        messages_to_text(&old_messages)
    );

    let msgs = vec![Message::user(&input_text)];

    log_console!(
        "[compact:{}] compressing {} messages ({} chars) → target ≤500 chars summary",
        tag,
        old_messages.len(),
        old_messages
            .iter()
            .map(|m| m.to_string().len())
            .sum::<usize>()
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
            let new_history = if history_messages.is_empty() {
                tagged
            } else {
                format!("{}\n{}", history_messages, tagged)
            };
            log_console!(
                "[compact:{}] done — summary: {} chars, keeping {} recent msgs",
                tag,
                new_history.len(),
                kept_context.len()
            );
            Some(CompactResult {
                new_history,
                kept_context,
            })
        }
        Err(e) => {
            log_console!("[compact:{}] summarization failed: {}", tag, e);
            None
        }
    }
}

/// Check whether compaction is needed and, if so, run it (内阁 version).
/// Uses the 内阁-specific compaction prompt.
/// Returns the new history summary + trimmed recent messages, or None.
pub async fn maybe_compact(
    client: &AnthropicClient,
    model: &str,
    history_messages: &str,
    context_messages: &[serde_json::Value],
    thresholds: &CompactThresholds,
) -> Option<CompactResult> {
    maybe_compact_with_prompt(
        client,
        model,
        history_messages,
        context_messages,
        thresholds,
        include_str!("prompt.md"),
        "cabinet",
    )
    .await
}

/// Check whether compaction is needed and, if so, run it (department version).
/// Uses a department-generic compaction prompt.
/// Returns the new history summary + trimmed recent messages, or None.
pub async fn maybe_compact_dept(
    client: &AnthropicClient,
    model: &str,
    history_messages: &str,
    context_messages: &[serde_json::Value],
    thresholds: &CompactThresholds,
) -> Option<CompactResult> {
    maybe_compact_with_prompt(
        client,
        model,
        history_messages,
        context_messages,
        thresholds,
        include_str!("dept_prompt.md"),
        "dept",
    )
    .await
}

/// Merge multiple accumulated summaries into one.
/// Returns the merged summary, or None if not needed.
pub async fn maybe_compact_history(
    client: &AnthropicClient,
    model: &str,
    history_messages: &str,
    thresholds: &CompactThresholds,
) -> Option<String> {
    if history_messages.len() < thresholds.history_char_threshold {
        return None;
    }

    let compact_prompt = include_str!("history_prompt.md");
    let msgs = vec![Message::user(history_messages)];

    log_console!(
        "[compact:history] merging summaries ({} chars → target ≤500 chars)",
        history_messages.len()
    );

    match client.send_message(compact_prompt, &msgs, model).await {
        Ok(summary) => {
            let trimmed = summary.trim();
            if trimmed.is_empty() || trimmed.len() < 20 {
                log_console!("[compact:history] returned empty or too-short summary, skipping");
                return None;
            }
            let tagged = if trimmed.starts_with("[对话摘要]") {
                trimmed.to_string()
            } else {
                format!("[对话摘要] {}", trimmed)
            };
            log_console!("[compact:history] done — merged to {} chars", tagged.len());
            Some(tagged)
        }
        Err(e) => {
            log_console!("[compact:history] summarization failed: {}", e);
            None
        }
    }
}

/// Run the iterative compaction loop on a loaded PersistedContext.
///
/// 1. Compresses context_messages into history (cabinet or dept prompt).
/// 2. Merges accumulated history summaries.
/// 3. Repeats until no further changes.
/// 4. Saves to disk after each mutation step.
///
/// Returns `true` if any compaction was actually performed.
pub async fn run_compaction_loop(
    client: &AnthropicClient,
    model: &str,
    ctx: &mut PersistedContext,
    thresholds: &CompactThresholds,
    is_cabinet: bool,
    working_dir: &Path,
    role: &str,
) -> bool {
    let mut changed = false;
    let mut safety = 0u32;
    loop {
        safety += 1;
        if safety > 20 {
            log_console!("[compact] safety limit reached, breaking compaction loop");
            break;
        }
        let mut round_changed = false;

        let result = if is_cabinet {
            maybe_compact(client, model, &ctx.history_messages, &ctx.context_messages, thresholds).await
        } else {
            maybe_compact_dept(client, model, &ctx.history_messages, &ctx.context_messages, thresholds).await
        };
        if let Some(result) = result {
            ctx.history_messages = result.new_history;
            ctx.context_messages = result.kept_context;
            ctx.save_to(working_dir, role).await;
            round_changed = true;
        }

        if let Some(merged) = maybe_compact_history(client, model, &ctx.history_messages, thresholds).await {
            ctx.history_messages = merged;
            ctx.save_to(working_dir, role).await;
            round_changed = true;
        }

        if !round_changed {
            break;
        }
        changed = true;
    }
    changed
}
