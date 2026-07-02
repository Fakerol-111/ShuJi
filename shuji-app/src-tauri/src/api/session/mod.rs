use std::path::PathBuf;
use std::sync::Arc;

use crate::api::client::AnthropicClient;
use crate::api::client::ToolDefinition;
use crate::api::reasoning::{self, LlmProvider};
use crate::config::{ReasoningPhase, ResolvedReasoningPolicy, RuntimeConfig};

pub mod persisted_context;
pub use persisted_context::*;

mod accessors;
mod builder;
mod debug;
mod error_retry;
mod length_retry;
mod logging;
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

    /// Public read-only access to the session message history.
    /// Returns a reference to the internal message array without cloning.
    pub fn messages(&self) -> &[serde_json::Value] {
        &self.messages
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
            let body = self.build_step_body();

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
                    if error_retry::should_retry_without_reasoning(
                        self.reasoning_policy.enabled,
                        reasoning_stripped,
                        &e,
                    ) {
                        log_console!(
                            "[{}] reasoning not supported by provider, retrying without",
                            self.role
                        );
                        reasoning_stripped = true;
                        self.set_reasoning(false);
                        continue;
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
            self.log_completion_preview(msg);

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

            // Parse tool calls via the shared parse_assistant helper
            // (also used by step_stream — single source of truth).
            let parsed = self.parse_assistant(msg, finish_reason == "length");
            if !parsed.calls.is_empty() {
                self.push_message(parsed.filtered_msg);
                return Ok(StepResult::ToolCalls {
                    calls: parsed.calls,
                    text: parsed.assistant_text,
                });
            }
            return Ok(StepResult::Text(parsed.assistant_text));
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
