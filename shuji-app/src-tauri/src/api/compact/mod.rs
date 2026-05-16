use crate::api::client::AnthropicClient;
use crate::models::message::Message;

/// Threshold: trigger compaction when context_messages exceed ~80k tokens (chars/2).
const COMPACT_CHAR_THRESHOLD: usize = 160_000;
/// After compaction, keep the most recent N messages.
const KEEP_RECENT_COUNT: usize = 6;

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

/// Check whether compaction is needed and, if so, run it.
/// Returns the new history summary + trimmed recent messages, or None.
pub async fn maybe_compact(
    client: &AnthropicClient,
    model: &str,
    history_messages: &str,
    context_messages: &[serde_json::Value],
) -> Option<CompactResult> {
    let total_chars: usize = context_messages.iter()
        .map(|m| m.to_string().len())
        .sum();

    if total_chars < COMPACT_CHAR_THRESHOLD {
        return None;
    }

    // Split: old messages to compress, recent messages to keep
    let split_at = context_messages.len().saturating_sub(KEEP_RECENT_COUNT);
    if split_at == 0 {
        return None;
    }
    let old_messages = context_messages[..split_at].to_vec();
    let kept_context = context_messages[split_at..].to_vec();

    let compact_prompt = include_str!("prompt.md");
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

    log_console!("[compact] compressing {} messages ({} chars) → target ≤500 chars summary",
        old_messages.len(), old_messages.iter().map(|m| m.to_string().len()).sum::<usize>());

    match client.send_message(compact_prompt, &msgs, model).await {
        Ok(summary) => {
            let trimmed = summary.trim();
            if trimmed.is_empty() || trimmed.len() < 20 {
                log_console!("[compact] returned empty or too-short summary, skipping");
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
            log_console!("[compact] done — summary: {} chars, keeping {} recent msgs",
                new_history.len(), kept_context.len());
            Some(CompactResult { new_history, kept_context })
        }
        Err(e) => {
            log_console!("[compact] summarization failed: {}", e);
            None
        }
    }
}

/// Threshold: trigger history compaction when accumulated summaries exceed 2000 chars.
const HISTORY_COMPACT_CHAR_THRESHOLD: usize = 2000;

/// Merge multiple accumulated summaries into one.
/// Returns the merged summary, or None if not needed.
pub async fn maybe_compact_history(
    client: &AnthropicClient,
    model: &str,
    history_messages: &str,
) -> Option<String> {
    if history_messages.len() < HISTORY_COMPACT_CHAR_THRESHOLD {
        return None;
    }

    let compact_prompt = include_str!("history_prompt.md");
    let msgs = vec![Message::user(history_messages)];

    log_console!("[compact:history] merging summaries ({} chars → target ≤500 chars)",
        history_messages.len());

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
