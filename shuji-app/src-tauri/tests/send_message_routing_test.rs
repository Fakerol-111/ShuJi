//! send_message 双路径与 PlanRuntime 持久化测试。

mod common;

use shuji_app_lib::pipeline::{should_resume_from_disk, PipelinePlan, PlanRuntime, PlanStep};

fn sample_runtime() -> PlanRuntime {
    PlanRuntime::new(PipelinePlan {
        plan_id: "resume-test".into(),
        summary: "awaiting user".into(),
        estimated_complexity: "low".into(),
        created: "2026-06-26".into(),
        steps: vec![PlanStep {
            step_id: "ask".into(),
            description: "ask emperor".into(),
            action: "ask_user".into(),
            action_params: serde_json::json!({"question": "name?"}),
            depends_on: vec![],
            require_approval: false,
            on_failure: "wake_cabinet".into(),
            retry: 1,
        }],
    })
}

#[test]
fn send_message_routing_matrix() {
    assert!(should_resume_from_disk(true, false));
    assert!(!should_resume_from_disk(false, false));
    assert!(!should_resume_from_disk(true, true));
    assert!(!should_resume_from_disk(false, true));
}

#[test]
fn plan_runtime_save_load_and_cleanup_roundtrip() {
    let dir = common::create_test_project("runtime_persist");
    let wd = dir.path();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let rt = sample_runtime();
            rt.save_to(wd).await.expect("save runtime");

            let path = PlanRuntime::runtime_file_path(wd);
            assert!(path.is_file(), "runtime.json should exist");

            let loaded = PlanRuntime::load_from(wd)
                .await
                .expect("load runtime from disk");
            assert_eq!(loaded.plan.plan_id, "resume-test");

            PlanRuntime::cleanup(wd).await;
            assert!(!path.exists(), "cleanup should remove runtime.json");
        });
}

#[test]
fn paused_runtime_on_disk_implies_send_message_resume_path() {
    let dir = common::create_test_project("resume_decision");
    let wd = dir.path();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            sample_runtime().save_to(wd).await.unwrap();
            let has_runtime = PlanRuntime::load_from(wd).await.is_some();
            assert!(should_resume_from_disk(has_runtime, false));
        });
}
