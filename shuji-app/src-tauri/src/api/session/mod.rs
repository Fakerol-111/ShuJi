use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::api::client::AnthropicClient;
use crate::api::client::ToolDefinition;
use crate::config::RuntimeConfig;

pub mod persisted_context;
pub use persisted_context::*;

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
    ToolCalls {
        calls: Vec<ToolCallInfo>,
        /// Text content from the assistant message that also contained tool_calls.
        /// The actor layer uses this to display text alongside tool execution results.
        text: String,
    },
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

    /// Access the messages for inspection (testing, debugging).
    pub fn messages(&self) -> &[serde_json::Value] {
        &self.messages
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
    max_tokens: Option<u32>,
    role: String,
    /// Whether this session has write_file tool (affects retry token floor).
    #[allow(dead_code)] // reserved for retry token floor logic
    has_write_file: bool,
    /// Force tool_choice = "none" when set (e.g. during skill selection).
    /// Prevents the LLM from calling tools until a mode is selected.
    tool_choice_none: bool,
    /// If set, truncated output is written here for debugging.
    debug_dir: Option<PathBuf>,
    /// Runtime configuration
    config: Arc<RuntimeConfig>,
    /// Control reasoning/thinking output (None = use config default, Some = override)
    reasoning_enabled: Option<bool>,
    /// Reasoning config from RuntimeConfig
    reasoning_config: crate::config::ReasoningConfig,
}

impl Session {
    /// Build a new Session from a system prompt, message history,
    /// model name, tool definitions, and an API client reference.
    pub fn new(
        system: &str,
        history: &[crate::models::message::Message],
        model: &str,
        tools: &[ToolDefinition],
        client: &Arc<AnthropicClient>,
        config: &Arc<RuntimeConfig>,
    ) -> Self {
        let mut messages: Vec<serde_json::Value> = Vec::new();
        messages.push(serde_json::json!({"role": "system", "content": system}));
        for m in history {
            messages.push(serde_json::json!({"role": m.role, "content": m.content}));
        }
        if messages.len() <= 1 {
            messages.push(serde_json::json!({"role": "user", "content": "Please continue"}));
        }

        let has_write_file = tools.iter().any(|t| t.function.name == "create_file");
        let has_append_document = tools.iter().any(|t| t.function.name == "append_document");
        let max_tokens = match if has_write_file {
            config.api.max_tokens.write_file
        } else if has_append_document {
            config.api.max_tokens.append_document
        } else if tools.iter().any(|t| t.function.name == "read_file") {
            config.api.max_tokens.readonly
        } else {
            config.api.max_tokens.text_only
        } {
            0 => None,
            tokens => Some(tokens),
        };

        // route_to is NOT injected here anymore — each agent's tools()
        // explicitly includes it if the agent can route.  Sub-agents
        // (expand_requirements, survey_codebase) intentionally lack it.
        let all_tools = tools.to_vec();

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
            config: config.clone(),
            reasoning_enabled: None,
            reasoning_config: config.api.reasoning.clone(),
        }
    }

    /// Set a role label for logging.
    pub fn with_role(mut self, role: &str) -> Self {
        self.role = role.to_string();
        self
    }

    /// Return the role label set via `with_role`.
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Inject a persona system message (`[soul: role]`) right after
    /// the base prompt but before other messages.
    pub fn with_soul(mut self, role: &str, content: &str) -> Self {
        if content.is_empty() {
            return self;
        }
        let soul_msg = serde_json::json!({
            "role": "system",
            "content": format!("[soul: {}]\n{}", role, content)
        });
        // Insert at index 1 (after base prompt)
        if self.messages.len() > 1 {
            self.messages.insert(1, soul_msg);
        } else {
            self.messages.push(soul_msg);
        }
        self
    }

    /// Override the auto-detected max_tokens value.
    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = (tokens != 0).then_some(tokens);
        self
    }

    /// Dynamically set max_tokens (for phase-based control).
    /// Passing 0 removes the request-level max_tokens field for OpenAI-compatible APIs.
    pub fn set_max_tokens(&mut self, tokens: u32) {
        self.max_tokens = (tokens != 0).then_some(tokens);
    }

    /// Enable or disable reasoning/thinking output.
    /// - None: auto-detect based on API URL (enabled for non-Anthropic)
    /// - Some(true): force enable
    /// - Some(false): force disable
    pub fn set_reasoning(&mut self, enabled: bool) {
        self.reasoning_enabled = Some(enabled);
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
        let max_length_retries = self.config.api.length_max_retries;
        let mut api_retries = 0u32;
        let max_api_retries = self.config.api.max_retries;

        loop {
            let mut body = serde_json::json!({
                "model": self.model,
                "messages": self.messages,
                "temperature": 0.1,
                "top_p": 0.9,
                "frequency_penalty": 0.1,
                "seed": 42,
            });
            if let Some(max_tokens) = self.max_tokens {
                body["max_tokens"] = serde_json::json!(max_tokens);
            }

            // Enable thinking/reasoning mode based on config (configurable per API type).
            // - Anthropic: `thinking` field with optional budget_tokens
            // - DeepSeek/OpenAI-compatible: `thinking` field in body (works for models that support it)
            let thinking_enabled = match self.reasoning_enabled {
                Some(enabled) => enabled,
                None => self.reasoning_config.enabled,
            };
            if thinking_enabled {
                if self.client.api_url.contains("anthropic.com") {
                    // Anthropic format: extended thinking with optional budget
                    #[allow(unused_mut)]
                    let mut thinking = serde_json::json!({"type": "enabled"});
                    if self.reasoning_config.budget_tokens > 0 {
                        thinking["budget_tokens"] =
                            serde_json::json!(self.reasoning_config.budget_tokens);
                    }
                    body["thinking"] = thinking;
                } else {
                    // OpenAI-compatible (DeepSeek etc.): thinking parameter
                    body["thinking"] = serde_json::json!({"type": "enabled"});
                }
            }

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

            log_console!(
                "[{}] step: sending {} messages",
                self.role,
                self.messages.len()
            );

            let data = match Self::api_request(&self.client, &body, self.config.api_timeout()).await
            {
                Ok(d) => d,
                Err(e) => {
                    api_retries += 1;
                    if api_retries < max_api_retries {
                        log_console!(
                            "[{}] API request failed (retry {}/{}), retrying in 2s: {}",
                            self.role,
                            api_retries,
                            max_api_retries,
                            e
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    }
                    return Err(e);
                }
            };
            let msg = &data["choices"][0]["message"];
            let finish_reason = data["choices"][0]["finish_reason"].as_str().unwrap_or("");

            // Log reasoning content length for debugging (DeepSeek reasoning_content field)
            if let Some(rc) = msg.get("reasoning_content").and_then(|v| v.as_str()) {
                if !rc.is_empty() {
                    log_console!("[{}] reasoning: {} chars", self.role, rc.chars().count());
                }
            }
            let completion_tokens = data
                .get("usage")
                .and_then(|usage| usage["completion_tokens"].as_u64())
                .unwrap_or(0);

            // Log token usage
            if let Some(usage) = data.get("usage") {
                let prompt = usage["prompt_tokens"].as_u64().unwrap_or(0);
                let cached = usage["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .unwrap_or(0);
                let completion = usage["completion_tokens"].as_u64().unwrap_or(0);
                log_console!(
                    "[{}] tokens: prompt={} cached={} completion={} total={}",
                    self.role,
                    prompt,
                    cached,
                    completion,
                    prompt + completion
                );
                if !self.role.is_empty() {
                    crate::token_tracker::record(
                        &self.role,
                        prompt,
                        cached,
                        completion,
                        &self.model,
                    );
                }
            }

            // Log completion content for debugging
            {
                let text = msg["content"].as_str().unwrap_or("");
                let has_tools = msg["tool_calls"].as_array().is_some_and(|a| !a.is_empty());
                if has_tools {
                    let tool_names: Vec<&str> = msg["tool_calls"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|tc| tc["function"]["name"].as_str())
                                .collect()
                        })
                        .unwrap_or_default();
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
                self.write_debug_truncated(msg, length_retries).await;

                // First pass: parse tool calls to see which ones have valid JSON
                let raw_tcs = msg["tool_calls"].as_array();
                let valid_tool_count = raw_tcs
                    .map(|tcs| {
                        tcs.iter()
                            .filter(|tc| match &tc["function"]["arguments"] {
                                serde_json::Value::String(s) => {
                                    serde_json::from_str::<serde_json::Value>(s).is_ok()
                                }
                                serde_json::Value::Object(_) => true,
                                _ => false,
                            })
                            .count()
                    })
                    .unwrap_or(0);

                let total_tool_count = raw_tcs.map(|tcs| tcs.len()).unwrap_or(0);
                let has_valid_calls = valid_tool_count > 0;
                let broken_count = total_tool_count - valid_tool_count;

                // Collect names of broken tools for the retry hint
                let broken_names: Vec<&str> = raw_tcs
                    .map(|tcs| {
                        tcs.iter()
                            .filter(|tc| match &tc["function"]["arguments"] {
                                serde_json::Value::String(s) => {
                                    serde_json::from_str::<serde_json::Value>(s).is_err()
                                }
                                serde_json::Value::Object(_) => false,
                                _ => true,
                            })
                            .filter_map(|tc| tc["function"]["name"].as_str())
                            .collect()
                    })
                    .unwrap_or_default();

                if length_retries < max_length_retries {
                    let previous_max_tokens = self.max_tokens;
                    self.max_tokens = Some(match self.max_tokens {
                        Some(tokens) => tokens.saturating_mul(2),
                        None => completion_tokens
                            .saturating_mul(2)
                            .try_into()
                            .unwrap_or(u32::MAX),
                    });
                    let previous_max_tokens_label = previous_max_tokens
                        .map(|tokens| tokens.to_string())
                        .unwrap_or_else(|| "unlimited".to_string());
                    log_console!(
                        "[{}] finish_reason=length: max_tokens {} → {}",
                        self.role,
                        previous_max_tokens_label,
                        self.max_tokens.unwrap_or(0)
                    );
                }

                if !has_valid_calls && length_retries < max_length_retries {
                    length_retries += 1;
                    log_console!(
                        "[{}] finish_reason=length (retry {}/{})",
                        self.role,
                        length_retries,
                        max_length_retries
                    );
                    self.messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": msg["content"].as_str().unwrap_or("")
                    }));
                    let names_hint = if broken_names.is_empty() {
                        String::new()
                    } else {
                        format!(" Lost calls: {}.", broken_names.join(", "))
                    };
                    let instruction = format!(
                        "The previous output was truncated due to length. The system has expanded the output space. Please continue. {}\nYou must:\n1. Prioritize completing truncated tool calls;\n2. Maximum 1 tool call per round;\n3. No explanatory text. If more tools are needed, continue in subsequent rounds.",
                        names_hint
                    );
                    self.messages.push(serde_json::json!({
                        "role": "user",
                        "content": instruction
                    }));
                    continue;
                }

                // Mixed case: some valid, some broken. Keep the valid ones,
                // and inject a hint so the LLM re-issues the broken ones.
                if broken_count > 0 {
                    let names_hint = broken_names.join(", ");
                    let hint = format!(
                        "The previous output was truncated due to length. The system has expanded the output space. {} tool call(s) were lost ({}). Please re-issue these tools, maximum 1 per round.",
                        broken_count, names_hint
                    );
                    self.messages.push(serde_json::json!({
                        "role": "user",
                        "content": hint
                    }));
                }

                let max_tokens = self
                    .max_tokens
                    .map(|tokens| tokens.to_string())
                    .unwrap_or_else(|| "unlimited".to_string());
                log_console!(
                    "[{}] WARNING: finish_reason=length — response truncated at {} tokens ({} valid, {} broken)",
                    self.role, max_tokens, valid_tool_count, broken_count
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
                        serde_json::Value::String(s) => match serde_json::from_str(s) {
                            Ok(v) => v,
                            Err(e) => {
                                let preview = &s[..s.floor_char_boundary(200.min(s.len()))];
                                log_console!(
                                    "[{}] JSON parse error for {}: {} — preview: {}",
                                    self.role,
                                    name,
                                    e,
                                    preview
                                );
                                serde_json::Value::Null
                            }
                        },
                        v @ serde_json::Value::Object(_) => v.clone(),
                        _ => {
                            log_console!(
                                "[{}] unexpected arguments type for {}: {:?}",
                                self.role,
                                name,
                                tc["function"]["arguments"]
                            );
                            serde_json::Value::Null
                        }
                    };

                    if args.is_null() {
                        log_console!(
                            "[{}] skipping tool call {} due to broken arguments (truncated)",
                            self.role,
                            name
                        );
                        continue;
                    }

                    let key_arg = if name == "route_to" {
                        args.get("to")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string()
                    } else {
                        args.get("path")
                            .or_else(|| args.get("command"))
                            .or_else(|| args.get("id"))
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
                    log_console!(
                        "[{}] all tool calls had broken arguments — falling back to text",
                        self.role
                    );
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
                let assistant_text = msg["content"].as_str().unwrap_or_default().to_string();
                self.messages.push(filtered);
                return Ok(StepResult::ToolCalls {
                    calls,
                    text: assistant_text,
                });
            } else {
                return Ok(StepResult::Text(
                    msg["content"].as_str().unwrap_or_default().to_string(),
                ));
            }
        } // end step loop
    }

    /// One API round-trip with streaming text/reasoning deltas when supported.
    /// OpenAI-compatible APIs only; falls back to [`step()`] on failure or Anthropic URLs.
    pub async fn step_stream<F>(&mut self, mut on_chunk: F) -> anyhow::Result<StepResult>
    where
        F: FnMut(crate::api::stream::AgentStreamChunk),
    {
        if self.client.api_url.contains("anthropic.com") {
            return self.step().await;
        }

        let mut body = self.build_step_body();
        body["stream"] = serde_json::json!(true);

        log_console!(
            "[{}] step_stream: sending {} messages",
            self.role,
            self.messages.len()
        );

        let resp = match self
            .client
            .http_client
            .post(&self.client.api_url)
            .header("Authorization", format!("Bearer {}", self.client.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .timeout(self.config.api_timeout())
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log_console!(
                    "[{}] step_stream request failed: {}, fallback step()",
                    self.role,
                    e
                );
                return self.step().await;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            log_console!(
                "[{}] step_stream API {}: {}, fallback step()",
                self.role,
                status,
                text.chars().take(120).collect::<String>()
            );
            return self.step().await;
        }

        let streamed =
            match crate::api::stream::consume_openai_agent_stream(resp.bytes_stream(), |chunk| {
                on_chunk(chunk);
                Ok(())
            })
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    log_console!(
                        "[{}] step_stream parse failed: {}, fallback step()",
                        self.role,
                        e
                    );
                    return self.step().await;
                }
            };

        if streamed.finish_reason == "length" {
            log_console!(
                "[{}] step_stream truncated (length), fallback step()",
                self.role
            );
            return self.step().await;
        }

        if !streamed.reasoning_content.is_empty() {
            log_console!(
                "[{}] reasoning(stream): {} chars",
                self.role,
                streamed.reasoning_content.chars().count()
            );
        }

        let msg = serde_json::json!({
            "content": streamed.content,
            "reasoning_content": streamed.reasoning_content,
            "tool_calls": streamed.tool_calls,
        });

        self.process_assistant_message(&msg)
    }

    /// Build the JSON body shared by `step()` and `step_stream()`.
    fn build_step_body(&self) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": self.messages,
            "temperature": 0.1,
            "top_p": 0.9,
            "frequency_penalty": 0.1,
            "seed": 42,
        });
        if let Some(max_tokens) = self.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }

        let thinking_enabled = match self.reasoning_enabled {
            Some(enabled) => enabled,
            None => self.reasoning_config.enabled,
        };
        if thinking_enabled && !self.client.api_url.contains("anthropic.com") {
            body["thinking"] = serde_json::json!({"type": "enabled"});
        }

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
        body
    }

    /// Parse an assistant message (from stream or non-stream API) into StepResult.
    fn process_assistant_message(&mut self, msg: &serde_json::Value) -> anyhow::Result<StepResult> {
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
                    serde_json::Value::String(s) => match serde_json::from_str(s) {
                        Ok(v) => v,
                        Err(e) => {
                            log_console!("[{}] JSON parse error for {}: {}", self.role, name, e);
                            serde_json::Value::Null
                        }
                    },
                    v @ serde_json::Value::Object(_) => v.clone(),
                    _ => serde_json::Value::Null,
                };

                if args.is_null() {
                    log_console!(
                        "[{}] skipping tool call {} due to broken arguments",
                        self.role,
                        name
                    );
                    continue;
                }

                calls.push(ToolCallInfo { id, name, args });
            }

            if calls.is_empty() {
                return Ok(StepResult::Text(
                    msg["content"].as_str().unwrap_or_default().to_string(),
                ));
            }

            let valid_ids: std::collections::HashSet<&str> =
                calls.iter().map(|c| c.id.as_str()).collect();
            let mut filtered = msg.clone();
            if let Some(arr) = filtered["tool_calls"].as_array_mut() {
                arr.retain(|tc| valid_ids.contains(tc["id"].as_str().unwrap_or("")));
            }
            let assistant_text = msg["content"].as_str().unwrap_or_default().to_string();
            self.messages.push(filtered);
            return Ok(StepResult::ToolCalls {
                calls,
                text: assistant_text,
            });
        }

        Ok(StepResult::Text(
            msg["content"].as_str().unwrap_or_default().to_string(),
        ))
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
        self.messages
            .push(serde_json::json!({"role": "system", "content": formatted}));
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
            let last: String = lines[lines.len() - 1]
                .chars()
                .rev()
                .take(20)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            format!("{}...{} ({} lines)", first, last, lines.len())
        }
    }

    /// Send API request, returning the response JSON on success.
    async fn api_request(
        client: &AnthropicClient,
        body: &serde_json::Value,
        timeout: Duration,
    ) -> anyhow::Result<serde_json::Value> {
        let resp = client
            .http_client
            .post(&client.api_url)
            .header("Authorization", format!("Bearer {}", client.api_key))
            .header("content-type", "application/json")
            .json(body)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| {
                let kind = if e.is_connect() {
                    "connection failed"
                } else if e.is_timeout() {
                    "request timeout"
                } else if e.is_body() {
                    "request body error"
                } else {
                    "request error"
                };
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
    /// Dumps the raw message, partial tool calls, and the last messages of
    /// session context (so we can see what caused the truncation).
    async fn write_debug_truncated(&self, msg: &serde_json::Value, retry: u32) {
        let dir = match self.debug_dir {
            Some(ref d) => d.clone(),
            None => return,
        };
        let path = dir.join("debug").join("truncated.md");
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let sep = "─".repeat(60);
        let content = msg["content"].as_str().unwrap_or("");
        let finish = msg
            .get("finish_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        let max_tokens = self
            .max_tokens
            .map(|tokens| tokens.to_string())
            .unwrap_or_else(|| "unlimited".to_string());

        let mut lines = vec![
            sep.clone(),
            "# Truncated Output".to_string(),
            format!(
                "Timestamp: {}",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")
            ),
            format!("Role: {}", self.role),
            format!("Model: {}", self.model),
            format!("Max Tokens: {}", max_tokens),
            format!("Retry: {}/5 — finish_reason: {}", retry + 1, finish),
            format!("Session messages: {} total", self.messages.len()),
            String::new(),
        ];

        // 1. Dump the truncated response content
        lines.push("## Response Content".to_string());
        if content.is_empty() {
            lines.push("(empty — content was fully truncated)".to_string());
            // Dump the raw message JSON to see if anything survived
            lines.push(format!(
                "\nRaw message:\n```json\n{}\n```",
                serde_json::to_string_pretty(msg).unwrap_or_default()
            ));
        } else {
            lines.push(format!("({} chars)", content.chars().count()));
            lines.push(String::new());
            lines.push(content.to_string());
        }
        lines.push(String::new());

        // 2. Dump tool calls (even partial ones with broken JSON args)
        if let Some(tcs) = msg["tool_calls"].as_array() {
            let valid = tcs
                .iter()
                .filter(|tc| match &tc["function"]["arguments"] {
                    serde_json::Value::String(s) => {
                        serde_json::from_str::<serde_json::Value>(s).is_ok()
                    }
                    serde_json::Value::Object(_) => true,
                    _ => false,
                })
                .count();
            lines.push(format!(
                "## Tool Calls ({} total, {} with valid args)",
                tcs.len(),
                valid
            ));
            for (i, tc) in tcs.iter().enumerate() {
                let name = tc["function"]["name"].as_str().unwrap_or("?");
                let args_raw = tc["function"]["arguments"].as_str().unwrap_or("");
                let args_valid = match &tc["function"]["arguments"] {
                    serde_json::Value::String(s) => {
                        serde_json::from_str::<serde_json::Value>(s).is_ok()
                    }
                    serde_json::Value::Object(_) => true,
                    _ => false,
                };
                let status = if args_valid { "✓" } else { "✗ BROKEN" };
                lines.push(format!(
                    "{}. {} {} — args ({} chars):",
                    i + 1,
                    name,
                    status,
                    args_raw.len()
                ));
                // Always show the raw args for broken ones; truncate valid ones only if huge
                if !args_valid || args_raw.len() <= 1000 {
                    lines.push(format!("```\n{}\n```", args_raw));
                } else {
                    let preview: String = args_raw.chars().take(500).collect();
                    lines.push(format!("```\n{}...\n```", preview));
                }
                lines.push(String::new());
            }
        } else {
            lines.push("## Tool Calls".to_string());
            lines.push("(none)".to_string());
        }
        lines.push(String::new());

        // 3. Dump last N session messages to show what led to truncation
        let last_n = 8usize;
        let start = self.messages.len().saturating_sub(last_n);
        lines.push(format!(
            "## Last {} Session Messages (of {})",
            last_n,
            self.messages.len()
        ));
        lines.push("(most recent at bottom — shows what triggered this response)".to_string());
        lines.push(String::new());
        for (j, m) in self.messages.iter().enumerate().skip(start) {
            let role = m["role"].as_str().unwrap_or("");
            let c = m["content"].as_str().unwrap_or("");
            let tool_names: Vec<&str> = m["tool_calls"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|tc| tc["function"]["name"].as_str())
                        .collect()
                })
                .unwrap_or_default();
            let tool_id = m["tool_call_id"].as_str().unwrap_or("");

            if role == "tool" {
                let preview: String = c.chars().take(150).collect();
                let suffix = if c.chars().count() > 150 { "..." } else { "" };
                lines.push(format!(
                    "[{}] tool (id={}) → {}{}",
                    j, tool_id, preview, suffix
                ));
            } else if !tool_names.is_empty() {
                lines.push(format!("[{}] {} → tools: {:?}", j, role, tool_names));
            } else {
                let preview: String = c.chars().take(200).collect();
                let suffix = if c.chars().count() > 200 { "..." } else { "" };
                lines.push(format!("[{}] {} → {}{}", j, role, preview, suffix));
            }
        }

        if let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(&path)
            .await
        {
            use tokio::io::AsyncWriteExt;
            let _ = file
                .write_all(format!("{}\n", lines.join("\n")).as_bytes())
                .await;
        }
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
