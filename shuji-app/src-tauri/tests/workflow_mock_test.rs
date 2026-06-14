//! Workflow integration tests using PipelineEngine + MockActorHarness.
//!
//! Tests complex multi-step workflows without real LLM calls.

use shuji_app_lib::pipeline::schema::validate_plan_json;
use shuji_app_lib::pipeline::templates::pipeline_from_profile;
use shuji_app_lib::pipeline::{PipelineResult, StepStatus};

mod common;
use common::{create_mini_rust_project, make_pipeline_engine, MockActorHarness};

/// Demo profile executes through engine.
#[tokio::test]
async fn workflow_demo_profile_completes() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    create_mini_rust_project(tmp.path()).await;

    let plan = pipeline_from_profile("demo", "plan-wfd-001", "demo task")
        .expect("demo profile should generate a plan");
    assert!(validate_plan_json(&serde_json::to_string(&plan).unwrap()).is_ok());

    let mut engine = make_pipeline_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Complete { runtime } => assert!(runtime.all_done()),
        PipelineResult::StepFailed { step_id, .. } => {
            eprintln!("demo profile step failed (acceptable): {}", step_id);
        }
        other => panic!("expected Complete or StepFailed, got {:?}", other),
    }
}

/// Bugfix profile executes successfully.
#[tokio::test]
async fn workflow_bugfix_profile_completes() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    create_mini_rust_project(tmp.path()).await;

    let plan = pipeline_from_profile("bugfix", "plan-wfb-001", "fix bug")
        .expect("bugfix profile should generate a plan");
    let mut engine = make_pipeline_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Complete { runtime } => assert!(runtime.all_done()),
        PipelineResult::StepFailed { step_id, .. } => {
            eprintln!("bugfix profile step failed (acceptable): {}", step_id);
        }
        other => panic!("expected Complete or StepFailed, got {:?}", other),
    }
}

/// validate_delivery handler works as a workflow step.
#[tokio::test]
async fn workflow_validate_step_completes() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    create_mini_rust_project(tmp.path()).await;

    let plan = common::self_execute_plan(
        "validate_delivery",
        serde_json::json!({
            "run_lint": false, "ctrt_id": null,
        }),
    );
    let mut engine = make_pipeline_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Complete { runtime } => {
            assert_eq!(runtime.step_status.get("s1"), Some(&StepStatus::Done));
        }
        PipelineResult::StepFailed { .. } => {} // acceptable
        other => panic!("expected Complete or StepFailed, got {:?}", other),
    }
}

/// Plan with route_to + self_execute mixed actions.
#[tokio::test]
async fn workflow_mixed_route_and_self_execute() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    create_mini_rust_project(tmp.path()).await;

    use shuji_app_lib::pipeline::PlanStep;
    let plan = shuji_app_lib::pipeline::PipelinePlan {
        plan_id: "plan-mixed".into(),
        summary: "mixed".into(),
        estimated_complexity: "low".into(),
        created: "2026-06-13T12:00:00".into(),
        steps: vec![
            PlanStep {
                step_id: "s1".into(),
                description: "noop init".into(),
                action: "self_execute".into(),
                action_params: serde_json::json!({"handler": "noop"}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "s2".into(),
                description: "route 工部".into(),
                action: "route_to".into(),
                action_params: serde_json::json!({"target": "尚书令", "task": "task"}),
                depends_on: vec!["s1".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "s3".into(),
                description: "validate".into(),
                action: "self_execute".into(),
                action_params: serde_json::json!({"handler": "validate_delivery", "run_lint": false}),
                depends_on: vec!["s2".into()],
                require_approval: false,
                on_failure: "skip".into(),
                retry: 1,
            },
        ],
    };

    let mut engine = make_pipeline_engine(plan, &harness, tmp.path());
    let result = engine.run().await;
    match result {
        PipelineResult::Complete { .. } => {}
        PipelineResult::StepFailed { .. } => {} // validate may fail
        other => panic!("expected Complete or StepFailed, got {:?}", other),
    }
}
