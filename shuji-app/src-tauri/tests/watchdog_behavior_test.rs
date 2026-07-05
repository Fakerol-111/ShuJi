//! Watchdog 行为集成测试
//!
//! 使用 wiremock 模拟 LLM API，验证 AgentController::run() 中 watchdog 的运行时行为：
//!   - same-tool 重复检测 → [干预] 提示注入
//!   - 连续错误终止 → RunResult::Stopped
//!   - delete-create 循环检测 → [干预] 提示注入
//!
//! 运行: cargo test --test watchdog_behavior_test -- --nocapture

mod common;

use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use shuji_app_lib::api::client::{LlmClient, ToolDefinition, ToolFunction};
use shuji_app_lib::api::control::AgentController;
use shuji_app_lib::api::session::Session;
use shuji_app_lib::config::RuntimeConfig;

use common::{mock_api_text, mock_api_tool, MockQueue};

/// 构建一个指向 mock HTTP server 的 Session。
fn make_session(api_url: &str, tools: &[ToolDefinition], config: &Arc<RuntimeConfig>) -> Session {
    let client = Arc::new(LlmClient::new(
        "test-key".to_string(),
        format!("{}/chat/completions", api_url),
    ));
    Session::new(
        "你是一个测试助手，请按需调用工具。",
        &[],
        "test-model",
        tools,
        &client,
        config,
    )
}

/// 构建最小工具列表（read_file + create_file + delete_file）
fn minimal_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".into(),
            function: ToolFunction {
                name: "read_file".into(),
                description: "读取文件内容".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: ToolFunction {
                name: "create_file".into(),
                description: "创建文件".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"}
                    },
                    "required": ["path", "content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: ToolFunction {
                name: "delete_file".into(),
                description: "删除文件".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }),
            },
        },
    ]
}

/// 返回成功结果的 tool exec（'static 匹配 ToolFuture 签名）
fn tool_exec_success(
    _name: &str,
    _args: &serde_json::Value,
) -> Pin<Box<dyn std::future::Future<Output = String> + Send + 'static>> {
    let result = serde_json::json!({"ok": true, "data": "mock content"}).to_string();
    Box::pin(async move { result })
}

/// 返回错误结果的 tool exec
fn tool_exec_error(
    _name: &str,
    _args: &serde_json::Value,
) -> Pin<Box<dyn std::future::Future<Output = String> + Send + 'static>> {
    let result = serde_json::json!({"ok": false, "error": "mock error"}).to_string();
    Box::pin(async move { result })
}

/// 从 session snapshot 中提取所有 tool result content 文本
fn extract_tool_results_text(session: &Session) -> String {
    let snap = session.snapshot();
    snap.messages()
        .iter()
        .filter_map(|m| {
            let role = m["role"].as_str().unwrap_or("");
            if role == "tool" {
                m["content"].as_str()
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── 测试 1: same-tool 重复 → [干预] 提示注入 ──────────────

#[tokio::test]
async fn test_watchdog_same_tool_triggers_intervention() {
    let mock_server = wiremock::MockServer::start().await;
    let runtime_config = Arc::new({
        let mut c = RuntimeConfig::default();
        c.checkpoint.interval_secs = 0;
        c.api.max_retries = 0;
        c.api.streaming.enabled = false;
        c.api.timeout_secs = 30;
        c.watchdog.same_tool_warning_count = 3;
        c.watchdog.max_consecutive_errors = 10;
        c
    });
    let tools = minimal_tools();
    let cancel = AtomicBool::new(false);

    // 5 次相同的 read_file("test.txt") → 一个文本响应结束
    let mut responses = Vec::new();
    for _ in 0..5 {
        responses.push(mock_api_tool(
            "read_file",
            serde_json::json!({"path": "test.txt"}),
        ));
    }
    responses.push(mock_api_text("完成"));

    let queue = MockQueue::new(responses);
    queue.clone().mount(&mock_server).await;

    let mut session = make_session(&mock_server.uri(), &tools, &runtime_config);
    let mut controller = AgentController::new();

    let result = controller
        .run(
            &mut session,
            &tool_exec_success,
            &cancel,
            &tools,
            None,
            &runtime_config,
            None,
        )
        .await
        .expect("run should succeed");

    // 验证最终文本
    let output = result.into_text();
    assert!(output.contains("完成"), "应返回文本响应: {}", output);

    // 验证 session 的 tool result 中包含 watchdog [干预] 提示
    let tool_text = extract_tool_results_text(&session);
    assert!(
        tool_text.contains("[Intervention]"),
        "same-tool watchdog should inject [Intervention] after 3rd repetition. tool results: {}",
        tool_text
    );
    assert!(
        tool_text.contains("[playbook: repeated-tool]"),
        "should inject repeated-tool playbook. tool results: {}",
        tool_text
    );
    assert!(
        tool_text.contains("read_file"),
        "Intervention hint should contain tool name. tool results: {}",
        tool_text
    );
}

// ── 测试 2: 连续错误 → RunResult::Stopped ──────────────

#[tokio::test]
async fn test_watchdog_consecutive_errors_stops_agent() {
    let mock_server = wiremock::MockServer::start().await;
    let runtime_config = Arc::new({
        let mut c = RuntimeConfig::default();
        c.checkpoint.interval_secs = 0;
        c.api.max_retries = 0;
        c.api.streaming.enabled = false;
        c.api.timeout_secs = 30;
        c.watchdog.max_consecutive_errors = 3;
        c
    });
    let tools = minimal_tools();
    let cancel = AtomicBool::new(false);

    // 5 次 read_file（全部返回错误）
    let mut responses = Vec::new();
    for _ in 0..5 {
        responses.push(mock_api_tool(
            "read_file",
            serde_json::json!({"path": "test.txt"}),
        ));
    }
    let queue = MockQueue::new(responses);
    queue.clone().mount(&mock_server).await;

    let mut session = make_session(&mock_server.uri(), &tools, &runtime_config);
    let mut controller = AgentController::new();

    let result = controller
        .run(
            &mut session,
            &tool_exec_error,
            &cancel,
            &tools,
            None,
            &runtime_config,
            None,
        )
        .await
        .expect("run should complete");

    // 验证结果是 Stopped（含终止信息）
    let text = result.into_text();
    // Check that the result indicates termination (case-insensitive "terminating" check)
    let text_lower = text.to_lowercase();
    assert!(
        text_lower.contains("terminating"),
        "watchdog should terminate after 3 consecutive errors. Output: {}",
        text
    );

    // 终止前应在 tool result 中注入 consecutive-tool-errors playbook
    let tool_text = extract_tool_results_text(&session);
    assert!(
        tool_text.contains("[playbook: consecutive-tool-errors]"),
        "should inject consecutive-tool-errors playbook before stop. tool results: {}",
        tool_text
    );
}

// ── 测试 4: 只读不写 → read-without-write playbook ──

#[tokio::test]
async fn test_watchdog_read_without_write_triggers_playbook() {
    let mock_server = wiremock::MockServer::start().await;
    let runtime_config = Arc::new({
        let mut c = RuntimeConfig::default();
        c.checkpoint.interval_secs = 0;
        c.api.max_retries = 0;
        c.api.streaming.enabled = false;
        c.api.timeout_secs = 30;
        c.watchdog.read_without_write_warning = 2;
        c.watchdog.max_consecutive_errors = 10;
        c
    });
    let tools = minimal_tools();
    let cancel = AtomicBool::new(false);

    let mut responses = Vec::new();
    for path in ["a.txt", "b.txt", "c.txt"] {
        responses.push(mock_api_tool(
            "read_file",
            serde_json::json!({"path": path}),
        ));
    }
    responses.push(mock_api_text("完成"));

    let queue = MockQueue::new(responses);
    queue.clone().mount(&mock_server).await;

    let mut session = make_session(&mock_server.uri(), &tools, &runtime_config);
    let mut controller = AgentController::new();

    let result = controller
        .run(
            &mut session,
            &tool_exec_success,
            &cancel,
            &tools,
            None,
            &runtime_config,
            None,
        )
        .await
        .expect("run should succeed");

    assert!(result.into_text().contains("完成"));

    let tool_text = extract_tool_results_text(&session);
    assert!(
        tool_text.contains("[playbook: read-without-write]"),
        "read-without-write playbook should be injected. tool results: {}",
        tool_text
    );
}

// ── 测试 3: delete-create 循环检测 → [干预] 提示注入 ──

#[tokio::test]
async fn test_watchdog_delete_create_cycle_triggers_intervention() {
    let mock_server = wiremock::MockServer::start().await;
    let runtime_config = Arc::new({
        let mut c = RuntimeConfig::default();
        c.checkpoint.interval_secs = 0;
        c.api.max_retries = 0;
        c.api.streaming.enabled = false;
        c.api.timeout_secs = 30;
        c.watchdog.delete_create_warning_count = 2;
        c.watchdog.max_consecutive_errors = 10;
        c
    });
    let tools = minimal_tools();
    let cancel = AtomicBool::new(false);

    // delete → create → delete → create → 文本
    let responses = vec![
        mock_api_tool("delete_file", serde_json::json!({"path": "test.txt"})),
        mock_api_tool(
            "create_file",
            serde_json::json!({"path": "test.txt", "content": "v1"}),
        ),
        mock_api_tool("delete_file", serde_json::json!({"path": "test.txt"})),
        mock_api_tool(
            "create_file",
            serde_json::json!({"path": "test.txt", "content": "v2"}),
        ),
        mock_api_text("完成"),
    ];

    let queue = MockQueue::new(responses);
    queue.clone().mount(&mock_server).await;

    let mut session = make_session(&mock_server.uri(), &tools, &runtime_config);
    let mut controller = AgentController::new();

    let result = controller
        .run(
            &mut session,
            &tool_exec_success,
            &cancel,
            &tools,
            None,
            &runtime_config,
            None,
        )
        .await
        .expect("run should succeed");

    let output = result.into_text();
    assert!(output.contains("完成"), "应正常结束: {}", output);

    // 验证 tool result 中包含 delete-create 循环的 [干预] 提示
    let tool_text = extract_tool_results_text(&session);
    assert!(
        tool_text.contains("[Intervention]"),
        "delete-create cycle should inject [Intervention] after 2 cycles. tool results: {}",
        tool_text
    );
    assert!(
        tool_text.contains("[playbook: delete-create-cycle]"),
        "delete-create cycle should inject playbook. tool results: {}",
        tool_text
    );
    assert!(
        tool_text.contains("delete"),
        "Intervention hint should mention the cycle. tool results: {}",
        tool_text
    );
}
