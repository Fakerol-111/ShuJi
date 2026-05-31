//! workflow_demo E2E 集成测试
//!
//! 使用 wiremock 模拟 LLM API，验证完整的 workflow_demo 流程：
//!   Part A — 内阁 workflow_demo 路由 + 文档创建
//!   Part B — 工部尚书 文件创建 + 磁盘产物断言
//!
//! 运行: cargo test --test workflow_demo_test -- --nocapture

mod common;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use shuji_app_lib::agent::gongbushangshu::GongbuShangshuAgent;
use shuji_app_lib::agent::neige::NeigeAgent;
use shuji_app_lib::agent::r#trait::{Agent, AgentInput, LoopDecision};
use shuji_app_lib::api::client::AnthropicClient;
use shuji_app_lib::config::RuntimeConfig;
use shuji_app_lib::models::message::Message;
use shuji_app_lib::models::role::Role;

use common::{create_test_project, mock_api_text, mock_api_tool, MockQueue};

#[tokio::test]
async fn test_workflow_demo_e2e() {
    // ── Setup: mock HTTP server ──────────────────────────────
    let mock_server = wiremock::MockServer::start().await;
    let api_url = format!("{}/chat/completions", mock_server.uri());
    let api_key = "test-key".to_string();

    // Shared runtime config: disable checkpoint, fast timeouts
    let runtime_config = Arc::new({
        let mut c = RuntimeConfig::default();
        c.checkpoint.interval_secs = 0;
        c.api.max_retries = 0;
        c.api.timeout_secs = 30;
        c
    });

    // ── Part A: 内阁 workflow_demo → 路由 + 文档创建 ──────

    // Mock 响应序列:
    //   1. 内阁 skill detection → <skill>workflow_demo</skill>
    //   2. 内阁 create_document
    //   3. 内阁 route_to(尚书令)
    let queue = MockQueue::new(vec![
        mock_api_text("<skill>workflow_demo</skill>"),
        mock_api_tool(
            "create_document",
            serde_json::json!({
                "type": "task",
                "refs": []
            }),
        ),
        mock_api_tool(
            "route_to",
            serde_json::json!({
                "to": "尚书令",
                "subject": "task_0"
            }),
        ),
    ])
    .mount(&mock_server)
    .await;

    let temp = create_test_project("workflow_demo_e2e");
    let working_dir = temp.path().to_path_buf();

    // 内阁 agent
    let neige_client = AnthropicClient::new(api_key.clone(), api_url.clone());
    let cancel = Arc::new(AtomicBool::new(false));
    let neige = NeigeAgent::new(neige_client, "test-model", cancel, None);

    let neige_input = AgentInput {
        role: Role::Neige,
        task_description: "创建一个 greeting.py 文件，输出 'Hello World'".into(),
        context_messages: vec![],
        project_dir: working_dir.clone(),
        working_dir: working_dir.clone(),
        skill_prompts: vec![],
        current_skill: None,
        resume_paused: false,
        context_window_config: Arc::new(HashMap::new()),
        runtime_config: runtime_config.clone(),
    };

    let neige_output = neige
        .execute(&neige_input)
        .await
        .expect("内阁 execute 失败");

    // ── Part A 断言 ──

    assert!(neige_output.route.is_some(), "内阁应输出 route_to 路由指令");
    let route = neige_output.route.as_ref().unwrap();
    assert_eq!(route.target, Role::Shangshuling, "应路由到尚书令");
    assert_eq!(route.subject, "task_0", "路由 subject 应与文档 ID 一致");

    let task_doc = working_dir.join(".shuji").join("tasks").join("task_0.md");
    assert!(task_doc.exists(), "任务文档 task_0.md 应已创建");
    let task_content = tokio::fs::read_to_string(&task_doc).await.unwrap();
    assert!(
        task_content.contains("id: task_0"),
        "文档应有 frontmatter id"
    );
    assert!(task_content.contains("type: task"), "文档类型应为 task");

    // ── Part B: 工部尚书 文件创建 + 磁盘产物断言 ──────────

    // 添加工部尚书所需的 mock 响应（延续使用同一队列）
    queue.push(mock_api_tool(
        "submit_plan",
        serde_json::json!({
            "batches": [{
                "name": "创建 greeting.py",
                "goal": "创建一个 greeting.py 文件，输出 Hello World"
            }]
        }),
    ));
    queue.push(mock_api_tool(
        "create_file",
        serde_json::json!({
            "path": "greeting.py",
            "content": "print('Hello World')"
        }),
    ));
    queue.push(mock_api_tool("complete_task", serde_json::json!({})));

    // 工部尚书 agent
    let gongbu_client = AnthropicClient::new(api_key, api_url);
    let gongbu_cancel = Arc::new(AtomicBool::new(false));
    let gongbu = GongbuShangshuAgent::new(gongbu_client, "test-model", gongbu_cancel);

    // 使用 actor 循环模式：execute() → after_execute() → Continue → re-execute
    let mut context_messages: Vec<Message> = vec![];
    let task_description = "task_0: 创建一个 greeting.py 输出 Hello World".to_string();
    loop {
        let gongbu_input = AgentInput {
            role: Role::GongbuShangshu,
            task_description: task_description.clone(),
            context_messages: context_messages.clone(),
            project_dir: working_dir.clone(),
            working_dir: working_dir.clone(),
            skill_prompts: vec![],
            current_skill: None,
            resume_paused: false,
            context_window_config: Arc::new(HashMap::new()),
            runtime_config: runtime_config.clone(),
        };

        let gongbu_output = gongbu
            .execute(&gongbu_input)
            .await
            .expect("工部尚书 execute 失败");

        match gongbu.after_execute(&gongbu_output) {
            LoopDecision::Done => break,
            LoopDecision::Continue(msg) => {
                context_messages.push(Message::user(&msg));
            }
        }
    }

    // ── Part B 断言 ──

    // 断言：greeting.py 已创建在项目根目录
    let greeting_py = working_dir.join("greeting.py");
    assert!(greeting_py.exists(), "greeting.py 文件应已创建");
    let greeting_content = tokio::fs::read_to_string(&greeting_py).await.unwrap();
    assert_eq!(
        greeting_content.trim(),
        "print('Hello World')",
        "greeting.py 内容应与 mock 指定的 content 一致"
    );

    // 断言：.shuji 结构完整
    assert!(
        working_dir
            .join(".shuji")
            .join("tasks")
            .join("task_0.md")
            .exists(),
        "task_0.md 应保留"
    );
    // 工部尚书会保存运行时上下文（以角色名"工部"命名）
    assert!(
        working_dir
            .join(".shuji")
            .join("context")
            .join("工部.json")
            .exists(),
        "工部尚书上下文应已保存"
    );

    // 断言：所有 mock 响应均已消费
    assert_eq!(
        queue.remaining(),
        0,
        "所有 mock 响应应被消费（剩余 {} 个）",
        queue.remaining()
    );
}
