//! E2E real-API integration tests.
//!
//! These tests call real LLM APIs and require `.env` configuration.
//! They are marked `#[ignore]` and skipped in CI.
//!
//! Run all:  cd src-tauri && cargo test --test e2e_real_api_test -- --nocapture --ignored
//! Run one:  cargo test --test e2e_real_api_test test_name -- --nocapture --ignored
//!
//! Required env vars:
//!   DEFAULT_API_KEY=sk-xxx
//!   DEFAULT_API_URL=https://api.deepseek.com/chat/completions
//!   DEFAULT_MODEL=deepseek-v4-flash

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

mod common;

/// Load .env file into environment variables.
fn load_env() {
    for path in &[".env", "../.env"] {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                std::env::set_var(k.trim(), v.trim().trim_matches('"').trim_matches('\''));
            }
        }
    }
}

/// Check that API credentials are available; skip test gracefully if not.
fn require_api() -> (Arc<shuji_app_lib::api::client::LlmClient>, String) {
    load_env();
    let api_key = std::env::var("NEIGE_API_KEY")
        .or_else(|_| std::env::var("DEFAULT_API_KEY"))
        .unwrap_or_else(|_| {
            eprintln!("SKIP: No API key found (DEFAULT_API_KEY / NEIGE_API_KEY)");
            std::process::exit(0);
        });
    let api_url = std::env::var("DEFAULT_API_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com/chat/completions".into());
    let model = std::env::var("DEFAULT_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".into());
    (
        Arc::new(shuji_app_lib::api::client::LlmClient::new(api_key, api_url)),
        model,
    )
}

/// Set up a task document for sub-agent consumption.
fn setup_task(project_dir: &std::path::Path, task_id: &str, counter: u64, content: &str) {
    let shuji_dir = project_dir.join(".shuji");
    std::fs::create_dir_all(shuji_dir.join("tasks")).unwrap();
    let task_content = format!("---\nid: {}\nrefs: [-1]\n---\n\n{}", task_id, content);
    let task_path = shuji_dir.join("tasks").join(format!("{}.md", task_id));
    std::fs::create_dir_all(task_path.parent().unwrap()).unwrap();
    std::fs::write(&task_path, task_content).unwrap();
    std::fs::write(shuji_dir.join("_counter"), counter.to_string()).unwrap();
}

// ── Scenario fixture tests (no API needed) ─────────────────────────

/// Verify the bundled scenario fixture is well-formed.
#[test]
fn e2e_scenario_fixture_valid() {
    let scenario_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/scenarios/todo-cli-demo.json");
    let content = std::fs::read_to_string(&scenario_path)
        .unwrap_or_else(|e| panic!("Failed to read scenario fixture: {}", e));

    let scenario = shuji_app_lib::scenario::load_scenario(&content).expect("scenario should load");

    assert_eq!(scenario.name, "Todo CLI Demo");
    assert_eq!(scenario.version, 2);
    assert_eq!(scenario.steps.len(), 3, "should have 3 steps");
    assert_eq!(scenario.steps[0].agent, "neige");
    assert_eq!(scenario.steps[1].agent, "gongbu");
    assert_eq!(scenario.steps[2].agent, "xingbu");

    shuji_app_lib::scenario::validate_scenario(&scenario).expect("scenario should be valid");

    // Verify expected files are collected correctly
    let files = shuji_app_lib::scenario::expected_files(&scenario);
    assert!(files.contains(&".shuji/pipeline/runtime.json".to_string()));
    assert!(files.contains(&"src/main.rs".to_string()));
    assert!(files.contains(&"tests/todo_test.rs".to_string()));
}

/// Scenario replay file check with real project structure.
#[tokio::test]
async fn e2e_scenario_replay_file_check() {
    let scenario_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/scenarios/todo-cli-demo.json");
    let content = std::fs::read_to_string(&scenario_path).unwrap();
    let scenario = shuji_app_lib::scenario::load_scenario(&content).unwrap();

    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    // Create expected files
    std::fs::create_dir_all(dir.join(".shuji/pipeline")).unwrap();
    std::fs::write(
        dir.join(".shuji/pipeline/runtime.json"),
        r#"{"plan_id":"plan-todo-001"}"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::create_dir_all(dir.join("tests")).unwrap();
    std::fs::write(dir.join("tests/todo_test.rs"), "#[test] fn t() {}").unwrap();

    // Replay should find all files
    let log = shuji_app_lib::scenario::replay::replay_scenario(&scenario, dir)
        .await
        .expect("replay should succeed");

    assert!(log.contains("回放完成"), "log should indicate completion");
    assert!(
        !log.contains("文件缺失"),
        "no files should be missing: {}",
        log
    );
}

/// Scenario replay detects missing files correctly.
#[tokio::test]
async fn e2e_scenario_replay_missing_files() {
    let scenario_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/scenarios/todo-cli-demo.json");
    let content = std::fs::read_to_string(&scenario_path).unwrap();
    let scenario = shuji_app_lib::scenario::load_scenario(&content).unwrap();

    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    // Don't create any expected files — replay should report missing
    let log = shuji_app_lib::scenario::replay::replay_scenario(&scenario, dir)
        .await
        .expect("replay should succeed");

    assert!(
        log.contains("文件缺失"),
        "should report missing files: {}",
        log
    );
}

// ── Real API tests (marked #[ignore]) ──────────────────────────────

/// E2E: expand_requirements sub-agent with real API.
/// Verifies that a vague requirement is expanded into structured specs.
#[tokio::test]
#[ignore = "requires real API key in .env"]
async fn e2e_expand_requirements_real_api() {
    let (client, model) = require_api();

    let tmp = tempfile::TempDir::new().unwrap();
    let project_dir = tmp.path();
    setup_task(
        project_dir,
        "task_e2e_1",
        1000,
        "皇帝需求：做一个简单的待办事项应用。用户能添加、完成、删除任务。",
    );

    let cancel = AtomicBool::new(false);
    let result = shuji_app_lib::agent::expand_requirements::run(
        "task_e2e_1",
        project_dir,
        &client,
        &model,
        &cancel,
    )
    .await;

    match &result {
        Ok(doc_id) => {
            assert!(!doc_id.is_empty(), "should return a document ID");
            eprintln!("expand_requirements returned doc: {}", doc_id);

            // Verify the document was created
            let req_dir = project_dir.join(".shuji/requirements");
            let exists = std::fs::read_dir(&req_dir)
                .map(|entries| entries.filter_map(|e| e.ok()).count() > 0)
                .unwrap_or(false);
            assert!(exists, "requirements directory should contain documents");
        }
        Err(e) => panic!("expand_requirements failed: {}", e),
    }
}

/// E2E: survey_codebase sub-agent with real API.
/// Verifies that the codebase survey produces an analysis document.
#[tokio::test]
#[ignore = "requires real API key in .env"]
async fn e2e_survey_codebase_real_api() {
    let (client, model) = require_api();

    let tmp = tempfile::TempDir::new().unwrap();
    let project_dir = tmp.path();

    // Create a minimal project to survey
    std::fs::write(
        project_dir.join("Cargo.toml"),
        r#"[package]
name = "demo"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(project_dir.join("src")).unwrap();
    std::fs::write(
        project_dir.join("src/main.rs"),
        "fn main() { println!(\"hello\"); }\n",
    )
    .unwrap();

    let shuji_dir = project_dir.join(".shuji");
    std::fs::create_dir_all(&shuji_dir).unwrap();
    std::fs::write(shuji_dir.join("_counter"), "0").unwrap();

    let cancel = AtomicBool::new(false);
    let result = shuji_app_lib::agent::survey_codebase::run(
        "分析当前代码库结构并生成分析报告",
        project_dir,
        &client,
        &model,
        &cancel,
    )
    .await;

    match &result {
        Ok(doc_id) => {
            assert!(!doc_id.is_empty(), "should return a document ID");
            eprintln!("survey_codebase returned doc: {}", doc_id);
        }
        Err(e) => panic!("survey_codebase failed: {}", e),
    }
}
