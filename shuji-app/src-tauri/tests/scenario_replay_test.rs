//! Scenario replay integration test.
//!
//! Tests the replay framework with sample scenario JSON.

use shuji_app_lib::scenario::{expected_files, load_scenario, validate_scenario};

/// A todo-cli v2 scenario fixture embedded inline.
const TODO_CLI_SCENARIO: &str = r#"{
    "name": "Todo CLI",
    "version": 2,
    "steps": [
        {
            "agent": "neige",
            "mock_response": {
                "content": "收到任务，开始规划",
                "tool_calls": [
                    {
                        "name": "submit_pipeline_plan",
                        "arguments": "{\"plan_id\":\"plan-001\",\"summary\":\"Todo CLI demo\",\"estimated_complexity\":\"low\",\"steps\":[{\"step_id\":\"init\",\"description\":\"初始化\",\"action\":\"self_execute\",\"action_params\":{\"handler\":\"noop\"},\"depends_on\":[]},{\"step_id\":\"gongbu\",\"description\":\"编码\",\"action\":\"route_to\",\"action_params\":{\"target\":\"尚书令\",\"task\":\"实现 Todo CLI\"},\"depends_on\":[\"init\"]},{\"step_id\":\"validate\",\"description\":\"验证\",\"action\":\"self_execute\",\"action_params\":{\"handler\":\"validate_delivery\",\"run_lint\":false},\"depends_on\":[\"gongbu\"]}]}"
                    }
                ]
            },
            "expect_files": [".shuji/pipeline/runtime.json"],
            "expect_audit_events": ["create_document"]
        },
        {
            "agent": "gongbu",
            "mock_response": {
                "content": "开始编码",
                "tool_calls": [
                    {"name": "create_file", "arguments": "{\"path\":\"main.rs\",\"content\":\"fn main() {}\"}"},
                    {"name": "run_tests", "arguments": "{\"scope\":\"unit\"}"}
                ]
            },
            "expect_files": ["main.rs"]
        },
        {
            "agent": "xingbu",
            "mock_response": {
                "content": "运行测试",
                "tool_calls": [
                    {"name": "run_tests", "arguments": "{\"scope\":\"all\"}"}
                ]
            },
            "expect_audit_events": ["validate_delivery"]
        }
    ],
    "expected_final_status": "complete"
}"#;

#[test]
fn test_load_todo_cli_scenario() {
    let scenario = load_scenario(TODO_CLI_SCENARIO).expect("todo-cli scenario should load");
    assert_eq!(scenario.name, "Todo CLI");
    assert_eq!(scenario.version, 2);
    assert_eq!(scenario.steps.len(), 3);
    assert_eq!(scenario.steps[0].agent, "neige");
    assert_eq!(scenario.steps[1].agent, "gongbu");
    assert_eq!(scenario.steps[2].agent, "xingbu");
}

#[test]
fn test_todo_cli_scenario_valid() {
    let scenario = load_scenario(TODO_CLI_SCENARIO).unwrap();
    assert!(
        validate_scenario(&scenario).is_ok(),
        "todo-cli scenario should be valid"
    );
}

#[test]
fn test_todo_cli_expected_files() {
    let scenario = load_scenario(TODO_CLI_SCENARIO).unwrap();
    let files = expected_files(&scenario);
    assert!(files.contains(&".shuji/pipeline/runtime.json".to_string()));
    assert!(files.contains(&"main.rs".to_string()));
}

#[test]
fn test_scenario_with_missing_agent_name() {
    let bad_json = r#"{
        "name": "Bad",
        "version": 2,
        "steps": [
            {"agent": "", "mock_response": {"content": "x"}, "expect_files": []}
        ]
    }"#;
    let scenario = load_scenario(bad_json).unwrap();
    let result = validate_scenario(&scenario);
    assert!(result.is_err(), "should reject empty agent name");
}
