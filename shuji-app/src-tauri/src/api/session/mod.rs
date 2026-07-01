use std::path::PathBuf;
use std::sync::Arc;

use crate::api::client::AnthropicClient;
use crate::api::client::ToolDefinition;
use crate::api::reasoning::{self, LlmProvider};
use crate::config::{ReasoningEffort, ReasoningPhase, ResolvedReasoningPolicy, RuntimeConfig};

pub mod persisted_context;
pub use persisted_context::*;

mod debug;
mod length_retry;
mod request;
mod response;
mod stream;
mod token_usage;
mod types;
pub use types::{SessionSnapshot, StepResult, ToolCallInfo};

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
    /// Resolved reasoning/thinking policy
    reasoning_policy: ResolvedReasoningPolicy,
    /// Detected LLM provider (used by reasoning adapter)
    provider: LlmProvider,
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

        let provider = reasoning::detect_provider("", model);
        let reasoning_policy = config.resolve_reasoning_policy("session", ReasoningPhase::Default);

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
            reasoning_policy,
            provider,
        }
    }

    /// Set a role label for logging, and re-resolve the reasoning policy for this role.
    pub fn with_role(mut self, role: &str) -> Self {
        self.role = role.to_string();
        self.provider = reasoning::detect_provider(&self.client.api_url, &self.model);
        self.reasoning_policy = self
            .config
            .resolve_reasoning_policy(role, ReasoningPhase::Default);
        self
    }

    /// Return the role label set via `with_role`.
    pub fn role(&self) -> &str {
        &self.role
    }

    // ── Read-only accessors for sub-modules (debug.rs etc.) ───────────────
    // These expose private fields to sibling modules without making them
    // `pub`. Each returns an immutable reference (or copy for Copy types).

    pub(super) fn model(&self) -> &str {
        &self.model
    }

    pub(super) fn max_tokens(&self) -> Option<u32> {
        self.max_tokens
    }

    pub(super) fn debug_dir(&self) -> Option<&std::path::Path> {
        self.debug_dir.as_deref()
    }

    pub(super) fn messages_len(&self) -> usize {
        self.messages.len()
    }

    pub(super) fn messages_iter(&self) -> impl Iterator<Item = &serde_json::Value> {
        self.messages.iter()
    }

    /// Push a message onto the internal history. Used by sub-modules
    /// (response.rs, length_retry.rs) that need to append assistant/user
    /// messages without going through the public `inject` API.
    pub(super) fn push_message(&mut self, msg: serde_json::Value) {
        self.messages.push(msg);
    }

    // ── Read-only accessors for request.rs / token_usage.rs ───────────────
    // These expose private fields to sibling modules without making them `pub`.

    pub(super) fn messages_ref(&self) -> &[serde_json::Value] {
        &self.messages
    }

    pub(super) fn tools_ref(&self) -> &[ToolDefinition] {
        &self.tools
    }

    pub(super) fn tool_choice_none(&self) -> bool {
        self.tool_choice_none
    }

    pub(super) fn provider(&self) -> crate::api::reasoning::LlmProvider {
        self.provider
    }

    pub(super) fn reasoning_policy(&self) -> crate::config::ResolvedReasoningPolicy {
        self.reasoning_policy
    }

    /// Internal max_tokens setter taking `Option<u32>`. Used by length_retry.rs
    /// to expand the token budget. (The public `set_max_tokens` takes `u32`
    /// and treats 0 as "unset"; this one is explicit about the Option.)
    pub(super) fn set_max_tokens_opt(&mut self, tokens: Option<u32>) {
        self.max_tokens = tokens;
    }

    // ── Accessors for stream.rs ───────────────────────────────────────────
    // step_stream() needs the HTTP client, API URL/key, and config timeout.
    // These return references to the Arc'd / owned fields without exposing
    // them publicly.

    pub(super) fn client(&self) -> &Arc<AnthropicClient> {
        &self.client
    }

    pub(super) fn config(&self) -> &Arc<RuntimeConfig> {
        &self.config
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

    /// Replace or insert the soul system message with latest content from disk.
    pub fn replace_soul(&mut self, role: &str, content: &str) {
        if content.trim().is_empty() {
            self.messages.retain(|m| {
                m["role"].as_str() != Some("system")
                    || !m["content"]
                        .as_str()
                        .is_some_and(|c| c.starts_with("[soul:"))
            });
            return;
        }
        let soul_msg = serde_json::json!({
            "role": "system",
            "content": format!("[soul: {}]\n{}", role, content)
        });
        if let Some(idx) = self.messages.iter().position(|m| {
            m["role"].as_str() == Some("system")
                && m["content"]
                    .as_str()
                    .is_some_and(|c| c.starts_with("[soul:"))
        }) {
            self.messages[idx] = soul_msg;
        } else if self.messages.len() > 1 {
            self.messages.insert(1, soul_msg);
        } else {
            self.messages.push(soul_msg);
        }
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

    /// Enable or disable reasoning/thinking output (backward-compatible).
    /// - true: enable with current effort level
    /// - false: disable reasoning entirely
    pub fn set_reasoning(&mut self, enabled: bool) {
        if enabled {
            self.reasoning_policy = ResolvedReasoningPolicy {
                enabled: true,
                effort: self.reasoning_policy.effort.max(ReasoningEffort::Low),
                budget_tokens: self.reasoning_policy.budget_tokens,
            };
        } else {
            self.reasoning_policy = ResolvedReasoningPolicy::disabled();
        }
    }

    /// Set an explicit reasoning policy (full control).
    pub fn set_reasoning_policy(&mut self, policy: ResolvedReasoningPolicy) {
        self.reasoning_policy = policy;
    }

    /// Re-resolve reasoning policy for the given phase (e.g. Planning/Execution for 工部).
    pub fn set_reasoning_phase(&mut self, phase: ReasoningPhase) {
        self.reasoning_policy = self.config.resolve_reasoning_policy(&self.role, phase);
        log_console!(
            "[{}] reasoning phase {:?}: enabled={} effort={} budget={}",
            self.role,
            phase,
            self.reasoning_policy.enabled,
            self.reasoning_policy.effort,
            self.reasoning_policy.budget_tokens,
        );
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
        let mut reasoning_stripped = false;

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

            // Apply reasoning/thinking fields via the centralized adapter
            reasoning::apply_reasoning_to_body(&mut body, self.provider, self.reasoning_policy);

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
                    // Check if this is a reasoning-unsupported error — strip reasoning and retry once
                    let err_str = e.to_string();
                    if self.reasoning_policy.enabled
                        && !reasoning_stripped
                        && (err_str.contains("400") || err_str.contains("422"))
                    {
                        let status_code = err_str
                            .split("API error (")
                            .nth(1)
                            .and_then(|s| s.split(')').next())
                            .and_then(|s| s.split(',').next())
                            .and_then(|s| s.trim().parse::<u16>().ok())
                            .unwrap_or(0);
                        let error_body = err_str
                            .split("API error (")
                            .nth(1)
                            .and_then(|s| s.split("): ").nth(1))
                            .unwrap_or("");
                        if reasoning::looks_like_unsupported_reasoning_error(
                            status_code,
                            error_body,
                        ) {
                            log_console!(
                                "[{}] reasoning not supported by provider, retrying without",
                                self.role
                            );
                            if let Some(obj) = body.as_object_mut() {
                                obj.remove("thinking");
                                obj.remove("reasoning_effort");
                            }
                            reasoning_stripped = true;
                            continue;
                        }
                    }

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

            // ── Log reasoning content + token usage ──────────────────────
            // Delegated to token_usage.rs. `completion_tokens` is needed by
            // the length-retry path below to expand the token budget.
            self.log_reasoning_content(msg);
            let completion_tokens = self.log_token_usage(&data);

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

            // ── Handle truncated responses (finish_reason=length) ────────
            // Delegated to length_retry.rs. Returns Continue (retry scheduled)
            // or Proceed (fall through to normal tool-call parsing).
            if finish_reason == "length" {
                match self
                    .handle_length_truncation(
                        msg,
                        length_retries,
                        max_length_retries,
                        completion_tokens,
                    )
                    .await
                {
                    crate::api::session::length_retry::LengthRetryAction::Continue {
                        length_retries: new_count,
                    } => {
                        length_retries = new_count;
                        continue;
                    }
                    crate::api::session::length_retry::LengthRetryAction::Proceed => {
                        // Fall through to tool-call parsing below.
                    }
                }
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
