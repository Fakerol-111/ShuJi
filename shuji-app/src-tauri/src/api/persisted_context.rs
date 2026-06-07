use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::client::AnthropicClient;
use crate::api::client::ToolDefinition;
use crate::config::RuntimeConfig;

// ── Public types ──────────────────────────────────────────────

/// Information about a single tool call returned by the LLM.
#[derive(Debug, Clone)]
use tokio::fs;

#[derive(Clone, Serialize, Deserialize)]
pub struct PersistedContext {
    /// The base system prompt (prompt.md content).
    pub base_prompt: String,
    /// Soul / persona message injected right after base prompt (`[soul: role]\n...`).
    #[serde(default)]
    pub soul_prompt: Option<String>,
    /// Recent user/assistant/tool conversation (the last N turns).
    /// Includes skill system messages and `[对话摘要]` summary messages.
    /// Old messages are compressed into `[对话摘要]` system messages.
    pub context_messages: Vec<serde_json::Value>,
}

impl PersistedContext {
    /// Extract 3 layers from a flat messages array.
    /// Skill messages (`[skill:` / `## Working mode:`) go into context_messages,
    /// not a separate layer.
    pub fn from_messages(messages: &[serde_json::Value]) -> Self {
        let mut base_prompt = String::new();
        let mut soul_prompt = None;
        let mut context_messages = Vec::new();

        for msg in messages {
            let role = msg["role"].as_str().unwrap_or("");
            if role == "system" {
                let content = msg["content"].as_str().unwrap_or("");
                if base_prompt.is_empty() {
                    base_prompt = content.to_string();
                } else if content.starts_with("[soul:") {
                    soul_prompt = Some(content.to_string());
                } else {
                    // Includes skill messages and [对话摘要] summaries
                    context_messages.push(msg.clone());
                }
            } else {
                context_messages.push(msg.clone());
            }
        }

        Self {
            base_prompt,
            soul_prompt,
            context_messages,
        }
    }

    /// Rebuild flat messages array from the 3 layers, preserving
    /// the original ordering: base → soul → context.
    /// Skill messages are already embedded in context_messages.
    pub fn to_messages(&self) -> Vec<serde_json::Value> {
        let mut msgs: Vec<serde_json::Value> = Vec::new();
        msgs.push(serde_json::json!({"role": "system", "content": self.base_prompt}));
        if let Some(ref soul) = self.soul_prompt {
            msgs.push(serde_json::json!({"role": "system", "content": soul}));
        }
        for m in &self.context_messages {
            msgs.push(m.clone());
        }
        sanitize_messages(&mut msgs);
        msgs
    }

    /// Save to `.shuji/context/{role}.json` using atomic write (tmp + rename).
    pub async fn save_to(&self, working_dir: &Path, role: &str) {
        let dir = working_dir.join(".shuji/context");
        let _ = tokio::fs::create_dir_all(&dir).await;
        let path = dir.join(format!("{}.json", role));
        let tmp = dir.join(format!("{}.json.tmp", role));
        if let Ok(json) = serde_json::to_string_pretty(&self) {
            let _ = tokio::fs::write(&tmp, &json).await;
            let _ = tokio::fs::rename(&tmp, &path).await;
        }
    }

    /// Truncate verbose `tool` role results to `max_chars` before persisting.
    /// This prevents file contents from inflating the saved context — the current
    /// run is unaffected; only the next `load_from()` sees the trimmed version.
    pub fn trim_tool_results(&mut self, max_chars: usize) {
        for msg in &mut self.context_messages {
            if msg["role"].as_str() == Some("tool") {
                if let Some(content) = msg["content"].as_str() {
                    if content.len() > max_chars {
                        let head: String = content.chars().take(max_chars).collect();
                        msg["content"] = serde_json::Value::String(format!(
                            "{}...(截断, 原 {} 字符)",
                            head,
                            content.len()
                        ));
                    }
                }
            }
        }
    }

    /// Load from `.shuji/context/{role}.json`.
    /// Migrates old format where `skill_prompts` and `history_messages` were
    /// stored as separate fields — converts them to context_messages entries.
    pub async fn load_from(working_dir: &Path, role: &str) -> Option<Self> {
        let path = working_dir
            .join(".shuji/context")
            .join(format!("{}.json", role));
        let data = tokio::fs::read_to_string(&path).await.ok()?;

        // Parse as generic JSON for migration checks
        let mut json: serde_json::Value = serde_json::from_str(&data).ok()?;

        // Migration: extract old fields' data before mutating
        let old_skills: Vec<String> = json
            .get("skill_prompts")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let old_history: Option<String> = json
            .get("history_messages")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if !old_skills.is_empty() || old_history.as_ref().is_some_and(|s| !s.is_empty()) {
            // Remove old fields, then mutate context_messages
            if let Some(obj) = json.as_object_mut() {
                obj.remove("skill_prompts");
                obj.remove("history_messages");
            }
            // Append old skill prompts to context_messages
            if let Some(ctx) = json
                .get_mut("context_messages")
                .and_then(|v| v.as_array_mut())
            {
                for content in &old_skills {
                    ctx.push(serde_json::json!({"role": "system", "content": content}));
                }
                // Prepend old history as a single [对话摘要] entry
                if let Some(ref history) = old_history {
                    if !history.is_empty() {
                        ctx.insert(
                            0,
                            serde_json::json!({
                                "role": "system",
                                "content": format!("[对话摘要] {}", history)
                            }),
                        );
                    }
                }
            }
        }

        serde_json::from_value(json).ok()
    }
}

/// Remove orphaned `tool` role messages and strip dangling `tool_calls` from
/// `assistant` messages. Uses a two-pass approach to avoid ordering-dependent
/// bugs (tool message before assistant, or assistant tool_calls with no
/// matching tool result after context compaction). Prevents 400 errors.
pub(crate) fn sanitize_messages(msgs: &mut Vec<serde_json::Value>) {
    // Pass 1: collect all tool_call_ids announced by assistants and all
    // tool_call_ids that have a corresponding tool result.
    let mut assistant_ids: Vec<String> = Vec::new();
    let mut result_ids: Vec<String> = Vec::new();
    for msg in msgs.iter() {
        match msg["role"].as_str().unwrap_or("") {
            "assistant" => {
                if let Some(tcs) = msg["tool_calls"].as_array() {
                    for tc in tcs {
                        if let Some(id) = tc["id"].as_str() {
                            assistant_ids.push(id.to_string());
                        }
                    }
                }
            }
            "tool" => {
                if let Some(id) = msg["tool_call_id"].as_str() {
                    if !id.is_empty() {
                        result_ids.push(id.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    // Pass 2: filter / clean messages.
    //   assistant — strip dangling tool_calls whose id has no tool result
    //   tool     — keep only if its id was announced by an assistant
    //   other    — keep as-is
    *msgs = msgs
        .iter()
        .filter_map(|msg| {
            let role = msg["role"].as_str().unwrap_or("");
            if role == "assistant" {
                let tcs = match msg["tool_calls"].as_array() {
                    Some(t) if !t.is_empty() => t,
                    _ => return Some(msg.clone()),
                };
                let valid: Vec<serde_json::Value> = tcs
                    .iter()
                    .filter(|tc| {
                        tc["id"]
                            .as_str()
                            .map(|id| result_ids.iter().any(|rid| rid == id))
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                if valid.len() == tcs.len() {
                    Some(msg.clone())
                } else if valid.is_empty() {
                    let mut cleaned = msg.clone();
                    cleaned.as_object_mut().unwrap().remove("tool_calls");
                    Some(cleaned)
                } else {
                    let mut cleaned = msg.clone();
                    cleaned
                        .as_object_mut()
                        .unwrap()
                        .insert("tool_calls".to_string(), serde_json::Value::Array(valid));
                    Some(cleaned)
                }
            } else if role == "tool" {
                let call_id = msg["tool_call_id"].as_str().unwrap_or("");
                let valid = !call_id.is_empty() && assistant_ids.iter().any(|id| id == call_id);
                if valid {
                    Some(msg.clone())
                } else {
                    None
                }
            } else {
                Some(msg.clone())
            }
        })
        .collect();
}

/// Opaque snapshot of Session internals, used for interrupt/restore.
#[derive(Clone)]
pub struct SessionSnapshot {
    pub(crate) messages: Vec<serde_json::Value>,
}

impl SessionSnapshot {
    pub fn from_messages(messages: Vec<serde_json::Value>) -> Self {
        Self { messages }
    }
}

