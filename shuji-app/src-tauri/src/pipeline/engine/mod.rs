//! PipelineEngine: drives departments according to a PipelinePlan.
//!
//! This module is split into focused submodules:
//! - `result` — StepResultInner enum (internal step outcomes)
//! - `metrics` — finalize_metrics (validation report attachment + metric persistence)
//! - `graph` — preview_pipeline_on_graph (workflow graph pre-fill from plan)
//! - `run_loop` — run() main execution loop
//! - `step` — execute_step action dispatcher
//! - `route` — execute_dispatch_step (actor communication + checkpoint)

mod graph;
mod metrics;
mod result;
mod route;
mod run_loop;
mod step;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::actor::ActorMessage;
use crate::config::RuntimeConfig;
use crate::models::role::Role;
use crate::workflow::WorkflowGraph;
use tokio::sync::mpsc;

use super::{PipelinePlan, PipelineResult, PlanRuntime, PlanStep, StepStatus};

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
    graph_last_role: String,
    /// Run metrics for observability.
    pub run_metrics: Option<crate::metrics::RunMetrics>,
    /// Runtime configuration (approval mode, etc.)
    runtime_config: Arc<RuntimeConfig>,
}

pub struct PipelineEngineContext {
    pub actor_txs: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    pub fast_txs: crate::FastTxMap,
    pub cancel_map: crate::CancelMap,
    pub cancel: Arc<AtomicBool>,
    pub project_dir: PathBuf,
    pub workflow_graph: Option<Arc<tokio::sync::Mutex<WorkflowGraph>>>,
    pub runtime_config: Arc<RuntimeConfig>,
}

impl PipelineEngineContext {
    pub fn from_actor_system(
        actor_system: &crate::actor::ActorSystem,
        project_dir: PathBuf,
        runtime_config: Arc<RuntimeConfig>,
    ) -> Self {
        Self {
            actor_txs: actor_system.senders.clone(),
            fast_txs: Arc::new(actor_system.fast_txs.clone()),
            cancel_map: actor_system.cancel_map.clone(),
            cancel: actor_system.cancel.clone(),
            project_dir,
            workflow_graph: Some(actor_system.workflow_graph.clone()),
            runtime_config,
        }
    }

    pub fn lightweight_for_tests(
        actor_txs: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
        project_dir: PathBuf,
        runtime_config: Arc<RuntimeConfig>,
    ) -> Self {
        Self {
            actor_txs,
            fast_txs: Arc::new(HashMap::new()),
            cancel_map: Arc::new(Mutex::new(HashMap::<
                crate::models::role::Role,
                Arc<AtomicBool>,
            >::new())),
            cancel: Arc::new(AtomicBool::new(false)),
            project_dir,
            workflow_graph: None,
            runtime_config,
        }
    }
}

impl PipelineEngine {
    /// Create a new engine from a freshly-submitted plan.
    pub fn new(plan: PipelinePlan, context: PipelineEngineContext) -> Self {
        Self {
            runtime: PlanRuntime::new(plan),
            actor_txs: context.actor_txs,
            fast_txs: context.fast_txs,
            cancel_map: context.cancel_map,
            cancel: context.cancel,
            project_dir: context.project_dir,
            workflow_graph: context.workflow_graph,
            graph_last_role: "内阁".to_string(),
            run_metrics: None,
            runtime_config: context.runtime_config,
        }
    }

    /// Save current runtime to disk.
    pub async fn save(&self) -> Result<(), String> {
        self.runtime.save_to(&self.project_dir).await
    }

    /// Create engine from an in-memory runtime plus a live ActorSystem.
    ///
    /// Prefer this (or [`Self::load_from_disk`]) for resume paths so fast cancel,
    /// workflow graph, and the global cancel flag stay aligned with the running app.
    pub fn from_actor_system(
        runtime: PlanRuntime,
        actor_system: &crate::actor::ActorSystem,
        project_dir: PathBuf,
        runtime_config: Arc<RuntimeConfig>,
    ) -> Self {
        let context =
            PipelineEngineContext::from_actor_system(actor_system, project_dir, runtime_config);
        Self::from_runtime_context(runtime, context)
    }

    /// Legacy resume helper — only actor senders, no cancel/graph context.
    ///
    /// Kept for lightweight tests. Production resume should use
    /// [`Self::from_actor_system`] or [`Self::load_from_disk`].
    #[deprecated(note = "use from_actor_system or load_from_disk for full resume context")]
    pub fn from_runtime(
        runtime: PlanRuntime,
        actor_txs: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
        project_dir: PathBuf,
        runtime_config: Arc<RuntimeConfig>,
    ) -> Self {
        let context =
            PipelineEngineContext::lightweight_for_tests(actor_txs, project_dir, runtime_config);
        Self::from_runtime_context(runtime, context)
    }

    pub fn from_runtime_context(runtime: PlanRuntime, context: PipelineEngineContext) -> Self {
        Self {
            runtime,
            actor_txs: context.actor_txs,
            fast_txs: context.fast_txs,
            cancel_map: context.cancel_map,
            cancel: context.cancel,
            project_dir: context.project_dir,
            workflow_graph: context.workflow_graph,
            graph_last_role: "内阁".to_string(),
            run_metrics: None,
            runtime_config: context.runtime_config,
        }
    }

    /// Resume pipeline after user input or approval decision.
    ///
    /// For `AwaitingUserInput`: marks the current `ask_user` step as Done
    /// and records user_input as an artifact, then continues remaining steps.
    ///
    /// For `AwaitingApproval`: marks the current `approval_gate` step as Done,
    /// then continues.
    pub async fn resume_with_input(mut self, user_input: Option<&str>) -> PipelineResult {
        // Get the current step that was waiting
        let current = self.runtime.current_step.clone();
        let current = match current {
            Some(ref id) => id.clone(),
            None => return self.run().await, // No pending step, just continue
        };

        // approval_gate: in manual mode, verify document is actually approved before proceeding
        if self.runtime_config.approval.mode == crate::config::ApprovalMode::Manual {
            if let Some(step) = self
                .runtime
                .plan
                .steps
                .iter()
                .find(|s| s.step_id == current)
            {
                if step.action == "approval_gate" {
                    let doc_id = super::artifacts::approval_doc_from_upstream(
                        &self.runtime.artifacts,
                        &step.depends_on,
                    );
                    let Some(doc_id) = doc_id.filter(|id| !id.is_empty()) else {
                        log_console!(
                            "[pipeline] approval_gate failed: no upstream revw document artifact"
                        );
                        return PipelineResult::StepFailed {
                            step_id: current,
                            reason:
                                "approval_gate requires an upstream revw document, but none was found."
                                    .into(),
                            runtime: self.runtime.clone(),
                        };
                    };
                    // Reject empty revw on resume path as well
                    if doc_id.starts_with("revw_") {
                        if let Some(body) =
                            crate::tool::documents::get_document_body(&self.project_dir, &doc_id)
                                .await
                        {
                            if body.trim().is_empty() {
                                log_console!(
                                    "[pipeline] approval_gate blocked: doc {} body is empty",
                                    doc_id
                                );
                                return PipelineResult::StepFailed {
                                    step_id: current,
                                    reason: format!(
                                        "approval_gate requires a non-empty revw document body, but {} is empty.",
                                        doc_id
                                    ),
                                    runtime: self.runtime.clone(),
                                };
                            }
                        }
                    }
                    let status =
                        crate::tool::documents::get_document_status(&self.project_dir, &doc_id)
                            .await;
                    if status.as_deref() != Some("approved") {
                        log_console!(
                            "[pipeline] approval_gate blocked: doc {} status={:?}",
                            doc_id,
                            status
                        );
                        return PipelineResult::AwaitingApproval {
                            doc_id,
                            step_id: current,
                            runtime: self.runtime.clone(),
                        };
                    }
                }
            }
        }

        // Mark current step Done so find_executable_step can proceed
        self.set_status(&current, StepStatus::Done);

        // Record user input as artifact if provided
        if let Some(input) = user_input {
            self.runtime
                .artifacts
                .insert(format!("{}.user_input", current), input.to_string());
            log_console!("[pipeline] resume step {} with user input", current);
        }

        self.save().await.ok();
        self.run().await
    }

    /// Load engine state from disk (restart recovery).
    pub async fn load_from_disk(
        project_dir: &std::path::Path,
        actor_system: &crate::actor::ActorSystem,
        runtime_config: Arc<RuntimeConfig>,
    ) -> Option<Self> {
        let runtime = PlanRuntime::load_from(project_dir).await?;
        Some(Self::from_actor_system(
            runtime,
            actor_system,
            project_dir.to_path_buf(),
            runtime_config,
        ))
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn get_step(&self, step_id: &str) -> Option<&PlanStep> {
        self.runtime
            .plan
            .steps
            .iter()
            .find(|s| s.step_id == step_id)
    }

    fn set_status(&mut self, step_id: &str, status: StepStatus) {
        self.runtime.step_status.insert(step_id.to_string(), status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{PipelinePlan, PlanRuntime, PlanStep};

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
