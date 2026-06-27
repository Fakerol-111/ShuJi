mod common;

use shuji_app_lib::config::esaa_contract;
use shuji_app_lib::tool;

#[test]
fn dispatch_gate_blocks_zhongshuling_file_write() {
    let dir = common::create_test_project("dispatch_gate");
    let wd = dir.path();
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tool::execute_named_tool(
                "create_file",
                wd,
                &serde_json::json!({"path": "src/main.rs", "content": "x"}),
                "中书令",
            )
            .await
        });
    assert!(result.contains("\"ok\":false"));
    assert!(result.contains("ROLE_GATE") || result.contains("create_file"));
}

#[test]
fn dispatch_gate_allows_gongbu_file_write() {
    let dir = common::create_test_project("dispatch_gate_ok");
    let wd = dir.path();
    std::fs::create_dir_all(wd.join("src")).unwrap();
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tool::execute_named_tool(
                "create_file",
                wd,
                &serde_json::json!({"path": "src/main.rs", "content": "fn main(){}"}),
                "工部尚书",
            )
            .await
        });
    assert!(result.contains("\"ok\":true"), "result: {result}");
}

#[test]
fn builtin_contract_covers_all_departments() {
    let contracts = esaa_contract::builtin_agent_contracts();
    for role in [
        "内阁",
        "中书令",
        "门下侍中",
        "尚书令",
        "吏部尚书",
        "兵部尚书",
        "工部尚书",
        "刑部尚书",
        "礼部尚书",
    ] {
        assert!(
            contracts.effective_for_role(role).is_some(),
            "missing builtin contract for {role}"
        );
    }
}
