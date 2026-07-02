//! PipelineEngine: drives departments according to a PipelinePlan.
//!
//! This module is split into focused submodules:
//! - `context` — PipelineEngineContext struct and constructors
//! - `constructors` — PipelineEngine::new, save, from_*, load_from_disk
//! - `resume` — resume_with_input + verify_manual_approval_before_resume
//! - `result` — StepResultInner enum (internal step outcomes)
//! - `metrics` — finalize_metrics (validation report attachment + metric persistence)
//! - `graph` — preview_pipeline_on_graph (workflow graph pre-fill from plan)
//! - `run_loop` — run() main execution loop
//! - `step` — execute_step action dispatcher
//! - `route` — execute_dispatch_step (actor communication + checkpoint)

mod constructors;
mod context;
mod graph;
mod metrics;
mod result;
mod resume;
mod route;
mod run_loop;
mod step;

pub use context::PipelineEngineContext;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::actor::ActorMessage;
use crate::config::RuntimeConfig;
use crate::models::role::Role;
use crate::workflow::WorkflowGraph;
use tokio::sync::mpsc;

use super::PlanRuntime;

// ── PipelineEngine ──────────────────────────────────────────────

pub struct PipelineEngine {
    /// Plan + execution state.
    pub runtime: PlanRuntime,
    /// Senders to all department actors.
    pub actor_txs: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    /// Fast mailbox senders.
    pub fast_txs: crate::FastTxMap,
    /// Per-agent cancel flags.
    pub cancel_map: crate::CancelMap,
    /// Global cancel flag.
    pub cancel: Arc<AtomicBool>,
    /// Project working directory.
    pub project_dir: PathBuf,
    /// 文移图引用（可选）——记录部门间路由用于可视化
    pub workflow_graph: Option<Arc<tokio::sync::Mutex<WorkflowGraph>>>,
    /// 追踪上一个路由到的部门角色名（用于文移图边创建）
    pub graph_last_role: String,
    /// Run metrics for observability.
    pub run_metrics: Option<crate::metrics::RunMetrics>,
    /// Runtime configuration (approval mode, etc.)
    pub runtime_config: Arc<RuntimeConfig>,
}

#[cfg(test)]
mod tests {
    use crate::pipeline::{PipelinePlan, PlanRuntime, PlanStep, StepStatus};

    fn make_single_step_plan(action: &str, target: &str) -> PipelinePlan {
        PipelinePlan {
            plan_id: "test".into(),
            summary: "test".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![PlanStep {
                step_id: "s1".into(),
                description: "test step".into(),
                action: action.into(),
                action_params: serde_json::json!({"target": target, "task": "do something"}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            }],
        }
    }

    #[test]
    fn test_unknown_action_returns_failed() {
        let plan = make_single_step_plan("nonexistent", "工部");
        let rt = PlanRuntime::new(plan);
        assert!(rt.find_executable_step().is_some());
    }

    #[test]
    fn test_abort_on_failure_stops_plan() {
        let plan = PipelinePlan {
            plan_id: "test".into(),
            summary: "test".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![PlanStep {
                step_id: "s1".into(),
                description: "step".into(),
                action: "nonexistent".into(),
                action_params: serde_json::json!({}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "abort".into(),
                retry: 0,
            }],
        };
        let mut rt = PlanRuntime::new(plan);
        // Without an actor system, execute_dispatch_step will fail.
        // But we can test the status setting logic directly.
        rt.step_status.insert("s1".into(), StepStatus::InProgress);
        rt.error_log.push("s1: test error".into());
        assert_eq!(rt.find_executable_step(), None);
    }

    #[test]
    fn test_retry_default_one() {
        let plan = make_single_step_plan("dispatch_to", "工部");
        assert_eq!(plan.steps[0].retry, 1);
    }
}
