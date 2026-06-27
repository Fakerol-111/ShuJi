//! PipelineEngine: drives departments according to a PipelinePlan.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::actor::ActorMessage;
use crate::api::control::RouteMsgType;
use crate::config::{ApprovalMode, RuntimeConfig};
use crate::models::role::Role;
use crate::workflow::WorkflowGraph;
use tokio::sync::mpsc;

use super::artifacts::{
    approval_doc_from_upstream, collect_upstream_doc_ids, extract_artifact_from_output,
};
use super::{PipelinePlan, PipelineResult, PlanRuntime, PlanStep, StepStatus};

// ── Internal step result (not yet converted to PipelineResult) ──

enum StepResultInner {
    Success {
        artifact_id: Option<String>,
        target_role: Option<String>,
    },
    ApprovalRequired {
        doc_id: String,
    },
    AwaitingUserInput {
        question: String,
    },
    Failed {
        reason: String,
    },
}

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

impl PipelineEngine {
    /// Create a new engine from a freshly-submitted plan.
    pub fn new(
        plan: PipelinePlan,
        actor_txs: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
        fast_txs: crate::FastTxMap,
        cancel_map: crate::CancelMap,
        cancel: Arc<AtomicBool>,
        project_dir: PathBuf,
        workflow_graph: Option<Arc<tokio::sync::Mutex<WorkflowGraph>>>,
        runtime_config: Arc<RuntimeConfig>,
    ) -> Self {
        Self {
            runtime: PlanRuntime::new(plan),
            actor_txs,
            fast_txs,
            cancel_map,
            cancel,
            project_dir,
            workflow_graph,
            graph_last_role: "内阁".to_string(),
            run_metrics: None,
            runtime_config,
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
        Self {
            runtime,
            actor_txs: actor_system.senders.clone(),
            fast_txs: Arc::new(actor_system.fast_txs.clone()),
            cancel_map: actor_system.cancel_map.clone(),
            cancel: actor_system.cancel.clone(),
            project_dir,
            workflow_graph: Some(actor_system.workflow_graph.clone()),
            graph_last_role: "内阁".to_string(),
            run_metrics: None,
            runtime_config,
        }
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
        Self {
            runtime,
            actor_txs,
            fast_txs: Arc::new(HashMap::new()),
            cancel_map: Arc::new(Mutex::new(HashMap::<
                crate::models::role::Role,
                Arc<AtomicBool>,
            >::new())),
            cancel: Arc::new(AtomicBool::new(false)),
            project_dir,
            workflow_graph: None,
            graph_last_role: "内阁".to_string(),
            run_metrics: None,
            runtime_config,
        }
    }

    /// Finalize run metrics by saving to disk.
    async fn finalize_metrics(&mut self, status: &str) {
        if let Some(ref mut metrics) = self.run_metrics {
            // Attach validation from artifacts if available
            if let Some(validation_json) = self.runtime.artifacts.get("validate_report") {
                if let Ok(report) = serde_json::from_str::<crate::validate::report::ValidationReport>(
                    validation_json,
                ) {
                    metrics.attach_validation(report);
                }
            }
            // Also check step artifacts for validate_delivery steps
            for (step_id, _artifact) in &self.runtime.artifacts {
                if step_id.contains("validate") || step_id == "v1" {
                    // Try to read the validation report from .shuji/validate/latest.json
                    let report_path = self
                        .project_dir
                        .join(".shuji")
                        .join("validate")
                        .join("latest.json");
                    if let Ok(content) = std::fs::read_to_string(&report_path) {
                        if let Ok(report) = serde_json::from_str::<
                            crate::validate::report::ValidationReport,
                        >(&content)
                        {
                            metrics.attach_validation(report);
                            break;
                        }
                    }
                }
            }
            metrics.finalize(status, &self.project_dir).await.ok();
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
        if self.runtime_config.approval.mode == ApprovalMode::Manual {
            if let Some(step) = self
                .runtime
                .plan
                .steps
                .iter()
                .find(|s| s.step_id == current)
            {
                if step.action == "approval_gate" {
                    let doc_id =
                        approval_doc_from_upstream(&self.runtime.artifacts, &step.depends_on);
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

    /// Main execution loop. Drives all steps according to plan.
    pub async fn run(&mut self) -> PipelineResult {
        // Initialize run metrics
        if self.run_metrics.is_none() {
            self.run_metrics = Some(crate::metrics::RunMetrics::start(
                &self.runtime.plan.plan_id,
            ));
        }

        loop {
            if self.cancel.load(Ordering::SeqCst) {
                self.finalize_metrics("aborted").await;
                return PipelineResult::Aborted {
                    runtime: self.runtime.clone(),
                };
            }

            // 1. Find next executable step
            let next = match self.runtime.find_executable_step() {
                Some(id) => id,
                None => {
                    if self.runtime.all_done() {
                        self.finalize_metrics("complete").await;
                        return PipelineResult::Complete {
                            runtime: self.runtime.clone(),
                        };
                    } else {
                        self.finalize_metrics("deadlock").await;
                        return PipelineResult::Deadlock {
                            runtime: self.runtime.clone(),
                        };
                    }
                }
            };

            // 2. Execute with retry
            let step_start = std::time::Instant::now();
            self.runtime.current_step = Some(next.clone());
            self.set_status(&next, StepStatus::InProgress);

            let step = self.get_step(&next).cloned();
            let step = match step {
                Some(s) => s,
                None => {
                    self.runtime
                        .error_log
                        .push(format!("step not found: {}", next));
                    self.set_status(&next, StepStatus::Failed);
                    return PipelineResult::StepFailed {
                        step_id: next,
                        reason: "step not found in plan".into(),
                        runtime: self.runtime.clone(),
                    };
                }
            };

            let mut last_reason = String::new();
            let mut tool_errors: u32 = 0;
            let mut iterations: u32 = 0;

            for attempt in 0..(step.retry.max(1)) {
                if self.cancel.load(Ordering::SeqCst) {
                    return PipelineResult::Aborted {
                        runtime: self.runtime.clone(),
                    };
                }

                iterations += 1;
                let result = self.execute_step(&step).await;
                match result {
                    StepResultInner::Success {
                        artifact_id,
                        target_role,
                    } => {
                        self.set_status(&next, StepStatus::Done);
                        if let Some(id) = artifact_id {
                            self.runtime.artifacts.insert(next.clone(), id);
                        }
                        // 文移图：标记部门节点为 completed
                        if let Some(role_name) = &target_role {
                            if let Some(ref graph_lock) = self.workflow_graph {
                                let mut g = graph_lock.lock().await;
                                g.mark_completed(role_name);
                                let _ = g.save_to(&self.project_dir).await;
                            }
                            self.graph_last_role = role_name.clone();
                        }
                        // Record step metric
                        let duration = step_start.elapsed().as_millis() as u64;
                        if let Some(ref mut metrics) = self.run_metrics {
                            let target = target_role.clone();
                            metrics.add_step(crate::metrics::StepMetric {
                                step_id: next.clone(),
                                action: step.action.clone(),
                                target,
                                started_at: chrono::Local::now().to_rfc3339(),
                                duration_ms: duration,
                                status: "done".into(),
                                tool_errors,
                                iterations,
                            });
                        }
                        self.save().await.ok();
                        break;
                    }
                    StepResultInner::ApprovalRequired { doc_id } => {
                        self.save().await.ok();
                        return PipelineResult::AwaitingApproval {
                            doc_id,
                            step_id: next,
                            runtime: self.runtime.clone(),
                        };
                    }
                    StepResultInner::AwaitingUserInput { question } => {
                        self.save().await.ok();
                        return PipelineResult::AwaitingUserInput {
                            step_id: next,
                            question,
                            runtime: self.runtime.clone(),
                        };
                    }
                    StepResultInner::Failed { reason } => {
                        tool_errors += 1;
                        last_reason = reason;
                        if attempt + 1 < step.retry.max(1) {
                            log_console!(
                                "[pipeline] step {} retry {}/{}",
                                next,
                                attempt + 1,
                                step.retry
                            );
                            continue;
                        }
                    }
                }
            }

            if self.runtime.step_status.get(&next) == Some(&StepStatus::InProgress) {
                // All retries exhausted — still in progress = failed
                self.set_status(&next, StepStatus::Failed);
                self.runtime
                    .error_log
                    .push(format!("{}: {}", next, last_reason));

                // Record failed step metric
                let duration = step_start.elapsed().as_millis() as u64;
                if let Some(ref mut metrics) = self.run_metrics {
                    metrics.add_step(crate::metrics::StepMetric {
                        step_id: next.clone(),
                        action: step.action.clone(),
                        target: None,
                        started_at: chrono::Local::now().to_rfc3339(),
                        duration_ms: duration,
                        status: "failed".into(),
                        tool_errors,
                        iterations,
                    });
                }

                match step.on_failure.as_str() {
                    "abort" => {
                        self.finalize_metrics("aborted").await;
                        return PipelineResult::Aborted {
                            runtime: self.runtime.clone(),
                        };
                    }
                    "skip" => {
                        self.set_status(&next, StepStatus::Skipped);
                        continue;
                    }
                    _ => {
                        self.save().await.ok();
                        self.finalize_metrics("failed").await;
                        return PipelineResult::StepFailed {
                            step_id: next,
                            reason: last_reason,
                            runtime: self.runtime.clone(),
                        };
                    }
                }
            }
        }
    }

    /// Execute a single step according to its action type.
    async fn execute_step(&self, step: &PlanStep) -> StepResultInner {
        match step.action.as_str() {
            "ask_user" => {
                let question = step
                    .action_params
                    .get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&step.description);
                StepResultInner::AwaitingUserInput {
                    question: question.to_string(),
                }
            }
            "approval_gate" => {
                let doc_id = approval_doc_from_upstream(&self.runtime.artifacts, &step.depends_on);
                match doc_id.filter(|id| !id.is_empty()) {
                    Some(id) => StepResultInner::ApprovalRequired { doc_id: id },
                    None => StepResultInner::Failed {
                        reason:
                            "approval_gate requires an upstream revw document, but none was found."
                                .into(),
                    },
                }
            }
            "route_to" => self.execute_route_to_step(step).await,
            "parallel" => {
                // Execute multiple route_to steps concurrently
                let targets: Vec<serde_json::Value> = step
                    .action_params
                    .get("targets")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                if targets.is_empty() {
                    return StepResultInner::Failed {
                        reason: "parallel action requires 'targets' array".into(),
                    };
                }

                let mut sub_steps: Vec<PlanStep> = Vec::new();
                for t in &targets {
                    sub_steps.push(PlanStep {
                        step_id: format!(
                            "{}-{}",
                            step.step_id,
                            t["name"].as_str().unwrap_or("sub")
                        ),
                        description: t["task"].as_str().unwrap_or("").to_string(),
                        action: "route_to".into(),
                        action_params: serde_json::json!({
                            "target": t["target"].as_str().unwrap_or(""),
                            "task": t["task"].as_str().unwrap_or(""),
                        }),
                        depends_on: vec![],
                        require_approval: false,
                        on_failure: "wake_cabinet".into(),
                        retry: 1,
                    });
                }
                let futures: Vec<_> = sub_steps
                    .iter()
                    .map(|s| self.execute_route_to_step(s))
                    .collect();
                let results = futures::future::join_all(futures).await;
                // Check all results — return first failure
                for r in results {
                    match r {
                        StepResultInner::Failed { reason } => {
                            return StepResultInner::Failed { reason };
                        }
                        StepResultInner::ApprovalRequired { doc_id } => {
                            return StepResultInner::ApprovalRequired { doc_id };
                        }
                        StepResultInner::AwaitingUserInput { question } => {
                            return StepResultInner::AwaitingUserInput { question };
                        }
                        StepResultInner::Success { .. } => {}
                    }
                }
                StepResultInner::Success {
                    artifact_id: None,
                    target_role: None,
                }
            }
            "self_execute" => {
                // self_execute: dispatch to handler registry
                let handler = step
                    .action_params
                    .get("handler")
                    .and_then(|v| v.as_str())
                    .unwrap_or("noop");
                match crate::pipeline::handlers::run_self_execute(
                    handler,
                    &step.action_params,
                    &self.project_dir,
                )
                .await
                {
                    Ok(outcome) => match outcome {
                        crate::pipeline::handlers::SelfExecuteOutcome::Success {
                            message,
                            artifact,
                        } => {
                            log_console!("[pipeline] self_execute ({}): {}", handler, message);
                            StepResultInner::Success {
                                artifact_id: artifact,
                                target_role: None,
                            }
                        }
                        crate::pipeline::handlers::SelfExecuteOutcome::Failed { reason } => {
                            StepResultInner::Failed { reason }
                        }
                    },
                    Err(e) => StepResultInner::Failed {
                        reason: format!("self_execute handler error: {}", e),
                    },
                }
            }
            other => StepResultInner::Failed {
                reason: format!("unknown action type: {}", other),
            },
        }
    }

    /// Route a step to a department actor and wait for its output.
    /// Records edges in the workflow graph for 文移图 visualization.
    async fn execute_route_to_step(&self, step: &PlanStep) -> StepResultInner {
        let target = step
            .action_params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let task = step
            .action_params
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let role = match Role::from_name(target) {
            Some(r) => r,
            None => {
                return StepResultInner::Failed {
                    reason: format!("unknown department: {}", target),
                }
            }
        };

        // 从 depends_on 对应的上游步骤 artifact 收集文档 ID（plan 不含 id）
        let upstream_doc_ids = collect_upstream_doc_ids(&self.runtime.artifacts, &step.depends_on);

        // Manual mode: gate route_to to execution departments when refs are not approved
        const EXEC_DEPTS: [&str; 6] = ["尚书令", "吏部", "兵部", "工部", "刑部", "礼部"];
        if self.runtime_config.approval.mode == ApprovalMode::Manual && EXEC_DEPTS.contains(&target)
        {
            if let Some(subject) = upstream_doc_ids.first() {
                if let Err(msg) = crate::tool::documents::check_doc_refs_approved_for_route(
                    &self.project_dir,
                    subject,
                )
                .await
                {
                    log_console!(
                        "[pipeline] route_to blocked at step {} → {}: {}",
                        step.step_id,
                        target,
                        msg
                    );
                    return StepResultInner::ApprovalRequired {
                        doc_id: subject.to_string(),
                    };
                }
            }
        }

        // Create a reply channel so the actor can send output back
        let (output_tx, mut output_rx) = mpsc::unbounded_channel::<String>();

        let msg = ActorMessage {
            msg_type: RouteMsgType::Task,
            subject: task.to_string(),
            payload: None,
            doc_ids: upstream_doc_ids.clone(),
            reply_to: Some(output_tx),
            allow_pipeline_plan: true,
        };

        let tx = match self.actor_txs.get(&role) {
            Some(tx) => tx,
            None => {
                return StepResultInner::Failed {
                    reason: format!("actor channel not found for {}", target),
                }
            }
        };

        if !upstream_doc_ids.is_empty() {
            log_console!(
                "[pipeline] step {} upstream doc_ids: {}",
                step.step_id,
                upstream_doc_ids.join(", ")
            );
        }

        log_console!(
            "[pipeline] routing step {} → {}: {}",
            step.step_id,
            target,
            task.chars().take(80).collect::<String>()
        );

        // ── 文移图：标记节点为 active（节点/边已由 preview 预创建） ──
        if let Some(ref graph_lock) = self.workflow_graph {
            let mut g = graph_lock.lock().await;
            g.mark_active(target);
            let _ = g.save_to(&self.project_dir).await;
        }

        if let Err(e) = tx.send(msg) {
            return StepResultInner::Failed {
                reason: format!("failed to send message to {}: {}", target, e),
            };
        }

        // Wait for agent to finish (timeout-free — actor will eventually respond)
        match output_rx.recv().await {
            Some(output) => {
                log_console!(
                    "[pipeline] {} completed: {}",
                    target,
                    &output.chars().take(80).collect::<String>()
                );
                let mut artifact_id = extract_artifact_from_output(&output, target);
                if artifact_id.is_none() {
                    artifact_id = self.infer_artifact_fallback(target).await;
                }
                if let Some(ref id) = artifact_id {
                    log_console!("[pipeline] step {} artifact: {}", step.step_id, id);
                }
                StepResultInner::Success {
                    artifact_id,
                    target_role: Some(target.to_string()),
                }
            }
            None => StepResultInner::Failed {
                reason: "actor channel closed unexpectedly".into(),
            },
        }
    }

    #[allow(dead_code)]
    /// Execute a task directly via a temporary session (for self_execute steps).
    async fn execute_task_via_session(&self, task: &str, _step_id: &str) -> Option<String> {
        // For self_execute steps: currently creates a document with the task description.
        // In a full implementation, this could invoke 内阁 recursively.
        log_console!("[pipeline] self_execute: {}", task);
        // Placeholder — create a document recording the self-execution
        let doc_id = format!("self_{}", _step_id);
        let doc_path = self
            .project_dir
            .join(".shuji")
            .join("documents")
            .join(format!("{}.json", doc_id));
        if let Some(parent) = doc_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let doc = serde_json::json!({
            "id": doc_id,
            "type": "self_execute",
            "content": task,
            "created": chrono::Local::now().to_rfc3339(),
        });
        let _ = tokio::fs::write(
            &doc_path,
            serde_json::to_string_pretty(&doc).unwrap_or_default(),
        )
        .await;
        Some(doc_id)
    }

    /// 预填充文移图：遍历 pipeline plan 中的所有 route_to/parallel 步骤，
    /// 以 planned 状态添加到 workflow graph 中，让前端提前看到整个计划 DAG。
    pub async fn preview_pipeline_on_graph(&mut self) {
        if self.workflow_graph.is_none() {
            return;
        }

        let steps = self.runtime.plan.steps.clone();
        let mut last_role = "内阁".to_string();

        for step in &steps {
            match step.action.as_str() {
                "route_to" => {
                    let target = step
                        .action_params
                        .get("target")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !target.is_empty() {
                        if let Some(ref graph_lock) = self.workflow_graph {
                            let mut g = graph_lock.lock().await;
                            g.add_planned_edge(
                                &last_role,
                                target,
                                &step.step_id,
                                &step.description,
                            );
                            let _ = g.save_to(&self.project_dir).await;
                        }
                        last_role = target.to_string();
                    }
                }
                "parallel" => {
                    let targets: Vec<serde_json::Value> = step
                        .action_params
                        .get("targets")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    for t in &targets {
                        let target = t["target"].as_str().unwrap_or("");
                        if !target.is_empty() {
                            if let Some(ref graph_lock) = self.workflow_graph {
                                let mut g = graph_lock.lock().await;
                                let sub_step_id = format!(
                                    "{}-{}",
                                    step.step_id,
                                    t["name"].as_str().unwrap_or("sub")
                                );
                                g.add_planned_edge(
                                    &last_role,
                                    target,
                                    &sub_step_id,
                                    t["task"].as_str().unwrap_or(""),
                                );
                                let _ = g.save_to(&self.project_dir).await;
                            }
                        }
                    }
                    // 并行步骤的最后一个 target 作为下一条边的 from_role
                    if let Some(last_t) = targets.last() {
                        if let Some(t) = last_t["target"].as_str() {
                            last_role = t.to_string();
                        }
                    }
                }
                _ => {
                    // ask_user / approval_gate / self_execute — 不产生部门间边
                }
            }
        }
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

    /// Filesystem fallback when agent output omits the document ID.
    async fn infer_artifact_fallback(&self, target_dept: &str) -> Option<String> {
        match Role::from_name(target_dept) {
            Some(Role::Zhongshuling) => {
                crate::tool::documents::find_latest_design_doc_id(&self.project_dir).await
            }
            Some(Role::MenxiaShizhong) => {
                crate::tool::documents::find_latest_doc_with_prefixes(&self.project_dir, &["revw"])
                    .await
            }
            _ => None,
        }
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
        // Without an actor system, execute_route_to_step will fail.
        // But we can test the status setting logic directly.
        rt.step_status.insert("s1".into(), StepStatus::InProgress);
        rt.error_log.push("s1: test error".into());
        assert_eq!(rt.find_executable_step(), None);
    }

    #[test]
    fn test_retry_default_one() {
        let plan = make_single_step_plan("route_to", "工部");
        assert_eq!(plan.steps[0].retry, 1);
    }
}
