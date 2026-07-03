//! Private accessor methods for Session sub-modules.
//!
//! Extracted from `mod.rs` to keep the root module focused on the struct
//! definition, constructors, and the public API surface.

use crate::api::client::ToolDefinition;

impl super::Session {
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

    pub(super) fn client(&self) -> &std::sync::Arc<crate::api::client::LlmClient> {
        &self.client
    }

    pub(super) fn config(&self) -> &std::sync::Arc<crate::config::RuntimeConfig> {
        &self.config
    }
}
