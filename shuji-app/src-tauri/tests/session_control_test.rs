//! Tests for core driver loop components: `sanitize_messages`,
//! `PersistedContext` round-trip, `RunResult` enum, and iteration budget.

use shuji_app_lib::api::session::PersistedContext;

// ── sanitize_messages (tested via PersistedContext::to_messages()) ────────

#[test]
fn sanitize_removes_orphaned_tool_message() {
    let ctx = PersistedContext {
        base_prompt: "system".into(),
        soul_prompt: None,
        context_messages: vec![
            serde_json::json!({"role": "assistant", "content": "hello"}),
            serde_json::json!({"role": "tool", "tool_call_id": "orphan_123", "content": "result"}),
        ],
    };
    let msgs = ctx.to_messages();
    let non_system: Vec<_> = msgs.iter().filter(|m| m["role"] != "system").collect();
    assert_eq!(non_system.len(), 1, "orphan tool message should be removed");
    assert_eq!(non_system[0]["content"], "hello");
}

#[test]
fn sanitize_keeps_matched_tool_message() {
    let ctx = PersistedContext {
        base_prompt: "system".into(),
        soul_prompt: None,
        context_messages: vec![
            serde_json::json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "read_file", "arguments": "{}"}}]
            }),
            serde_json::json!({"role": "tool", "tool_call_id": "call_1", "content": "file content"}),
        ],
    };
    let msgs = ctx.to_messages();
    let tool_msgs: Vec<_> = msgs.iter().filter(|m| m["role"] == "tool").collect();
    assert_eq!(tool_msgs.len(), 1, "matched tool message should be kept");
}

#[test]
fn sanitize_strips_dangling_tool_calls() {
    let ctx = PersistedContext {
        base_prompt: "system".into(),
        soul_prompt: None,
        context_messages: vec![serde_json::json!({
            "role": "assistant",
            "content": "let me read",
            "tool_calls": [{"id": "call_no_result", "type": "function", "function": {"name": "read_file", "arguments": "{}"}}]
        })],
    };
    let msgs = ctx.to_messages();
    let assistant: Vec<_> = msgs.iter().filter(|m| m["role"] == "assistant").collect();
    assert_eq!(assistant.len(), 1, "assistant message should be kept");
    assert!(
        assistant[0]["tool_calls"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(true),
        "dangling tool_calls should be stripped, got: {:?}",
        assistant[0]["tool_calls"]
    );
    assert_eq!(assistant[0]["content"], "let me read");
}

#[test]
fn sanitize_keeps_partial_valid_tool_calls() {
    let ctx = PersistedContext {
        base_prompt: "system".into(),
        soul_prompt: None,
        context_messages: vec![
            serde_json::json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {"id": "call_valid", "type": "function", "function": {"name": "read", "arguments": "{}"}},
                    {"id": "call_broken", "type": "function", "function": {"name": "write", "arguments": "{}"}}
                ]
            }),
            serde_json::json!({"role": "tool", "tool_call_id": "call_valid", "content": "ok"}),
        ],
    };
    let msgs = ctx.to_messages();
    let assistant: Vec<_> = msgs.iter().filter(|m| m["role"] == "assistant").collect();
    let remaining_ids: Vec<String> = assistant[0]["tool_calls"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|tc| tc["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        remaining_ids,
        vec!["call_valid"],
        "only the matched tool call should remain"
    );
}

#[test]
fn sanitize_handles_empty_tool_call_id() {
    let ctx = PersistedContext {
        base_prompt: "system".into(),
        soul_prompt: None,
        context_messages: vec![
            serde_json::json!({"role": "tool", "tool_call_id": "", "content": "empty id"}),
        ],
    };
    let msgs = ctx.to_messages();
    let tool_msgs: Vec<_> = msgs.iter().filter(|m| m["role"] == "tool").collect();
    assert!(
        tool_msgs.is_empty(),
        "tool messages with empty id should be removed"
    );
}

// ── PersistedContext round-trip ──────────────────────────────────────────

#[test]
fn persisted_context_round_trip_preserves_skill_messages() {
    let original = PersistedContext {
        base_prompt: "You are an agent.".into(),
        soul_prompt: Some("[soul: neige]\nI am wise.".into()),
        context_messages: vec![
            serde_json::json!({"role": "system", "content": "[skill: workflow_demo]\nDemo mode active."}),
            serde_json::json!({"role": "user", "content": "do something"}),
            serde_json::json!({"role": "assistant", "content": "ok"}),
        ],
    };
    let msgs = original.to_messages();
    let rehydrated = PersistedContext::from_messages(&msgs);
    assert_eq!(rehydrated.base_prompt, original.base_prompt);
    assert_eq!(rehydrated.soul_prompt, original.soul_prompt);
    assert_eq!(
        rehydrated.context_messages.len(),
        original.context_messages.len()
    );
    let skill_msg = &rehydrated.context_messages[0];
    assert!(
        skill_msg["content"].as_str().unwrap().contains("[skill:"),
        "skill message should survive round-trip"
    );
}

#[test]
fn persisted_context_round_trip_with_summary() {
    let original = PersistedContext {
        base_prompt: "base".into(),
        soul_prompt: None,
        context_messages: vec![
            serde_json::json!({"role": "system", "content": "[对话摘要] Previous work summarized."}),
            serde_json::json!({"role": "user", "content": "continue"}),
        ],
    };
    let msgs = original.to_messages();
    let rehydrated = PersistedContext::from_messages(&msgs);
    assert_eq!(rehydrated.context_messages.len(), 2);
    assert!(rehydrated.context_messages[0]["content"]
        .as_str()
        .unwrap()
        .contains("[对话摘要]"));
}

// ── PersistedContext::trim_tool_results ──────────────────────────────────

#[test]
fn trim_tool_results_truncates_long_content() {
    let long = "x".repeat(5000);
    let mut ctx = PersistedContext {
        base_prompt: "base".into(),
        soul_prompt: None,
        context_messages: vec![
            serde_json::json!({"role": "tool", "tool_call_id": "call_1", "content": long}),
            serde_json::json!({"role": "tool", "tool_call_id": "call_2", "content": "short"}),
        ],
    };
    ctx.trim_tool_results(2000);
    let content_0 = ctx.context_messages[0]["content"].as_str().unwrap();
    assert!(content_0.len() < 5000, "should be truncated");
    assert!(content_0.contains("截断"), "should mark truncation");
    assert_eq!(ctx.context_messages[1]["content"], "short");
}

#[test]
fn trim_tool_results_ignores_non_tool_messages() {
    let mut ctx = PersistedContext {
        base_prompt: "base".into(),
        soul_prompt: None,
        context_messages: vec![
            serde_json::json!({"role": "user", "content": "hello world"}),
            serde_json::json!({"role": "assistant", "content": "ok"}),
        ],
    };
    ctx.trim_tool_results(5);
    assert_eq!(ctx.context_messages[0]["content"], "hello world");
    assert_eq!(ctx.context_messages[1]["content"], "ok");
}

// ── PersistedContext::from_messages layer splitting ───────────────────────

#[test]
fn from_messages_splits_layers_correctly() {
    let msgs = vec![
        serde_json::json!({"role": "system", "content": "base prompt text"}),
        serde_json::json!({"role": "system", "content": "[soul: neige]\nbe wise"}),
        serde_json::json!({"role": "system", "content": "[skill: demo]\ndemo mode"}),
        serde_json::json!({"role": "user", "content": "hello"}),
        serde_json::json!({"role": "assistant", "content": "hi"}),
        serde_json::json!({"role": "system", "content": "[对话摘要] summary"}),
    ];
    let ctx = PersistedContext::from_messages(&msgs);
    assert_eq!(ctx.base_prompt, "base prompt text");
    assert_eq!(ctx.soul_prompt.as_deref(), Some("[soul: neige]\nbe wise"));
    assert_eq!(ctx.context_messages.len(), 4);
    assert!(ctx.context_messages[0]["content"]
        .as_str()
        .unwrap()
        .contains("[skill:"));
    assert_eq!(ctx.context_messages[1]["role"], "user");
    assert_eq!(ctx.context_messages[2]["role"], "assistant");
    assert!(ctx.context_messages[3]["content"]
        .as_str()
        .unwrap()
        .contains("[对话摘要]"));
}

#[test]
fn from_messages_without_soul_still_splits_correctly() {
    let msgs = vec![
        serde_json::json!({"role": "system", "content": "base prompt"}),
        serde_json::json!({"role": "user", "content": "hello"}),
    ];
    let ctx = PersistedContext::from_messages(&msgs);
    assert_eq!(ctx.base_prompt, "base prompt");
    assert!(ctx.soul_prompt.is_none());
    assert_eq!(ctx.context_messages.len(), 1);
}

// ── RunResult enum behavior ──────────────────────────────────────────────

use shuji_app_lib::api::control::{RouteMsgType, RouteTo, RunResult};
use shuji_app_lib::models::role::Role;

#[test]
fn run_result_done_text() {
    let result = RunResult::Done("完成".to_string());
    assert_eq!(result.text(), "完成");
    assert!(result.into_route().is_none());
}

#[test]
fn run_result_routed_text_and_route() {
    let route = RouteTo {
        target: Role::GongbuShangshu,
        msg_type: RouteMsgType::Task,
        subject: "dsgn_001".to_string(),
        payload: None,
    };
    let result = RunResult::Routed {
        text: "已路由".to_string(),
        route: route.clone(),
    };
    assert_eq!(result.text(), "已路由");
    let extracted = result.into_route();
    assert!(extracted.is_some());
    assert_eq!(extracted.unwrap().subject, "dsgn_001");
}

#[test]
fn run_result_stopped_text() {
    let result = RunResult::Stopped("中断".to_string());
    assert_eq!(result.text(), "中断");
    assert!(result.into_route().is_none());
}
