//! Builder and configuration methods for Session.
//!
//! Extracted from `mod.rs` to keep the root module focused on the struct
//! definition, constructor, public API surface, and `step()`.

use std::path::PathBuf;

use crate::config::{ReasoningEffort, ReasoningPhase, ResolvedReasoningPolicy};

impl super::Session {
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
}
