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
    /// Number of tool calls that were broken/invalid (missing id, empty name,
    /// broken JSON arguments, etc.).
    pub broken_count: usize,
    /// Names of broken tool calls (useful for recovery hints).
    pub broken_names: Vec<String>,
}

impl ParsedAssistant {
    /// Convert into a `StepResult`.
    ///
    /// - `calls` non-empty, `broken_count` == 0 → `ToolCalls` (all valid)
    /// - `calls` non-empty, `broken_count` > 0 → `ToolCalls` (partial recovery:
    ///   valid calls are included, broken ones skipped — the caller should
    ///   inject a recovery hint about the missing broken calls)
    /// - `calls` empty, `broken_count` > 0 → `InvalidToolCalls` (all broken)
    /// - `calls` empty, `broken_count` == 0 → `Text` (pure text response)
    pub fn into_step_result(self) -> StepResult {
        if self.calls.is_empty() && self.broken_count > 0 {
            let reason = format!(
                "All {} tool calls were invalid: {:?}",
                self.broken_count, self.broken_names
            );
            StepResult::InvalidToolCalls {
                assistant_text: self.assistant_text,
                broken_count: self.broken_count,
                broken_names: self.broken_names,
                reason,
            }
        } else if self.calls.is_empty() {
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
                broken_count: 0,
                broken_names: Vec::new(),
            };
        };

        if tcs.is_empty() {
            return ParsedAssistant {
                calls: Vec::new(),
                filtered_msg: msg.clone(),
                assistant_text,
                broken_count: 0,
                broken_names: Vec::new(),
            };
        }

        let role = self.role();
        let mut calls = Vec::new();
        let mut broken_count = 0;
        let mut broken_names = Vec::new();
        let suffix = if is_truncated { " (truncated)" } else { "" };

        for tc in tcs {
            let id = tc["id"].as_str().unwrap_or("").to_string();
            let name = tc["function"]["name"].as_str().unwrap_or("").to_string();

            // ── P0.3: Strong validation — reject empty id/name ──────────
            if id.is_empty() {
                broken_count += 1;
                let display_name = if name.is_empty() { "<unnamed>" } else { &name };
                broken_names.push(display_name.to_string());
                log_console!(
                    "[{}] skipping tool call {} due to empty tool_call_id{}",
                    role,
                    display_name,
                    suffix
                );
                continue;
            }
            if name.is_empty() {
                broken_count += 1;
                broken_names.push(format!("id={}", id));
                log_console!(
                    "[{}] skipping tool call id={} due to empty function.name{}",
                    role,
                    id,
                    suffix
                );
                continue;
            }

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
                broken_count += 1;
                broken_names.push(name.clone());
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
                "[{}] all {} tool calls were invalid — returning InvalidToolCalls: {:?}",
                role,
                broken_count,
                broken_names
            );
        } else if broken_count > 0 {
            log_console!(
                "[{}] {} valid, {} broken tool calls (broken: {:?})",
                role,
                calls.len(),
                broken_count,
                broken_names
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
            broken_count,
            broken_names,
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

// ── Fuzz tests ────────────────────────────────────────────────────

#[cfg(test)]
mod fuzz_tests {
    use super::*;
    use crate::api::client::LlmClient;
    use crate::config::RuntimeConfig;
    use std::sync::Arc;

    fn test_session() -> super::super::Session {
        let client = Arc::new(LlmClient::new(
            "test-key".into(),
            "https://api.test.com/chat/completions".into(),
        ));
        let config = Arc::new(RuntimeConfig::default());
        super::super::Session::new(
            "test system prompt",
            &[],
            "test-model",
            &[],
            &client,
            &config,
        )
        .with_role("test")
    }

    // ── Normal cases ──────────────────────────────────────────────

    #[test]
    fn fuzz_normal_text_response() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "Hello, world!"
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.assistant_text, "Hello, world!");
    }

    #[test]
    fn fuzz_normal_tool_call() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "Creating file...",
            "tool_calls": [{
                "id": "call_001",
                "type": "function",
                "function": {
                    "name": "create_file",
                    "arguments": "{\"path\":\"main.rs\",\"content\":\"fn main() {}\"}"
                }
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 1);
        assert_eq!(parsed.calls[0].name, "create_file");
    }

    #[test]
    fn fuzz_multiple_tool_calls() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"a.rs\"}"}},
                {"id": "c2", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"b.rs\"}"}},
                {"id": "c3", "type": "function", "function": {"name": "list_dir", "arguments": "{\"path\":\"src\"}"}}
            ]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 3);
    }

    // ── Truncated JSON arguments ──────────────────────────────────

    #[test]
    fn fuzz_truncated_json_arguments() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call_001",
                "type": "function",
                "function": {
                    "name": "create_file",
                    "arguments": "{\"path\":\"main.rs\",\"content\":\"fn ma"
                }
            }]
        });
        let parsed = session.parse_assistant(&msg, true);
        assert!(parsed.calls.is_empty());
    }

    #[test]
    fn fuzz_partial_json_with_some_valid_calls() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"a.rs\"}"}},
                {"id": "c2", "type": "function", "function": {"name": "create_file", "arguments": "{\"path\":\"main.rs\",\"content\":\"fn ma"}}
            ]
        });
        let parsed = session.parse_assistant(&msg, true);
        assert_eq!(parsed.calls.len(), 1);
        assert_eq!(parsed.calls[0].id, "c1");
    }

    // ── Null and missing fields ───────────────────────────────────

    #[test]
    fn fuzz_null_arguments() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call_001",
                "type": "function",
                "function": {"name": "create_file", "arguments": null}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
    }

    #[test]
    fn fuzz_missing_function_name() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call_001",
                "type": "function",
                "function": {"arguments": "{\"path\":\"test.rs\"}"}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        // P0.3: empty name → broken, not accepted
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.broken_count, 1);
        assert!(parsed.broken_names[0].contains("call_001"));
    }

    #[test]
    fn fuzz_missing_tool_call_id() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "type": "function",
                "function": {"name": "read_file", "arguments": "{\"path\":\"test.rs\"}"}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        // P0.3: empty id → broken, not accepted
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.broken_count, 1);
    }

    #[test]
    fn fuzz_null_content() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": null,
            "tool_calls": []
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.assistant_text, "");
    }

    #[test]
    fn fuzz_missing_content_field() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "read_file", "arguments": "{\"path\":\"a.rs\"}"}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 1);
        assert_eq!(parsed.assistant_text, "");
    }

    // ── Non-string/non-object arguments ───────────────────────────

    #[test]
    fn fuzz_arguments_as_number() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "test", "arguments": 42}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
    }

    #[test]
    fn fuzz_arguments_as_array() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "test", "arguments": [1, 2, 3]}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
    }

    #[test]
    fn fuzz_arguments_as_boolean() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "test", "arguments": true}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
    }

    #[test]
    fn fuzz_arguments_as_object() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "test", "arguments": {"path": "test.rs"}}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 1);
    }

    // ── Empty and degenerate inputs ───────────────────────────────

    #[test]
    fn fuzz_empty_tool_calls_array() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "No tools needed",
            "tool_calls": []
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.assistant_text, "No tools needed");
    }

    #[test]
    fn fuzz_no_tool_calls_field() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "Just text"
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
    }

    #[test]
    fn fuzz_empty_message() {
        let session = test_session();
        let msg = serde_json::json!({});
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.assistant_text, "");
    }

    #[test]
    fn fuzz_all_calls_broken_falls_back_to_text() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "I tried but failed",
            "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "test", "arguments": "broken"}},
                {"id": "c2", "type": "function", "function": {"name": "test", "arguments": "{invalid}"}},
                {"id": "c3", "type": "function", "function": {"name": "test", "arguments": "\"unclosed"}}
            ]
        });
        let parsed = session.parse_assistant(&msg, false);
        // P0.2: All broken → calls empty, broken_count > 0
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.broken_count, 3);
        assert_eq!(parsed.assistant_text, "I tried but failed");

        // P0.2: into_step_result must produce InvalidToolCalls, NOT Text
        let result = parsed.into_step_result();
        match result {
            StepResult::InvalidToolCalls { broken_count, .. } => {
                assert_eq!(broken_count, 3);
            }
            other => panic!("Expected InvalidToolCalls, got {:?}", other),
        }
    }

    // ── P0.2/P0.3: Regression tests ─────────────────────────────────

    #[test]
    fn fuzz_all_broken_returns_invalid_tool_calls_not_done() {
        // All tool calls have broken arguments → must NOT become StepResult::Text.
        // The controller uses this to distinguish "model gave up" from
        // "model tried to call tools but the format was broken".
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "Let me create the file...",
            "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "create_file", "arguments": "{broken_json}"}},
                {"id": "c2", "type": "function", "function": {"name": "read_file", "arguments": "not_json_either"}}
            ]
        });
        let parsed = session.parse_assistant(&msg, false);
        let result = parsed.into_step_result();
        match result {
            StepResult::InvalidToolCalls {
                broken_count,
                broken_names,
                assistant_text,
                ..
            } => {
                assert_eq!(broken_count, 2);
                assert_eq!(broken_names.len(), 2);
                assert_eq!(assistant_text, "Let me create the file...");
            }
            other => panic!("Expected InvalidToolCalls, got {:?}", other),
        }
    }

    #[test]
    fn fuzz_empty_id_is_broken() {
        // P0.3: empty tool_call_id must be rejected
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "",
                "type": "function",
                "function": {"name": "read_file", "arguments": "{\"path\":\"test.rs\"}"}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.broken_count, 1);
        let result = parsed.into_step_result();
        match result {
            StepResult::InvalidToolCalls { broken_count, .. } => assert_eq!(broken_count, 1),
            other => panic!("Expected InvalidToolCalls, got {:?}", other),
        }
    }

    #[test]
    fn fuzz_empty_name_is_broken() {
        // P0.3: empty function.name must be rejected
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call_001",
                "type": "function",
                "function": {"arguments": "{\"path\":\"test.rs\"}"}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.broken_count, 1);
    }

    #[test]
    fn fuzz_mixed_valid_and_empty_id() {
        // P0.3: valid tool calls survive even when some have empty ids
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "Mixed",
            "tool_calls": [
                {"id": "valid_1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"a.rs\"}"}},
                {"id": "", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"b.rs\"}"}}
            ]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 1);
        assert_eq!(parsed.broken_count, 1);
        assert_eq!(parsed.calls[0].id, "valid_1");
    }

    // ── Adversarial inputs ────────────────────────────────────────

    #[test]
    fn fuzz_extremely_long_arguments() {
        let session = test_session();
        let long_content = "x".repeat(100_000);
        let args = format!("{{\"path\":\"test.rs\",\"content\":\"{}\"}}", long_content);
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "create_file", "arguments": args}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 1);
        let content = parsed.calls[0].args["content"].as_str().unwrap();
        assert_eq!(content.len(), 100_000);
    }

    #[test]
    fn fuzz_unicode_in_arguments() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "create_file", "arguments": "{\"path\":\"测试.rs\",\"content\":\"你好世界 🌍\"}"}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 1);
        let path = parsed.calls[0].args["path"].as_str().unwrap();
        assert_eq!(path, "测试.rs");
    }

    #[test]
    fn fuzz_deeply_nested_json_arguments() {
        let session = test_session();
        let mut args = "{\"path\":\"test.rs\"}".to_string();
        for i in 0..50 {
            args = format!("{{\"level{}\":{}}}", i, args);
        }
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "test", "arguments": args}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 1);
    }

    #[test]
    fn fuzz_arguments_with_escaped_quotes() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "c1",
                "type": "function",
                "function": {"name": "create_file", "arguments": "{\"path\":\"test.rs\",\"content\":\"fn main() { println!(\\\"hello\\\"); }\"}"}
            }]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 1);
    }

    // ── Message structure edge cases ──────────────────────────────

    #[test]
    fn fuzz_tool_calls_as_non_array() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "test",
            "tool_calls": "not_an_array"
        });
        let parsed = session.parse_assistant(&msg, false);
        assert!(parsed.calls.is_empty());
        assert_eq!(parsed.assistant_text, "test");
    }

    #[test]
    fn fuzz_content_as_number() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": 42
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.assistant_text, "");
    }

    #[test]
    fn fuzz_tool_call_without_function_field() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{"id": "c1", "type": "function"}]
        });
        let parsed = session.parse_assistant(&msg, false);
        // Should not panic — call may be kept with empty name/null args or skipped
        let _ = parsed.calls.len();
    }

    #[test]
    fn fuzz_mixed_valid_invalid_null() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "Mixed results",
            "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"a.rs\"}"}},
                {"id": "c2", "type": "function", "function": {"name": "bad_tool", "arguments": null}},
                {"id": "c3", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"b.rs\"}"}},
                {"id": "c4", "type": "function", "function": {"name": "broken", "arguments": "{invalid json}"}},
                {"id": "c5", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"c.rs\"}"}}
            ]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 3);
        let ids: Vec<&str> = parsed.calls.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"c1"));
        assert!(ids.contains(&"c3"));
        assert!(ids.contains(&"c5"));
    }

    #[test]
    fn fuzz_filtered_msg_contains_only_valid_calls() {
        let session = test_session();
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"a.rs\"}"}},
                {"id": "c2", "type": "function", "function": {"name": "bad", "arguments": "broken"}}
            ]
        });
        let parsed = session.parse_assistant(&msg, false);
        assert_eq!(parsed.calls.len(), 1);
        let filtered_tcs = parsed.filtered_msg["tool_calls"].as_array().unwrap();
        assert_eq!(filtered_tcs.len(), 1);
        assert_eq!(filtered_tcs[0]["id"].as_str().unwrap(), "c1");
    }
}
