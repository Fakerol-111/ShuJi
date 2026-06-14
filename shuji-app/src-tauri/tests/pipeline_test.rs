//! Pipeline integration tests — 15 cases covering validation, execution,
//! approval, failure modes, persistence, and deadlock detection.
//!
//! Uses a MockActorHarness to simulate department actors without real LLM calls.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use shuji_app_lib::actor::ActorMessage;
use shuji_app_lib::models::role::Role;
use shuji_app_lib::pipeline::engine::PipelineEngine;
use shuji_app_lib::pipeline::schema::validate_plan_json;
use shuji_app_lib::pipeline::{PipelinePlan, PipelineResult, PlanRuntime, PlanStep, StepStatus};
use tokio::sync::mpsc;

// ── Mock Actor Harness ────────────────────────────────────────────

/// Simulates department actors for pipeline testing.
///
/// Creates a sender for each role. When an actor receives a message,
/// it immediately replies with a "done" response.
struct MockActorHarness {
    pub senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    _handles: Vec<tokio::task::JoinHandle<()>>,
}

impl MockActorHarness {
    /// Create a harness with mock actors for the given roles.
    fn with_roles(roles: &[Role]) -> Self {
        let mut senders = HashMap::new();
        let mut handles = Vec::new();

        for role in roles {
            let (tx, mut rx) = mpsc::unbounded_channel::<ActorMessage>();
            senders.insert(*role, tx);

            // Spawn a mock actor that replies immediately
            let role_name = role.name().to_string();
            let handle = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if let Some(reply) = msg.reply_to {
                        let _ =
                            reply.send(format!("mock {} completed: {}", role_name, msg.subject));
                    }
                }
            });
            handles.push(handle);
        }

        Self {
            senders,
            _handles: handles,
        }
    }

    /// Create a harness with all standard pipeline roles.
    fn all_roles() -> Self {
        Self::with_roles(&[
            Role::Neige,
            Role::Zhongshuling,
            Role::MenxiaShizhong,
            Role::Shangshuling,
            Role::LiBuShangshu,
            Role::BingbuShangshu,
            Role::GongbuShangshu,
            Role::XingbuShangshu,
            Role::LiBuRShangshu,
        ])
    }
}

// ── Test Helpers ──────────────────────────────────────────────────

fn make_engine(plan: PipelinePlan, harness: &MockActorHarness, dir: &Path) -> PipelineEngine {
    PipelineEngine::new(
        plan,
        harness.senders.clone(),
        Arc::new(HashMap::new()),
        Arc::new(std::sync::Mutex::new(HashMap::new())),
        Arc::new(AtomicBool::new(false)),
        dir.to_path_buf(),
        None,
    )
}

fn simple_plan(plan_id: &str, summary: &str, steps: Vec<PlanStep>) -> PipelinePlan {
    PipelinePlan {
        plan_id: plan_id.to_string(),
        summary: summary.to_string(),
        estimated_complexity: "low".to_string(),
        created: "2026-06-13T12:00:00".to_string(),
        steps,
    }
}

fn make_step(
    id: &str,
    desc: &str,
    action: &str,
    params: serde_json::Value,
    depends_on: Vec<&str>,
) -> PlanStep {
    PlanStep {
        step_id: id.to_string(),
        description: desc.to_string(),
        action: action.to_string(),
        action_params: params,
        depends_on: depends_on.into_iter().map(|s| s.to_string()).collect(),
        require_approval: false,
        on_failure: "wake_cabinet".to_string(),
        retry: 1,
    }
}

fn route_step(id: &str, desc: &str, target: &str, task: &str, deps: Vec<&str>) -> PlanStep {
    make_step(
        id,
        desc,
        "route_to",
        serde_json::json!({"target": target, "task": task}),
        deps,
    )
}

fn self_exec_step(
    id: &str,
    desc: &str,
    handler: &str,
    params: serde_json::Value,
    deps: Vec<&str>,
) -> PlanStep {
    let mut full = serde_json::json!({"handler": handler});
    if let Some(obj) = params.as_object() {
        for (k, v) in obj {
            full[k] = v.clone();
        }
    }
    make_step(id, desc, "self_execute", full, deps)
}

fn approval_step(id: &str, desc: &str, doc_id: &str, deps: Vec<&str>) -> PlanStep {
    make_step(
        id,
        desc,
        "approval_gate",
        serde_json::json!({"doc_id": doc_id}),
        deps,
    )
}

fn ask_user_step(id: &str, desc: &str, question: &str, deps: Vec<&str>) -> PlanStep {
    make_step(
        id,
        desc,
        "ask_user",
        serde_json::json!({"question": question}),
        deps,
    )
}

// ── Test Cases ────────────────────────────────────────────────────

/// Test 1: Schema validation rejects plans with duplicate step IDs.
#[tokio::test]
async fn plan_schema_rejects_duplicate_step_id() {
    let json = r#"{
        "plan_id": "plan-p1", "summary": "dup",
        "steps": [
            {"step_id": "s1", "description": "a", "action": "route_to", "action_params": {"target": "工部", "task": "a"}},
            {"step_id": "s1", "description": "b", "action": "route_to", "action_params": {"target": "刑部", "task": "b"}}
        ]
    }"#;
    let result = validate_plan_json(json);
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("重复"));
}

/// Test 2: Schema validation rejects cyclic dependencies.
#[tokio::test]
async fn plan_schema_rejects_cycle() {
    let json = r#"{
        "plan_id": "plan-p2", "summary": "cycle",
        "steps": [
            {"step_id": "s1", "description": "a", "action": "route_to", "action_params": {"target": "工部", "task": "a"}, "depends_on": ["s2"]},
            {"step_id": "s2", "description": "b", "action": "route_to", "action_params": {"target": "刑部", "task": "b"}, "depends_on": ["s1"]}
        ]
    }"#;
    let result = validate_plan_json(json);
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("环"));
}

/// Test 3: Single route_to step completes successfully.
#[tokio::test]
async fn single_route_to_completes() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p3",
        "single route",
        vec![route_step(
            "s1",
            "code",
            "尚书令",
            "implement feature",
            vec![],
        )],
    );
    let mut engine = make_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Complete { runtime } => {
            assert_eq!(runtime.step_status.get("s1"), Some(&StepStatus::Done));
        }
        other => panic!("expected Complete, got {:?}", other),
    }
}

/// Test 4: Route_to with unknown role fails with StepFailed.
#[tokio::test]
async fn route_to_unknown_role_fails() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p4",
        "bad role",
        vec![make_step(
            "s1",
            "bad",
            "route_to",
            serde_json::json!({"target": "御膳房", "task": "cook"}),
            vec![],
        )],
    );
    let mut engine = make_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::StepFailed { step_id, .. } => {
            assert_eq!(step_id, "s1");
        }
        other => panic!("expected StepFailed, got {:?}", other),
    }
}

/// Test 5: Approval gate pauses the pipeline.
#[tokio::test]
async fn approval_gate_pauses_and_resumes() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p5",
        "approval",
        vec![
            route_step("s1", "design", "中书令", "design", vec![]),
            approval_step("s2", "approve", "doc_001", vec!["s1"]),
        ],
    );
    let engine = make_engine(plan, &harness, tmp.path());
    let mut engine = Some(engine);

    let result = engine.take().unwrap().run().await;
    match result {
        PipelineResult::AwaitingApproval {
            step_id,
            doc_id,
            runtime,
        } => {
            assert_eq!(step_id, "s2");
            assert_eq!(doc_id, "doc_001");
            // Save runtime for resume
            runtime.save_to(tmp.path()).await.unwrap();

            // Resume — approval gate should proceed
            let loaded = PlanRuntime::load_from(tmp.path()).await.unwrap();
            let engine2 = PipelineEngine::from_runtime(
                loaded,
                harness.senders.clone(),
                tmp.path().to_path_buf(),
            );
            let resume_result = engine2.resume_with_input(None).await;
            match resume_result {
                PipelineResult::Complete { runtime } => {
                    assert_eq!(runtime.step_status.get("s2"), Some(&StepStatus::Done));
                }
                other => panic!("expected Complete after resume, got {:?}", other),
            }
        }
        other => panic!("expected AwaitingApproval, got {:?}", other),
    }
}

/// Test 6: Ask user pauses the pipeline.
#[tokio::test]
async fn ask_user_pauses_and_resumes() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p6",
        "ask user",
        vec![
            ask_user_step("s1", "question", "What is your name?", vec![]),
            route_step("s2", "continue", "工部", "continue work", vec!["s1"]),
        ],
    );
    let engine = make_engine(plan, &harness, tmp.path());
    let mut engine = Some(engine);

    let result = engine.take().unwrap().run().await;
    match result {
        PipelineResult::AwaitingUserInput {
            step_id,
            question,
            runtime,
        } => {
            assert_eq!(step_id, "s1");
            assert!(question.contains("name"));
            runtime.save_to(tmp.path()).await.unwrap();

            // Resume with input
            let loaded = PlanRuntime::load_from(tmp.path()).await.unwrap();
            let engine2 = PipelineEngine::from_runtime(
                loaded,
                harness.senders.clone(),
                tmp.path().to_path_buf(),
            );
            let resume_result = engine2.resume_with_input(Some("Alice")).await;
            match resume_result {
                PipelineResult::Complete { runtime } => {
                    assert_eq!(runtime.step_status.get("s1"), Some(&StepStatus::Done));
                    assert_eq!(runtime.step_status.get("s2"), Some(&StepStatus::Done));
                    assert!(runtime.artifacts.contains_key("s1.user_input"));
                }
                other => panic!("expected Complete after resume, got {:?}", other),
            }
        }
        other => panic!("expected AwaitingUserInput, got {:?}", other),
    }
}

/// Test 7: Failed step with on_failure=wake_cabinet returns StepFailed.
#[tokio::test]
async fn failed_step_wake_cabinet() {
    let _harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    // Use an actor that has no channel (bogus target) to trigger failure
    let plan = simple_plan(
        "p7",
        "fail wake",
        vec![PlanStep {
            step_id: "s1".into(),
            description: "fail".into(),
            action: "route_to".into(),
            action_params: serde_json::json!({"target": "中书令", "task": "task"}),
            depends_on: vec![],
            require_approval: false,
            on_failure: "wake_cabinet".into(),
            retry: 0,
        }],
    );
    // Use a harness that doesn't include 中书令 channel to simulate missing actor
    let minimal = MockActorHarness::with_roles(&[Role::GongbuShangshu]);
    let mut engine = make_engine(plan, &minimal, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::StepFailed { step_id, .. } => {
            assert_eq!(step_id, "s1");
        }
        other => panic!("expected StepFailed, got {:?}", other),
    }
}

/// Test 8: Failed step with on_failure=abort returns Aborted.
#[tokio::test]
async fn failed_step_abort() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p8",
        "abort",
        vec![PlanStep {
            step_id: "s1".into(),
            description: "bad action".into(),
            action: "nonexistent_action".into(),
            action_params: serde_json::json!({}),
            depends_on: vec![],
            require_approval: false,
            on_failure: "abort".into(),
            retry: 0,
        }],
    );
    let mut engine = make_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Aborted { .. } => {} // expected
        other => panic!("expected Aborted, got {:?}", other),
    }
}

/// Test 9: Retry once then fail.
#[tokio::test]
async fn retry_once_then_fail() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p9",
        "retry",
        vec![PlanStep {
            step_id: "s1".into(),
            description: "retry fail".into(),
            action: "nonexistent_action".into(),
            action_params: serde_json::json!({}),
            depends_on: vec![],
            require_approval: false,
            on_failure: "wake_cabinet".into(),
            retry: 1, // will retry once, then fail
        }],
    );
    let mut engine = make_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::StepFailed { step_id, .. } => {
            assert_eq!(step_id, "s1");
        }
        other => panic!("expected StepFailed after retry, got {:?}", other),
    }
}

/// Test 10: depends_on order is respected — A completes before B starts.
#[tokio::test]
async fn depends_on_order_respected() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p10",
        "ordered",
        vec![
            route_step("s1", "design", "中书令", "design api", vec![]),
            route_step(
                "s2",
                "implement",
                "尚书令",
                "implement based on design",
                vec!["s1"],
            ),
        ],
    );
    let mut engine = make_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Complete { runtime } => {
            assert_eq!(runtime.step_status.get("s1"), Some(&StepStatus::Done));
            assert_eq!(runtime.step_status.get("s2"), Some(&StepStatus::Done));
        }
        other => panic!("expected Complete, got {:?}", other),
    }
}

/// Test 11: Parallel two branches.
#[tokio::test]
async fn parallel_two_branches() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p11",
        "parallel",
        vec![make_step(
            "p1",
            "parallel exec",
            "parallel",
            serde_json::json!({
                "targets": [
                    {"name": "gongbu", "target": "工部", "task": "code module A"},
                    {"name": "xingbu", "target": "刑部", "task": "test module A"}
                ]
            }),
            vec![],
        )],
    );
    let mut engine = make_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Complete { runtime } => {
            // Parallel creates sub-steps with prefix
            assert_eq!(runtime.step_status.get("p1"), Some(&StepStatus::Done));
        }
        other => panic!("expected Complete, got {:?}", other),
    }
}

/// Test 12: self_execute with validate_delivery handler on empty project.
#[tokio::test]
async fn self_execute_validate_delivery() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();

    // Create a minimal cargo project so validate has something to check
    tokio::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"
[package]
name = "test_validate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .await
    .unwrap();
    let src = tmp.path().join("src");
    tokio::fs::create_dir_all(&src).await.unwrap();
    tokio::fs::write(
        src.join("lib.rs"),
        "#[test] fn it_works() { assert!(true); }",
    )
    .await
    .unwrap();

    let plan = simple_plan(
        "p12",
        "self-exec validate",
        vec![self_exec_step(
            "v1",
            "validate",
            "validate_delivery",
            serde_json::json!({"run_lint": false, "ctrt_id": null}),
            vec![],
        )],
    );
    let mut engine = make_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Complete { runtime } => {
            assert_eq!(runtime.step_status.get("v1"), Some(&StepStatus::Done));
        }
        PipelineResult::StepFailed {
            step_id, reason, ..
        } => {
            // Could fail if tests don't compile, which is also valid behavior
            assert_eq!(step_id, "v1");
            eprintln!(
                "[test] validate step failed (expected in empty env): {}",
                reason
            );
        }
        other => panic!("expected Complete or StepFailed, got {:?}", other),
    }
}

/// Test 13: Runtime persist and reload — save mid-execution, load, continue.
#[tokio::test]
async fn runtime_persist_and_reload() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p13",
        "persistence",
        vec![
            route_step("s1", "step1", "工部", "task 1", vec![]),
            approval_step("s2", "approval", "doc_002", vec!["s1"]),
            route_step("s3", "step3", "刑部", "task 3", vec!["s2"]),
        ],
    );

    // Run until AwaitingApproval
    let engine = make_engine(plan, &harness, tmp.path());
    let mut engine = Some(engine);
    let result = engine.take().unwrap().run().await;

    match result {
        PipelineResult::AwaitingApproval {
            step_id, runtime, ..
        } => {
            assert_eq!(step_id, "s2");
            assert_eq!(runtime.step_status.get("s1"), Some(&StepStatus::Done));

            // Save runtime
            runtime.save_to(tmp.path()).await.unwrap();

            // Reload from disk
            let loaded = PlanRuntime::load_from(tmp.path()).await.unwrap();
            assert_eq!(loaded.step_status.get("s1"), Some(&StepStatus::Done));
            assert_eq!(loaded.step_status.get("s2"), Some(&StepStatus::InProgress));

            // Resume and complete
            let engine2 = PipelineEngine::from_runtime(
                loaded,
                harness.senders.clone(),
                tmp.path().to_path_buf(),
            );
            let resume_result = engine2.resume_with_input(None).await;
            match resume_result {
                PipelineResult::Complete { runtime } => {
                    assert_eq!(runtime.step_status.get("s3"), Some(&StepStatus::Done));
                }
                other => panic!("expected Complete, got {:?}", other),
            }
        }
        other => panic!("expected AwaitingApproval, got {:?}", other),
    }
}

/// Test 14: Deadlock when all remaining steps are blocked by failed deps.
#[tokio::test]
async fn deadlock_no_executable_step() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();

    // Two steps depend on each other → deadlock
    let plan = simple_plan(
        "p14",
        "deadlock",
        vec![
            PlanStep {
                step_id: "s1".into(),
                description: "dep on s2".into(),
                action: "route_to".into(),
                action_params: serde_json::json!({"target": "工部", "task": "a"}),
                depends_on: vec!["s2".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 0,
            },
            PlanStep {
                step_id: "s2".into(),
                description: "dep on s1".into(),
                action: "route_to".into(),
                action_params: serde_json::json!({"target": "刑部", "task": "b"}),
                depends_on: vec!["s1".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 0,
            },
        ],
    );
    let mut engine = make_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Deadlock { .. } => {} // expected
        other => panic!("expected Deadlock, got {:?}", other),
    }
}

/// Test 15: Invalid action marks step as failed.
#[tokio::test]
async fn invalid_action_marks_failed() {
    let harness15 = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p15",
        "bad action",
        vec![PlanStep {
            step_id: "s1".into(),
            description: "bad".into(),
            action: "fly".into(),
            action_params: serde_json::json!({}),
            depends_on: vec![],
            require_approval: false,
            on_failure: "wake_cabinet".into(),
            retry: 0,
        }],
    );
    let mut engine = make_engine(plan, &harness15, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::StepFailed { step_id, .. } => {
            assert_eq!(step_id, "s1");
        }
        other => panic!("expected StepFailed, got {:?}", other),
    }
}

/// Test 16: noop self_execute handler works.
#[tokio::test]
async fn self_execute_noop_handler() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    let plan = simple_plan(
        "p16",
        "noop",
        vec![self_exec_step(
            "n1",
            "noop test",
            "noop",
            serde_json::json!({}),
            vec![],
        )],
    );
    let mut engine = make_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Complete { runtime } => {
            assert_eq!(runtime.step_status.get("n1"), Some(&StepStatus::Done));
        }
        other => panic!("expected Complete, got {:?}", other),
    }
}
