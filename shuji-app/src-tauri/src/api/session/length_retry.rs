//! Length-truncation retry logic for the Session module.
//!
//! Extracted from `step()`. When the API returns `finish_reason=length`,
//! the response was truncated. This module decides what to do:
//!
//! - **No valid tool calls + retries remaining** → expand `max_tokens`,
//!   push the partial assistant content + a "please continue" instruction,
//!   and signal `Continue` so the step loop retries.
//! - **Mixed (some valid, some broken)** → expand `max_tokens`, push a hint
//!   about the lost calls, and signal `Proceed` so normal parsing picks up
//!   the valid calls.
//! - **Valid calls only (no broken)** → just log the warning, signal
//!   `Proceed`.
//! - **No retries remaining** → log the warning, signal `Proceed` (the
//!   valid calls, if any, are parsed normally; if none, text fallback).
//!
//! Behavior preserved bit-for-bit: same `max_tokens` doubling formula, same
//! instruction strings, same log messages.

use serde_json::Value;

/// What `step()` should do after `handle_length_truncation` returns.
pub(super) enum LengthRetryAction {
    /// A retry was scheduled (max_tokens expanded + continue hint pushed).
    /// The loop should `continue`. `length_retries` is the updated count.
    Continue { length_retries: u32 },
    /// No retry — proceed to normal tool-call parsing.
    Proceed,
}

impl super::Session {
    /// Handle a `finish_reason=length` response.
    ///
    /// `length_retries` is the current retry count (0 = first truncation).
    /// `max_length_retries` is the cap from `config.api.length_max_retries`.
    /// `completion_tokens` is from the API's `usage` block — used to expand
    /// the budget when `max_tokens` was previously unset.
    pub(super) async fn handle_length_truncation(
        &mut self,
        msg: &Value,
        length_retries: u32,
        max_length_retries: u32,
        completion_tokens: u64,
    ) -> LengthRetryAction {
        // Dump the truncated response to the debug file (if debug_dir is set).
        self.write_debug_truncated(msg, length_retries).await;

        // ── Count valid vs broken tool calls ───────────────────────────────
        let raw_tcs = msg["tool_calls"].as_array();
        let valid_tool_count = raw_tcs
            .map(|tcs| {
                tcs.iter()
                    .filter(|tc| match &tc["function"]["arguments"] {
                        Value::String(s) => serde_json::from_str::<Value>(s).is_ok(),
                        Value::Object(_) => true,
                        _ => false,
                    })
                    .count()
            })
            .unwrap_or(0);

        let total_tool_count = raw_tcs.map(|tcs| tcs.len()).unwrap_or(0);
        let has_valid_calls = valid_tool_count > 0;
        let broken_count = total_tool_count - valid_tool_count;

        // Collect names of broken tools for the retry hint.
        let broken_names: Vec<&str> = raw_tcs
            .map(|tcs| {
                tcs.iter()
                    .filter(|tc| match &tc["function"]["arguments"] {
                        Value::String(s) => serde_json::from_str::<Value>(s).is_err(),
                        Value::Object(_) => false,
                        _ => true,
                    })
                    .filter_map(|tc| tc["function"]["name"].as_str())
                    .collect()
            })
            .unwrap_or_default();

        // ── Expand max_tokens (if retries remaining) ───────────────────────
        if length_retries < max_length_retries {
            let previous_max_tokens = self.max_tokens();
            let doubled = previous_max_tokens
                .map(|t| t.saturating_mul(2))
                .unwrap_or_else(|| {
                    // completion_tokens is a u64; clamp to u32 range.
                    let doubled_u64 = completion_tokens.saturating_mul(2);
                    if doubled_u64 > u32::MAX as u64 {
                        u32::MAX
                    } else {
                        doubled_u64 as u32
                    }
                });
            self.set_max_tokens_opt(Some(doubled));
            let previous_label = previous_max_tokens
                .map(|tokens| tokens.to_string())
                .unwrap_or_else(|| "unlimited".to_string());
            log_console!(
                "[{}] finish_reason=length: max_tokens {} → {}",
                self.role(),
                previous_label,
                doubled
            );
        }

        // ── No valid calls + retries remaining → retry ─────────────────────
        if !has_valid_calls && length_retries < max_length_retries {
            let new_length_retries = length_retries + 1;
            log_console!(
                "[{}] finish_reason=length (retry {}/{})",
                self.role(),
                new_length_retries,
                max_length_retries
            );
            // Push the partial assistant content so the LLM sees what it
            // managed to produce before truncation.
            self.push_message(serde_json::json!({
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
            self.push_message(serde_json::json!({
                "role": "user",
                "content": instruction
            }));
            return LengthRetryAction::Continue {
                length_retries: new_length_retries,
            };
        }

        // ── Mixed case: some valid, some broken → hint + proceed ───────────
        // Keep the valid ones (normal parsing below handles them), and inject
        // a hint so the LLM re-issues the broken ones in a future round.
        if broken_count > 0 {
            let names_hint = broken_names.join(", ");
            let hint = format!(
                "The previous output was truncated due to length. The system has expanded the output space. {} tool call(s) were lost ({}). Please re-issue these tools, maximum 1 per round.",
                broken_count, names_hint
            );
            self.push_message(serde_json::json!({
                "role": "user",
                "content": hint
            }));
        }

        let max_tokens = self
            .max_tokens()
            .map(|tokens| tokens.to_string())
            .unwrap_or_else(|| "unlimited".to_string());
        log_console!(
            "[{}] WARNING: finish_reason=length — response truncated at {} tokens ({} valid, {} broken)",
            self.role(),
            max_tokens,
            valid_tool_count,
            broken_count
        );

        LengthRetryAction::Proceed
    }
}
