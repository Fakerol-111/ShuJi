//! Session截断处理测试 - 测试finish_reason=length的重试机制
//!
//! 运行: cargo test --test session_test -- --nocapture
//!
//! 注意：这些测试需要mock AnthropicClient，因为我们不想真正调用API

mod common;

// 由于Session依赖真实的AnthropicClient，这些测试主要验证逻辑而非实际API调用
// 实际的截断处理测试需要集成测试或mock框架

#[test]
fn test_mock_text_response_format() {
    let response = common::mock_text_response("Hello, World!");
    
    assert_eq!(response["choices"][0]["message"]["role"], "assistant");
    assert_eq!(response["choices"][0]["message"]["content"], "Hello, World!");
    assert_eq!(response["choices"][0]["finish_reason"], "stop");
}

#[test]
fn test_mock_tool_response_format() {
    let args = serde_json::json!({"path": "test.txt"});
    let response = common::mock_tool_response("read_file", args.clone());
    
    assert_eq!(response["choices"][0]["finish_reason"], "tool_calls");
    
    let tool_calls = response["choices"][0]["message"]["tool_calls"].as_array().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["function"]["name"], "read_file");
    
    let parsed_args: serde_json::Value = serde_json::from_str(
        tool_calls[0]["function"]["arguments"].as_str().unwrap()
    ).unwrap();
    assert_eq!(parsed_args, args);
}

#[test]
fn test_mock_truncated_response_format() {
    let response = common::mock_truncated_response(
        "This is a partial response",
        Some(("read_file", r#"{"path": "test.t"#))
    );
    
    assert_eq!(response["choices"][0]["finish_reason"], "length");
    assert_eq!(response["choices"][0]["message"]["content"], "This is a partial response");
    
    let tool_calls = response["choices"][0]["message"]["tool_calls"].as_array().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["function"]["name"], "read_file");
    
    // 验证参数是截断的（无效JSON）
    let broken_args = tool_calls[0]["function"]["arguments"].as_str().unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(broken_args).is_err());
}

#[test]
fn test_mock_truncated_response_no_tools() {
    let response = common::mock_truncated_response("Partial text only", None);
    
    assert_eq!(response["choices"][0]["finish_reason"], "length");
    assert!(response["choices"][0]["message"]["tool_calls"].is_null());
}

// ── 测试辅助函数：验证JSON解析 ────────────────────────────────

#[test]
fn test_valid_json_tool_arguments() {
    let valid_args = r#"{"path": "test.txt", "content": "hello"}"#;
    let parsed = serde_json::from_str::<serde_json::Value>(valid_args);
    assert!(parsed.is_ok());
}

#[test]
fn test_invalid_json_tool_arguments() {
    let invalid_args = r#"{"path": "test.txt", "content": "hel"#;
    let parsed = serde_json::from_str::<serde_json::Value>(invalid_args);
    assert!(parsed.is_err());
}

// ── 测试场景：模拟截断处理逻辑 ──────────────────────────────────

#[test]
fn test_detect_truncated_tool_call() {
    // 模拟一个被截断的工具调用
    let tool_call = serde_json::json!({
        "id": "call_123",
        "function": {
            "name": "create_file",
            "arguments": r#"{"path": "test.txt", "content": "This is a very long cont"#
        }
    });
    
    // 尝试解析参数
    let args_str = tool_call["function"]["arguments"].as_str().unwrap();
    let parse_result = serde_json::from_str::<serde_json::Value>(args_str);
    
    // 应该解析失败，表明这是一个被截断的调用
    assert!(parse_result.is_err(), "Truncated JSON should fail to parse");
}

#[test]
fn test_detect_valid_tool_call() {
    // 模拟一个完整的工具调用
    let tool_call = serde_json::json!({
        "id": "call_123",
        "function": {
            "name": "create_file",
            "arguments": r#"{"path": "test.txt", "content": "Complete content"}"#
        }
    });
    
    // 尝试解析参数
    let args_str = tool_call["function"]["arguments"].as_str().unwrap();
    let parse_result = serde_json::from_str::<serde_json::Value>(args_str);
    
    // 应该解析成功
    assert!(parse_result.is_ok(), "Valid JSON should parse successfully");
}

// ── 测试场景：混合有效和无效工具调用 ────────────────────────────

#[test]
fn test_mixed_valid_and_broken_tool_calls() {
    let tool_calls = vec![
        serde_json::json!({
            "id": "call_1",
            "function": {
                "name": "read_file",
                "arguments": r#"{"path": "file1.txt"}"#
            }
        }),
        serde_json::json!({
            "id": "call_2",
            "function": {
                "name": "create_file",
                "arguments": r#"{"path": "file2.txt", "content": "truncated"#
            }
        }),
        serde_json::json!({
            "id": "call_3",
            "function": {
                "name": "delete_file",
                "arguments": r#"{"path": "file3.txt"}"#
            }
        }),
    ];
    
    let mut valid_count = 0;
    let mut broken_count = 0;
    let mut broken_names = Vec::new();
    
    for tc in &tool_calls {
        let args_str = tc["function"]["arguments"].as_str().unwrap();
        if serde_json::from_str::<serde_json::Value>(args_str).is_ok() {
            valid_count += 1;
        } else {
            broken_count += 1;
            broken_names.push(tc["function"]["name"].as_str().unwrap());
        }
    }
    
    assert_eq!(valid_count, 2);
    assert_eq!(broken_count, 1);
    assert_eq!(broken_names, vec!["create_file"]);
}

// ── 测试场景：重试提示生成 ────────────────────────────────────

#[test]
fn test_generate_retry_hint_for_broken_tools() {
    let broken_names = vec!["create_file", "modify_file"];
    let hint = format!(
        "上一轮输出因长度截断，有 {} 个工具调用丢失（{}）。请重新调用这些工具，本轮最多 1 个。",
        broken_names.len(),
        broken_names.join("、")
    );
    
    assert!(hint.contains("create_file"));
    assert!(hint.contains("modify_file"));
    assert!(hint.contains("2 个"));
}

// ── 测试场景：max_tokens递减逻辑 ──────────────────────────────

#[test]
fn test_max_tokens_halving_logic() {
    let initial_tokens = 2048u32;
    let mut current_tokens = initial_tokens;
    let mut retry_count = 0;
    const MAX_RETRIES: u32 = 5;
    
    let mut history = vec![initial_tokens];
    
    while retry_count < MAX_RETRIES {
        current_tokens = current_tokens / 2;
        retry_count += 1;
        history.push(current_tokens);
    }
    
    assert_eq!(history, vec![2048, 1024, 512, 256, 128, 64]);
    assert_eq!(retry_count, MAX_RETRIES);
}

// ── 测试场景：工具调用ID过滤 ──────────────────────────────────

#[test]
fn test_filter_tool_calls_by_valid_ids() {
    let all_tool_calls = vec![
        serde_json::json!({
            "id": "call_1",
            "function": {"name": "read_file", "arguments": r#"{"path": "a.txt"}"#}
        }),
        serde_json::json!({
            "id": "call_2",
            "function": {"name": "create_file", "arguments": r#"{"path": "b.txt"#}
        }),
        serde_json::json!({
            "id": "call_3",
            "function": {"name": "delete_file", "arguments": r#"{"path": "c.txt"}"#}
        }),
    ];
    
    // 模拟只有call_1和call_3有效
    let valid_ids: std::collections::HashSet<&str> = 
        vec!["call_1", "call_3"].into_iter().collect();
    
    let filtered: Vec<_> = all_tool_calls.iter()
        .filter(|tc| valid_ids.contains(tc["id"].as_str().unwrap()))
        .collect();
    
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0]["id"], "call_1");
    assert_eq!(filtered[1]["id"], "call_3");
}

// ── 测试场景：空工具调用数组处理 ──────────────────────────────

#[test]
fn test_empty_tool_calls_array() {
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "I cannot help with that.",
                "tool_calls": []
            },
            "finish_reason": "stop"
        }]
    });
    
    let tool_calls = response["choices"][0]["message"]["tool_calls"].as_array().unwrap();
    assert!(tool_calls.is_empty());
}

// ── 测试场景：finish_reason类型 ───────────────────────────────

#[test]
fn test_finish_reason_types() {
    let reasons = vec!["stop", "length", "tool_calls", "content_filter"];
    
    for reason in reasons {
        let response = serde_json::json!({
            "choices": [{
                "message": {"role": "assistant", "content": "test"},
                "finish_reason": reason
            }]
        });
        
        assert_eq!(response["choices"][0]["finish_reason"], reason);
    }
}

// ── 测试场景：token使用统计 ───────────────────────────────────

#[test]
fn test_token_usage_parsing() {
    let response = serde_json::json!({
        "choices": [{
            "message": {"role": "assistant", "content": "test"},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 150,
            "completion_tokens": 75,
            "total_tokens": 225
        }
    });
    
    let usage = &response["usage"];
    assert_eq!(usage["prompt_tokens"], 150);
    assert_eq!(usage["completion_tokens"], 75);
    assert_eq!(usage["total_tokens"], 225);
}

// ── 测试场景：消息历史构建 ────────────────────────────────────

#[test]
fn test_message_history_structure() {
    let mut messages = Vec::new();
    
    // System message
    messages.push(serde_json::json!({
        "role": "system",
        "content": "You are a helpful assistant."
    }));
    
    // User message
    messages.push(serde_json::json!({
        "role": "user",
        "content": "Read file test.txt"
    }));
    
    // Assistant with tool call
    messages.push(serde_json::json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [{
            "id": "call_1",
            "function": {"name": "read_file", "arguments": r#"{"path": "test.txt"}"#}
        }]
    }));
    
    // Tool result
    messages.push(serde_json::json!({
        "role": "tool",
        "content": "File content here",
        "tool_call_id": "call_1"
    }));
    
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[2]["role"], "assistant");
    assert_eq!(messages[3]["role"], "tool");
}
