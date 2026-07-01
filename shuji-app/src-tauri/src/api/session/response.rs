//! Response parsing for the Session module.
//!
//! Extracted from `mod.rs`. Merges the tool-call parsing logic that was
//! duplicated between `step()` (non-streaming) and
//! `process_assistant_message()` (streaming). Both paths now call
//! `parse_assistant()`, which returns a `ParsedAssistant` struct; the caller
//! decides whether to push the filtered message and how to wrap the result.
//!
//! Behavior preserved:
//! - All tool calls with broken arguments → return `StepResult::Text`
//!   (NOT empty `ToolCalls`, which would cause `control.rs` to loop forever).
//! - The assistant message pushed to history contains only valid tool_calls
//!   (broken ones are filtered out so the API doesn't 400 on unmatched IDs).

use std::collections::HashSet;

use super::types::{StepResult, ToolCallInfo};

/// Result of parsing an assistant message's tool calls.
///
/// The caller is responsible for:
/// 1. Pushing `filtered_msg` to the session history (when `calls` is non-empty).
/// 2. Returning the appropriate `StepResult` based on whether `calls` is empty.
pub(super) struct ParsedAssistant {
    /// Valid (non-broken) tool calls, in original order.
    pub calls: Vec<ToolCallInfo>,
    /// The assistant message with broken tool_calls filtered out.
    /// Push this to history when `calls` is non-empty.
    pub filtered_msg: serde_json::Value,
    /// Text content from the assistant message (for `StepResult::ToolCalls.text`
    /// or `StepResult::Text` fallback).
    pub assistant_text: String,
}

impl ParsedAssistant {
    /// Convert into a `StepResult`. Returns `Text` if all tool calls were
    /// broken (calls empty), otherwise `ToolCalls`.
    pub fn into_step_result(self) -> StepResult {
        if self.calls.is_empty() {
            StepResult::Text(self.assistant_text)
        } else {
            StepResult::ToolCalls {
                calls: self.calls,
                text: self.assistant_text,
            }
        }
    }
}

impl super::Session {
    /// Parse an assistant message into a `ParsedAssistant`.
    ///
    /// Shared by `step()` (non-streaming) and `step_stream()` (streaming).
    /// Handles:
    /// - Empty tool_calls array → all fields empty, caller treats as Text.
    /// - JSON string arguments that fail to parse → logged + skipped.
    /// - Non-string/non-object arguments → logged + skipped.
    /// - All calls broken → `calls` is empty (caller falls back to Text).
    ///
    /// `is_truncated` controls the log message suffix: `step()` passes `true`
    /// when the response was truncated (finish_reason=length), so the skip
    /// message says "broken arguments (truncated)"; `step_stream()` passes
    /// `false`.
    pub(super) fn parse_assistant(
        &self,
        msg: &serde_json::Value,
        is_truncated: bool,
    ) -> ParsedAssistant {
        let assistant_text = msg["content"].as_str().unwrap_or_default().to_string();

        let Some(tcs) = msg["tool_calls"].as_array() else {
            // No tool_calls at all — pure text response.
            return ParsedAssistant {
                calls: Vec::new(),
                filtered_msg: msg.clone(),
                assistant_text,
            };
        };

        if tcs.is_empty() {
            return ParsedAssistant {
                calls: Vec::new(),
                filtered_msg: msg.clone(),
                assistant_text,
            };
        }

        let role = self.role();
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
                            role,
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
                        role,
                        name,
                        tc["function"]["arguments"]
                    );
                    serde_json::Value::Null
                }
            };

            if args.is_null() {
                let suffix = if is_truncated { " (truncated)" } else { "" };
                log_console!(
                    "[{}] skipping tool call {} due to broken arguments{}",
                    role,
                    name,
                    suffix
                );
                continue;
            }

            // Log the tool call with its key argument for debugging.
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
            log_console!("[{}] {} {}", role, name, key_arg);

            calls.push(ToolCallInfo { id, name, args });
        }

        // If all calls had broken arguments, log the fallback.
        if calls.is_empty() {
            log_console!(
                "[{}] all tool calls had broken arguments — falling back to text",
                role
            );
        }

        // Build filtered message: keep only tool_calls whose IDs survived.
        let valid_ids: HashSet<&str> = calls.iter().map(|c| c.id.as_str()).collect();
        let mut filtered = msg.clone();
        if let Some(arr) = filtered["tool_calls"].as_array_mut() {
            arr.retain(|tc| valid_ids.contains(tc["id"].as_str().unwrap_or("")));
        }

        ParsedAssistant {
            calls,
            filtered_msg: filtered,
            assistant_text,
        }
    }

    /// Parse an assistant message and produce a `StepResult`, pushing the
    /// filtered assistant message to history when tool calls are present.
    ///
    /// This is the streaming-path entry point (was `process_assistant_message`).
    /// `is_truncated` is `false` here because streaming truncation falls back
    /// to `step()` before reaching this method.
    pub(super) fn process_assistant_into_result(&mut self, msg: &serde_json::Value) -> StepResult {
        let parsed = self.parse_assistant(msg, false);
        if !parsed.calls.is_empty() {
            self.push_message(parsed.filtered_msg.clone());
        }
        parsed.into_step_result()
    }
}
