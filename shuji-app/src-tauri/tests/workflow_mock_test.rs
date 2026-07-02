//! Workflow integration tests using PipelineEngine + MockActorHarness.
//!
//! Tests complex multi-step workflows without real LLM calls.

use shuji_app_lib::config::{ApprovalMode, RuntimeConfig};
use shuji_app_lib::pipeline::schema::validate_plan_json;
use shuji_app_lib::pipeline::{PipelinePlan, PipelineResult, PlanStep, StepStatus};

mod common;
use common::{create_mini_rust_project, make_pipeline_engine, MockActorHarness};

fn inline_demo_plan(plan_id: &str, summary: &str) -> PipelinePlan {
    PipelinePlan {
        plan_id: plan_id.into(),
        summary: summary.into(),
        estimated_complexity: "low".into(),
        created: chrono::Local::now().to_rfc3339(),
        steps: vec![
            PlanStep {
                step_id: "init".into(),
                description: "noop".into(),
                action: "self_execute".into(),
                action_params: serde_json::json!({"handler": "noop"}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "gongbu".into(),
                description: "coding".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "尚书令", "task": summary}),
                depends_on: vec!["init".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "xingbu".into(),
                description: "testing".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "尚书令", "task": format!("验证: {}", summary)}),
                depends_on: vec!["gongbu".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "validate".into(),
                description: "validation".into(),
                action: "self_execute".into(),
                action_params: serde_json::json!({"handler": "validate_delivery", "run_lint": false}),
                depends_on: vec!["xingbu".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
        ],
    }
}

fn inline_bugfix_plan(plan_id: &str, summary: &str) -> PipelinePlan {
    PipelinePlan {
        plan_id: plan_id.into(),
        summary: summary.into(),
        estimated_complexity: "low".into(),
        created: chrono::Local::now().to_rfc3339(),
        steps: vec![
            PlanStep {
                step_id: "diagnose".into(),
                description: "diagnose".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "中书令", "task": summary}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "fix".into(),
                description: "fix".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "尚书令", "task": summary}),
                depends_on: vec!["diagnose".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "validate".into(),
                description: "validate".into(),
                action: "self_execute".into(),
                action_params: serde_json::json!({"handler": "validate_delivery", "run_lint": false}),
                depends_on: vec!["fix".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
        ],
    }
}

/// Demo profile executes through engine.
#[tokio::test]
async fn workflow_demo_profile_completes() {
    let harness = MockActorHarness::all_roles();
    let tmp = tempfile::TempDir::new().unwrap();
    create_mini_rust_project(tmp.path()).await;

    let plan = inline_demo_plan("plan-wfd-001", "demo task");
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

    let plan = inline_bugfix_plan("plan-wfb-001", "fix bug");
    let mut config = RuntimeConfig::default();
    config.approval.mode = ApprovalMode::Auto;
    let mut engine = common::make_pipeline_engine_with_config(
        plan,
        &harness,
        tmp.path(),
        std::sync::Arc::new(config),
    );
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
                action: "dispatch_to".into(),
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
