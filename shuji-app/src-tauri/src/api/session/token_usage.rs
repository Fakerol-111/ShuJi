//! Token usage logging and recording for the Session module.
//!
//! Extracted from `mod.rs`. Two `impl Session` methods:
//! - `log_reasoning_content()` — log the reasoning_content length (DeepSeek).
//! - `log_token_usage()` — parse the `usage` block, log a summary line, and
//!   record into `token_tracker` for the current role.
//!
//! Both were inline blocks in `step()`; pulled out to keep the step loop
//! readable. `completion_tokens` is returned by `log_token_usage` because
//! the length-retry path needs it to expand the token budget.

impl super::Session {
    /// Log the reasoning_content length if present and non-empty.
    /// (DeepSeek-specific field; other providers don't emit it.)
    pub(super) fn log_reasoning_content(&self, msg: &serde_json::Value) {
        if let Some(rc) = msg.get("reasoning_content").and_then(|v| v.as_str()) {
            if !rc.is_empty() {
                log_console!("[{}] reasoning: {} chars", self.role(), rc.chars().count());
            }
        }
    }

    /// Parse `data["usage"]`, log a token summary, and record into
    /// `token_tracker`. Returns `completion_tokens` (0 if absent) — the
    /// caller (length-retry path) uses it to expand the token budget when
    /// `max_tokens` was unset.
    pub(super) fn log_token_usage(&self, data: &serde_json::Value) -> u64 {
        let completion_tokens = data
            .get("usage")
            .and_then(|usage| usage["completion_tokens"].as_u64())
            .unwrap_or(0);

        if let Some(usage) = data.get("usage") {
            let prompt = usage["prompt_tokens"].as_u64().unwrap_or(0);
            let cached = usage["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .unwrap_or(0);
            let completion = usage["completion_tokens"].as_u64().unwrap_or(0);
            log_console!(
                "[{}] tokens: prompt={} cached={} completion={} total={}",
                self.role(),
                prompt,
                cached,
                completion,
                prompt + completion
            );
            if !self.role().is_empty() {
                crate::token_tracker::record(self.role(), prompt, cached, completion, self.model());
            }
        }

        completion_tokens
    }
}
