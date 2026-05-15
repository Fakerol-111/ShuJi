#![allow(dead_code)]
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::api::client::AnthropicClient;
use crate::api::client::ToolDefinition;

const API_TIMEOUT: Duration = Duration::from_secs(180);

// ── Public types ──────────────────────────────────────────────

/// Information about a single tool call returned by the LLM.
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

/// Outcome of one Session::step().
pub enum StepResult {
    Text(String),
    ToolCalls(Vec<ToolCallInfo>),
}

/// Persisted context with 4 separated layers for independent management.
#[derive(Clone, Serialize, Deserialize)]
pub struct PersistedContext {
    /// The base system prompt (prompt.md content).
    pub base_prompt: String,
    /// Active skill prompts, each prefixed with `[skill: name]\n`.
    pub skill_prompts: Vec<String>,
    /// Compressed summary of early conversation history.
    pub history_messages: String,
    /// Recent user/assistant/tool conversation (the last N turns).
    pub context_messages: Vec<serde_json::Value>,
}

impl PersistedContext {
    /// Extract 4 layers from a flat messages array.
    pub fn from_messages(messages: &[serde_json::Value]) -> Self {
        let mut base_prompt = String::new();
        let mut skill_prompts = Vec::new();
        let mut history_messages = String::new();
        let mut context_messages = Vec::new();

        for msg in messages {
            let role = msg["role"].as_str().unwrap_or("");
            if role == "system" {
                let content = msg["content"].as_str().unwrap_or("");
                if base_prompt.is_empty() {
                    base_prompt = content.to_string();
                } else if content.starts_with("[skill:") {
                    skill_prompts.push(content.to_string());
                } else if content.starts_with("[对话摘要]") {
                    history_messages = content.to_string();
                } else {
                    context_messages.push(msg.clone());
                }
            } else {
                context_messages.push(msg.clone());
            }
        }

        Self { base_prompt, skill_prompts, history_messages, context_messages }
    }

    /// Rebuild flat messages array from the 4 layers.
    pub fn to_messages(&self) -> Vec<serde_json::Value> {
        let mut msgs: Vec<serde_json::Value> = Vec::new();
        msgs.push(serde_json::json!({"role": "system", "content": self.base_prompt}));
        for sp in &self.skill_prompts {
            msgs.push(serde_json::json!({"role": "system", "content": sp}));
        }
        if !self.history_messages.is_empty() {
            msgs.push(serde_json::json!({"role": "system", "content": self.history_messages}));
        }
        for m in &self.context_messages {
            msgs.push(m.clone());
        }
        msgs
    }

    /// Save to `.shuji/context/{role}.json`.
    pub fn save_to(&self, working_dir: &Path, role: &str) {
        let dir = working_dir.join(".shuji/context");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{}.json", role));
        if let Ok(json) = serde_json::to_string_pretty(&self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Load from `.shuji/context/{role}.json`.
    pub fn load_from(working_dir: &Path, role: &str) -> Option<Self> {
        let path = working_dir.join(".shuji/context").join(format!("{}.json", role));
        let data = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }
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

// ── Session ──────────────────────────────────────────────────

/// Pure LLM layer: owns the message history and provides one-step
/// API interaction.  Never runs tool loops — that's AgentController's job.
pub struct Session {
    messages: Vec<serde_json::Value>,
    model: String,
    tools: Vec<ToolDefinition>,
    client: Arc<AnthropicClient>,
    max_tokens: u32,
    role: String,
    /// Whether this session has write_file tool (affects retry token floor).
    has_write_file: bool,
    /// Force tool_choice = "none" when set (e.g. during skill selection).
    /// Prevents the LLM from calling tools until a mode is selected.
    tool_choice_none: bool,
    /// If set, truncated output is written here for debugging.
    debug_dir: Option<PathBuf>,
}

impl Session {
    /// Build a new Session from a system prompt, message history,
    /// model name, tool definitions, and an API client reference.
    /// `skill_prompts` are system messages inserted between the base prompt
    /// and the history (used by 内阁 for cross-turn skill persistence).
    pub fn new(
        system: &str,
        history: &[crate::models::message::Message],
        model: &str,
        tools: &[ToolDefinition],
        client: &Arc<AnthropicClient>,
        skill_prompts: &[String],
    ) -> Self {
        let mut messages: Vec<serde_json::Value> = Vec::new();
        messages.push(serde_json::json!({"role": "system", "content": system}));
        for sp in skill_prompts {
            messages.push(serde_json::json!({"role": "system", "content": sp}));
        }
        for m in history {
            messages.push(serde_json::json!({"role": m.role, "content": m.content}));
        }
        if messages.len() <= 1 {
            messages.push(serde_json::json!({"role": "user", "content": "请继续"}));
        }

        let has_write_file = tools.iter().any(|t| t.function.name == "create_file");
        let has_append_document = tools.iter().any(|t| t.function.name == "append_document");
        let max_tokens = if has_write_file {
            2048
        } else if has_append_document {
            // 中书令等使用 append_document 的 agent 需要更多 tokens
            // 因为需要多次调用工具（每次最多 500 chars）
            1536
        } else if tools.iter().any(|t| t.function.name == "read_file") {
            1024
        } else {
            512
        };

        // Inject the cross-department route_to tool for every agent.
        let mut all_tools = tools.to_vec();
        all_tools.push(crate::tool::registry::route_tool());

        Self {
            messages,
            model: model.to_string(),
            tools: all_tools,
            client: client.clone(),
            max_tokens,
            role: "session".to_string(),
            has_write_file: has_write_file || has_append_document,
            tool_choice_none: false,
            debug_dir: None,
        }
    }

    /// Set a role label for logging.
    pub fn with_role(mut self, role: &str) -> Self {
        self.role = role.to_string();
        self
    }

    /// Override the auto-detected max_tokens value.
    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    /// Enable truncation debug output to `.shuji/debug/truncated.md`.
    pub fn with_debug_dir(mut self, dir: PathBuf) -> Self {
        self.debug_dir = Some(dir);
        self
    }

    /// Force tool_choice to "none", preventing the LLM from calling tools.
    /// Used during skill selection to force text-only responses.
    pub fn set_tool_choice_none(&mut self, force: bool) {
        self.tool_choice_none = force;
    }

    /// One complete API round-trip.  Builds the request, sends it,
    /// logs token usage, and returns the LLM's decision.
    ///
    /// - If the LLM returns **text** → `StepResult::Text(content)`
    /// - If the LLM returns **tool calls** → appends the assistant
    ///   message to the internal history, returns `StepResult::ToolCalls(list)`
    pub async fn step(&mut self) -> anyhow::Result<StepResult> {
        let mut length_retries = 0u32;
        const MAX_LENGTH_RETRIES: u32 = 5;
        let mut api_retries = 0u32;
        const MAX_API_RETRIES: u32 = 3;

        loop {
            let mut body = serde_json::json!({
                "model": self.model,
                "max_tokens": self.max_tokens,
                "messages": self.messages,
                "temperature": 0.1,
                "top_p": 0.9,
                "frequency_penalty": 0.1,
                "seed": 42,
            });
            if self.tool_choice_none {
                body["tool_choice"] = serde_json::json!("none");
                if !self.tools.is_empty() {
                    body["tools"] = serde_json::to_value(&self.tools).unwrap_or_default();
                    body["parallel_tool_calls"] = serde_json::json!(false);
                    body["temperature"] = serde_json::json!(0.0);
                }
            } else if !self.tools.is_empty() {
                body["tools"] = serde_json::to_value(&self.tools).unwrap_or_default();
                body["tool_choice"] = serde_json::json!("auto");
            }

            log_console!("[{}] step: sending {} messages", self.role, self.messages.len());

            let data = match Self::api_request(&self.client, &body).await {
                Ok(d) => d,
                Err(e) => {
                    api_retries += 1;
                    if api_retries < MAX_API_RETRIES {
                        log_console!("[{}] API 请求失败 (retry {}/{}), 2s 后重试: {}", self.role, api_retries, MAX_API_RETRIES, e);
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    }
                    return Err(e);
                }
            };
            let msg = &data["choices"][0]["message"];
            let finish_reason = data["choices"][0]["finish_reason"]
                .as_str()
                .unwrap_or("");

            // Log token usage
            if let Some(usage) = data.get("usage") {
                let prompt = usage["prompt_tokens"].as_u64().unwrap_or(0);
                let completion = usage["completion_tokens"].as_u64().unwrap_or(0);
                log_console!(
                    "[{}] tokens: prompt={} completion={} total={}",
                    self.role,
                    prompt,
                    completion,
                    prompt + completion
                );
                if !self.role.is_empty() {
                    crate::token_tracker::record(&self.role, prompt, completion);
                }
            }

            // Log completion content for debugging
            {
                let text = msg["content"].as_str().unwrap_or("");
                let has_tools = msg["tool_calls"].as_array().map_or(false, |a| !a.is_empty());
                if has_tools {
                    let tool_names: Vec<&str> = msg["tool_calls"].as_array().unwrap()
                        .iter().filter_map(|tc| tc["function"]["name"].as_str()).collect();
                    if text.is_empty() {
                        log_console!("[{}] → tools: {:?}", self.role, tool_names);
                    } else {
                        let preview = Self::preview(text);
                        log_console!("[{}] → {} | tools: {:?}", self.role, preview, tool_names);
                    }
                } else if !text.is_empty() {
                    log_console!("[{}] → {}", self.role, Self::preview(text));
                }
            }

            // Handle truncated responses: auto-continue when text was cut off
            if finish_reason == "length" {
                self.write_debug_truncated(msg, length_retries);

                // First pass: parse tool calls to see which ones have valid JSON
                let raw_tcs = msg["tool_calls"].as_array();
                let valid_tool_count = raw_tcs.map(|tcs| {
                    tcs.iter().filter(|tc| {
                        match &tc["function"]["arguments"] {
                            serde_json::Value::String(s) => serde_json::from_str::<serde_json::Value>(s).is_ok(),
                            serde_json::Value::Object(_) => true,
                            _ => false,
                        }
                    }).count()
                }).unwrap_or(0);

                let has_valid_calls = valid_tool_count > 0;

                if !has_valid_calls && length_retries < MAX_LENGTH_RETRIES {
                    length_retries += 1;
                    log_console!(
                        "[{}] finish_reason=length (retry {}/{})",
                        self.role, length_retries, MAX_LENGTH_RETRIES
                    );
                    self.messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": msg["content"].as_str().unwrap_or("")
                    }));
                    let instruction = if length_retries >= 3 {
                        "上一轮输出再次因长度截断。你现在只剩 256 token 的输出空间。\n必须立即调用一个最小必要工具并输出最简参数（每个参数最多 150 字符）。\n禁止任何解释、禁止任何分析、禁止任何计划。\n如果是 append_document，每次只写 150 字符。"
                    } else {
                        "上一轮输出因长度截断。不要继续解释，不要继续分析，不要重复计划。\n下一轮必须执行以下之一：\n1. 立即调用一个最小必要工具（每个参数最多 150-200 字符）；\n2. 若无法调用工具，只能用一句话说明下一步要调用哪个工具以及原因。\n禁止输出长文本。如果是 append_document，每次只写 150-200 字符。"
                    };
                    self.messages.push(serde_json::json!({
                        "role": "user",
                        "content": instruction
                    }));
                    continue;
                }

                log_console!(
                    "[{}] WARNING: finish_reason=length — response truncated at {} tokens{}",
                    self.role, self.max_tokens,
                    if has_valid_calls { format!(" ({} valid tool calls)", valid_tool_count) } else { " (giving up)".to_string() }
                );
            }

            // Parse tool calls
            if let Some(tcs) = msg["tool_calls"].as_array() {
                if tcs.is_empty() {
                    return Ok(StepResult::Text(
                        msg["content"].as_str().unwrap_or_default().to_string(),
                    ));
                }

            let mut calls = Vec::new();
            for tc in tcs {
                let id = tc["id"].as_str().unwrap_or("").to_string();
                let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                let args: serde_json::Value = match &tc["function"]["arguments"] {
                    serde_json::Value::String(s) => {
                        match serde_json::from_str(s) {
                            Ok(v) => v,
                            Err(e) => {
                                let preview =
                                    &s[..s.floor_char_boundary(200.min(s.len()))];
                                log_console!(
                                    "[{}] JSON parse error for {}: {} — preview: {}",
                                    self.role, name, e, preview
                                );
                                serde_json::Value::Null
                            }
                        }
                    }
                    v @ serde_json::Value::Object(_) => v.clone(),
                    _ => {
                        log_console!(
                            "[{}] unexpected arguments type for {}: {:?}",
                            self.role, name, tc["function"]["arguments"]
                        );
                        serde_json::Value::Null
                    }
                };

                if args.is_null() {
                    log_console!("[{}] skipping tool call {} due to broken arguments (truncated)", self.role, name);
                    continue;
                }

                let key_arg = if name == "route_to" {
                    args.get("to").and_then(|v| v.as_str()).unwrap_or("?").to_string()
                } else {
                    args.get("path")
                        .or_else(|| args.get("command"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };
                log_console!("[{}] {} {}", self.role, name, key_arg);

                calls.push(ToolCallInfo { id, name, args });
            }

            // If all calls had broken arguments, return text content instead of
            // empty ToolCalls (which would cause control.rs to loop infinitely).
            if calls.is_empty() {
                log_console!("[{}] all tool calls had broken arguments — falling back to text", self.role);
                return Ok(StepResult::Text(
                    msg["content"].as_str().unwrap_or_default().to_string(),
                ));
            }

            // Only push assistant message with valid tool calls remaining.
            // If we pushed the raw msg (which may contain truncated calls),
            // the API would 400 because those IDs never get tool_result.
            let valid_ids: std::collections::HashSet<&str> =
                calls.iter().map(|c| c.id.as_str()).collect();
            let mut filtered = msg.clone();
            if let Some(arr) = filtered["tool_calls"].as_array_mut() {
                arr.retain(|tc| valid_ids.contains(tc["id"].as_str().unwrap_or("")));
            }
            self.messages.push(filtered);
            return Ok(StepResult::ToolCalls(calls));
        } else {
            return Ok(StepResult::Text(
                msg["content"].as_str().unwrap_or_default().to_string(),
            ));
        }
        } // end step loop
    }

    /// Inject a system-level message into the conversation.
    /// Used by AgentController for interrupt / restart signals.
    pub fn inject(&mut self, content: &str) {
        self.messages
            .push(serde_json::json!({"role": "system", "content": content}));
    }

    /// Append a skill-level system message to the conversation.
    /// Skills accumulate in the context (context compression will handle pruning later).
    pub fn inject_skill(&mut self, skill_name: &str, content: &str) {
        let formatted = format!("[skill: {}]\n{}", skill_name, content);
        self.messages.push(serde_json::json!({"role": "system", "content": formatted}));
    }

    /// Replace the previous skill message with a new one (no accumulation).
    /// Falls back to append if no prior `[skill:` message exists.
    pub fn replace_skill(&mut self, skill_name: &str, content: &str) {
        let formatted = format!("[skill: {}]\n{}", skill_name, content);
        let msg = serde_json::json!({"role": "system", "content": formatted});
        if let Some(pos) = self.messages.iter().rposition(|m| {
            m["role"].as_str() == Some("system")
                && m["content"].as_str().map_or(false, |c| c.starts_with("[skill:"))
        }) {
            self.messages[pos] = msg;
        } else {
            self.messages.push(msg);
        }
    }

    /// Append a tool result to the conversation so the LLM sees it.
    pub fn feed_tool_result(&mut self, id: &str, _name: &str, output: &str) {
        self.messages.push(serde_json::json!({
            "role": "tool",
            "content": output,
            "tool_call_id": id,
        }));
    }

    /// Truncate content for log preview, safe for multi-byte text (e.g. Chinese).
    fn preview(s: &str) -> String {
        let char_count = s.chars().count();
        if char_count <= 80 {
            return s.to_string();
        }
        let lines: Vec<&str> = s.lines().collect();
        if lines.len() <= 1 {
            // Single line: show first 30 chars + "..." + last 30 chars
            let head: String = s.chars().take(30).collect();
            let tail: String = s.chars().skip(char_count.saturating_sub(30)).collect();
            format!("{}...{}", head, tail)
        } else {
            // Multi-line: show first 20 chars of first line + last 20 chars of last line
            let first: String = lines[0].chars().take(20).collect();
            let last: String = lines[lines.len()-1].chars().rev().take(20).collect::<String>().chars().rev().collect();
            format!("{}...{} ({}行)", first, last, lines.len())
        }
    }

    /// Send API request, returning the response JSON on success.
    async fn api_request(
        client: &AnthropicClient,
        body: &serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let resp = client
            .http_client
            .post(&client.api_url)
            .header("Authorization", format!("Bearer {}", client.api_key))
            .header("content-type", "application/json")
            .json(body)
            .timeout(API_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                let kind = if e.is_connect() { "连接失败" }
                    else if e.is_timeout() { "请求超时" }
                    else if e.is_body() { "请求体错误" }
                    else { "请求错误" };
                anyhow::anyhow!("[{}] {} {}", client.api_url, kind, e)
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, text);
        }

        Ok(resp.json().await?)
    }

    /// Write truncated output to `.shuji/debug/truncated.md` for debugging.
    fn write_debug_truncated(&self, msg: &serde_json::Value, retry: u32) {
        let dir = match self.debug_dir {
            Some(ref d) => d.clone(),
            None => return,
        };
        let path = dir.join("debug").join("truncated.md");
        let _ = std::fs::create_dir_all(path.parent().unwrap());

        let sep = "─".repeat(60);
        let content = msg["content"].as_str().unwrap_or("");
        let mut lines = vec![
            sep.clone(),
            "# Truncated Output".to_string(),
            format!("Timestamp: {}", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")),
            format!("Role: {}", self.role),
            format!("Model: {}", self.model),
            format!("Max Tokens: {}", self.max_tokens),
            format!("Retry: {}/5", retry + 1),
            String::new(),
            "## Content".to_string(),
            if content.is_empty() { "(empty)".to_string() } else { content.to_string() },
            String::new(),
        ];

        if let Some(tcs) = msg["tool_calls"].as_array() {
            if !tcs.is_empty() {
                lines.push("## Tool Calls".to_string());
                for (i, tc) in tcs.iter().enumerate() {
                    let name = tc["function"]["name"].as_str().unwrap_or("?");
                    let args_raw = tc["function"]["arguments"].as_str().unwrap_or("(non-string)");
                    lines.push(format!("{}. {} — args ({} chars):", i + 1, name, args_raw.len()));
                    if args_raw.len() > 500 {
                        let truncated: String = args_raw.chars().take(500).collect();
                        lines.push(format!("{}...", truncated));
                    } else {
                        lines.push(args_raw.to_string());
                    }
                    lines.push(String::new());
                }
            }
        }

        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(&path)
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(f, "{}", lines.join("\n"))
            });
    }

    /// Snapshot the current message history (for interrupt/restore).
    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            messages: self.messages.clone(),
        }
    }

    /// Restore a previous message history.
    pub fn restore(&mut self, snap: &SessionSnapshot) {
        self.messages = snap.messages.clone();
    }
}
