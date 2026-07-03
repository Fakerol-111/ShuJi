use std::path::Path;

use crate::api::client::LlmClient;
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
    client: &LlmClient,
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

    // Partition into skill and non-skill in a single pass,
    // avoiding the full `to_vec()` + `strip_skill_messages` double-copy.
    let mut skill_msgs: Vec<serde_json::Value> = Vec::new();
    let mut non_skill: Vec<serde_json::Value> = Vec::with_capacity(context_messages.len());
    for msg in context_messages {
        if crate::api::session::is_skill_message(msg) {
            skill_msgs.push(msg.clone());
        } else {
            non_skill.push(msg.clone());
        }
    }

    let split_at = non_skill.len().saturating_sub(thresholds.keep_recent_count);
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

    match client
        .send_message_with_reasoning(
            compact_prompt,
            &msgs,
            model,
            crate::config::ResolvedReasoningPolicy::disabled(),
        )
        .await
    {
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
    client: &LlmClient,
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
    client: &LlmClient,
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
    client: &LlmClient,
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
    client: &LlmClient,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_messages_to_text_user_and_assistant() {
        let msgs = vec![
            serde_json::json!({"role": "user", "content": "hello"}),
            serde_json::json!({"role": "assistant", "content": "world"}),
        ];
        let text = messages_to_text(&msgs);
        assert!(text.contains("[user]: hello"));
        assert!(text.contains("[assistant]: world"));
    }

    #[test]
    fn test_messages_to_text_with_tool_calls() {
        let msgs = vec![serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{"id": "c1", "type": "function", "function": {"name": "read_file", "arguments": "{}"}}]
        })];
        let text = messages_to_text(&msgs);
        assert!(text.contains("[assistant → read_file]"));
    }

    #[test]
    fn test_messages_to_text_tool_result() {
        let msgs = vec![
            serde_json::json!({"role": "tool", "tool_call_id": "c1", "content": "file content"}),
        ];
        let text = messages_to_text(&msgs);
        assert!(text.contains("[tool_result]"));
    }

    #[test]
    fn test_messages_to_text_empty_content_omitted() {
        let msgs = vec![
            serde_json::json!({"role": "user", "content": ""}),
            serde_json::json!({"role": "assistant", "content": "real content"}),
        ];
        let text = messages_to_text(&msgs);
        assert!(!text.contains("[user]:"));
        assert!(text.contains("[assistant]: real content"));
    }

    #[test]
    fn test_messages_to_text_mixed() {
        let msgs = vec![
            serde_json::json!({"role": "user", "content": "first"}),
            serde_json::json!({"role": "assistant", "content": "thinking", "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "search", "arguments": "{}"}}
            ]}),
            serde_json::json!({"role": "tool", "tool_call_id": "c1", "content": "results"}),
        ];
        let text = messages_to_text(&msgs);
        assert!(text.contains("[user]: first"));
        assert!(text.contains("[assistant → search]"));
        assert!(text.contains("[tool_result]"));
    }

    #[test]
    fn test_keep_recent_count_splitting_non_skill_only() {
        // Simulate the split logic inside maybe_compact_with_prompt:
        // 5 non-skill messages, keep_recent_count = 2 → split at index 3
        let msgs: Vec<serde_json::Value> = (0..5)
            .map(|i| serde_json::json!({"role": "user", "content": format!("msg {}", i)}))
            .collect();
        let keep = 2;
        let split = msgs.len().saturating_sub(keep);
        assert_eq!(split, 3);

        let old = &msgs[..split];
        let kept = &msgs[split..];
        assert_eq!(old.len(), 3);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0]["content"], "msg 3");
        assert_eq!(kept[1]["content"], "msg 4");
    }

    #[test]
    fn test_keep_recent_count_splitting_with_skill_msgs() {
        // Simulate: 2 skill msgs + 5 non-skill msgs, keep_recent_count = 3
        // After stripping skills: 5 non-skill → split at 5-3=2, so 2 old + 3 kept + 2 re-appended skills
        let skill_msgs: Vec<serde_json::Value> = (0..2)
            .map(|i| serde_json::json!({"role": "system", "content": format!("[skill: test] skill {}", i)}))
            .collect();
        let non_skill: Vec<serde_json::Value> = (0..5)
            .map(|i| serde_json::json!({"role": "user", "content": format!("msg {}", i)}))
            .collect();

        let keep = 3;
        let split = non_skill.len().saturating_sub(keep);
        assert_eq!(split, 2);

        let old = &non_skill[..split];
        let kept = &non_skill[split..];
        assert_eq!(old.len(), 2);
        assert_eq!(kept.len(), 3);

        // Re-construct: summary + kept + skills
        let mut new_ctx: Vec<serde_json::Value> = vec![];
        new_ctx.push(serde_json::json!({"role": "system", "content": "[对话摘要] summary"}));
        new_ctx.extend_from_slice(kept);
        new_ctx.extend(skill_msgs);

        assert_eq!(new_ctx.len(), 1 + 3 + 2);
        assert!(new_ctx[0]["content"]
            .as_str()
            .unwrap()
            .contains("[对话摘要]"));
        // Skill messages should be at the end
        assert!(new_ctx[5]["content"].as_str().unwrap().contains("[skill:"));
        assert_eq!(new_ctx[3]["content"], "msg 4"); // last kept msg at index 3
    }

    #[test]
    fn test_keep_recent_count_zero_split_no_compression() {
        // If keep_recent_count >= total messages, split_at = 0 → no compression
        let msgs: Vec<serde_json::Value> = (0..3)
            .map(|i| serde_json::json!({"role": "user", "content": format!("msg {}", i)}))
            .collect();
        let keep = 5; // more than total
        let split = msgs.len().saturating_sub(keep);
        assert_eq!(split, 0, "should be 0 when keep > total");
    }

    #[test]
    fn test_messages_to_text_non_empty_msg_after_skill_strip() {
        let msgs = vec![
            serde_json::json!({"role": "system", "content": "[skill: demo] active"}),
            serde_json::json!({"role": "user", "content": "run demo"}),
        ];
        let text = messages_to_text(&msgs);
        assert!(text.contains("[system]: [skill: demo]"));
        assert!(text.contains("[user]: run demo"));
    }
}
