use std::path::Path;

use serde::{Deserialize, Serialize};

/// Persisted context with 3 separated layers for independent management.
/// Skill messages (`[skill:` or `## Working mode:`) are stored inside
/// `context_messages` as regular system messages, not in a separate layer.
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

    /// Replace the soul layer with freshly loaded content.
    pub fn with_refreshed_soul(mut self, role: &str, soul_content: &str) -> Self {
        if soul_content.trim().is_empty() {
            self.soul_prompt = None;
        } else {
            self.soul_prompt = Some(format!("[soul: {role}]\n{soul_content}"));
        }
        self
    }

    /// Rebuild flat messages array from the 3 layers, preserving
    /// the original ordering: base → soul → context.
    pub fn to_messages(&self) -> Vec<serde_json::Value> {
        self.to_messages_with_soul_override(None)
    }

    /// Rebuild messages, optionally overriding the soul layer with latest content.
    pub fn to_messages_with_soul_override(
        &self,
        soul_override: Option<(&str, &str)>,
    ) -> Vec<serde_json::Value> {
        let mut msgs: Vec<serde_json::Value> = Vec::new();
        msgs.push(serde_json::json!({"role": "system", "content": self.base_prompt}));
        if let Some((role, content)) = soul_override {
            if !content.trim().is_empty() {
                msgs.push(serde_json::json!({"role": "system", "content": format!("[soul: {role}]\n{content}")}));
            }
        } else if let Some(ref soul) = self.soul_prompt {
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
            crate::usage_notify::notify(role, crate::usage_notify::UsageUpdateKind::Context);
        }
    }

    /// Truncate verbose `tool` role results to `max_chars` before persisting.
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
    pub async fn load_from(working_dir: &Path, role: &str) -> Option<Self> {
        let path = working_dir
            .join(".shuji/context")
            .join(format!("{}.json", role));
        let data = tokio::fs::read_to_string(&path).await.ok()?;

        let mut json: serde_json::Value = serde_json::from_str(&data).ok()?;

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
            if let Some(obj) = json.as_object_mut() {
                obj.remove("skill_prompts");
                obj.remove("history_messages");
            }
            if let Some(ctx) = json
                .get_mut("context_messages")
                .and_then(|v| v.as_array_mut())
            {
                for content in &old_skills {
                    ctx.push(serde_json::json!({"role": "system", "content": content}));
                }
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
/// `assistant` messages. Two-pass approach to avoid ordering-dependent bugs.
fn sanitize_messages(msgs: &mut Vec<serde_json::Value>) {
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
                    if let Some(obj) = cleaned.as_object_mut() {
                        obj.remove("tool_calls");
                        Some(cleaned)
                    } else {
                        log_console!("[sanitize] 跳过非 Object 的 assistant 消息");
                        None
                    }
                } else {
                    let mut cleaned = msg.clone();
                    if let Some(obj) = cleaned.as_object_mut() {
                        obj.insert("tool_calls".to_string(), serde_json::Value::Array(valid));
                        Some(cleaned)
                    } else {
                        log_console!("[sanitize] 跳过非 Object 的 assistant 消息");
                        None
                    }
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

// ── Skill message helpers ──────────────────────────────────

/// Check whether a message is a skill system message (`[skill:` or `## Working mode:`).
pub(crate) fn is_skill_message(msg: &serde_json::Value) -> bool {
    msg["role"].as_str() == Some("system")
        && msg["content"]
            .as_str()
            .is_some_and(|c| c.starts_with("[skill:") || c.starts_with("## Working mode:"))
}

/// Remove all skill messages from the vector in-place.
pub(crate) fn strip_skill_messages(msgs: &mut Vec<serde_json::Value>) {
    msgs.retain(|m| !is_skill_message(m));
}

/// Count how many skill messages are in the slice.
pub(crate) fn count_skill_messages(msgs: &[serde_json::Value]) -> usize {
    msgs.iter().filter(|m| is_skill_message(m)).count()
}
