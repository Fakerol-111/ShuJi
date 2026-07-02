//! PipelineEngine constructors and load/save helpers.
//!
//! Extracted from `mod.rs`. Groups all construction paths together so the
//! root module can declare the struct and delegate to these impl blocks.

use std::collections::HashMap;
use std::sync::Arc;

use crate::actor::ActorMessage;
use crate::config::RuntimeConfig;
use crate::models::role::Role;
use crate::pipeline::PlanRuntime;
use tokio::sync::mpsc;

use super::context::PipelineEngineContext;
use super::PipelineEngine;

impl PipelineEngine {
    /// Create a new engine from a freshly-submitted plan.
    pub fn new(plan: crate::pipeline::PipelinePlan, context: PipelineEngineContext) -> Self {
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
        project_dir: std::path::PathBuf,
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
        project_dir: std::path::PathBuf,
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

    pub(super) fn get_step(&self, step_id: &str) -> Option<&crate::pipeline::PlanStep> {
        self.runtime
            .plan
            .steps
            .iter()
            .find(|s| s.step_id == step_id)
    }

    pub(super) fn set_status(&mut self, step_id: &str, status: crate::pipeline::StepStatus) {
        self.runtime.step_status.insert(step_id.to_string(), status);
    }
}
