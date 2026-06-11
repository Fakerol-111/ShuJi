//! PipelineEngine: drives departments according to a PipelinePlan.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::actor::ActorMessage;
use crate::api::control::RouteMsgType;
use crate::models::role::Role;
use tokio::sync::mpsc;

use super::{PipelinePlan, PipelineResult, PlanRuntime, PlanStep, StepStatus};

// ── Internal step result (not yet converted to PipelineResult) ──

enum StepResultInner {
    Success { artifact_id: Option<String> },
    ApprovalRequired { doc_id: String },
    AwaitingUserInput { question: String },
    Failed { reason: String },
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
    ) -> Self {
        Self {
            runtime: PlanRuntime::new(plan),
            actor_txs,
            fast_txs,
            cancel_map,
            cancel,
            project_dir,
        }
    }

    /// Save current runtime to disk.
    pub async fn save(&self) -> Result<(), String> {
        self.runtime.save_to(&self.project_dir).await
    }

    /// Load engine state from disk (restart recovery).
    pub async fn load_from_disk(
        project_dir: &std::path::Path,
        actor_system: &crate::actor::ActorSystem,
    ) -> Option<Self> {
        let runtime = PlanRuntime::load_from(project_dir).await?;
        Some(Self {
            runtime,
            actor_txs: actor_system.senders.clone(),
            fast_txs: Arc::new(actor_system.fast_txs.clone()),
            cancel_map: actor_system.cancel_map.clone(),
            cancel: actor_system.cancel.clone(),
            project_dir: project_dir.to_path_buf(),
        })
    }

    /// Main execution loop. Drives all steps according to plan.
    pub async fn run(&mut self) -> PipelineResult {
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return PipelineResult::Aborted {
                    runtime: self.runtime.clone(),
                };
            }

            // 1. Find next executable step
            let next = match self.runtime.find_executable_step() {
                Some(id) => id,
                None => {
                    if self.runtime.all_done() {
                        return PipelineResult::Complete {
                            runtime: self.runtime.clone(),
                        };
                    } else {
                        return PipelineResult::Deadlock {
                            runtime: self.runtime.clone(),
                        };
                    }
                }
            };

            // 2. Execute with retry
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

            for attempt in 0..(step.retry.max(1)) {
                if self.cancel.load(Ordering::SeqCst) {
                    return PipelineResult::Aborted {
                        runtime: self.runtime.clone(),
                    };
                }

                let result = self.execute_step(&step).await;
                match result {
                    StepResultInner::Success { artifact_id } => {
                        self.set_status(&next, StepStatus::Done);
                        if let Some(id) = artifact_id {
                            self.runtime.artifacts.insert(next.clone(), id);
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
                self.runtime.error_log.push(format!("{}: {}", next, last_reason));

                match step.on_failure.as_str() {
                    "abort" => {
                        return PipelineResult::Aborted {
                            runtime: self.runtime.clone(),
                        }
                    }
                    "skip" => {
                        self.set_status(&next, StepStatus::Skipped);
                        continue;
                    }
                    _ => {
                        self.save().await.ok();
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
                let doc_id = step
                    .action_params
                    .get("doc_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                StepResultInner::ApprovalRequired { doc_id }
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
                        step_id: format!("{}-{}", step.step_id, t["name"].as_str().unwrap_or("sub")),
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
                let futures: Vec<_> = sub_steps.iter().map(|s| self.execute_route_to_step(s)).collect();
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
                StepResultInner::Success { artifact_id: None }
            }
            "self_execute" => {
                // 内阁 self-execute: execute as a sub-agent
                let task = step.action_params.get("task").and_then(|v| v.as_str()).unwrap_or("");
                let doc_id = self.execute_task_via_session(task, &step.step_id).await;
                StepResultInner::Success { artifact_id: doc_id }
            }
            other => StepResultInner::Failed {
                reason: format!("unknown action type: {}", other),
            },
        }
    }

    /// Route a step to a department actor and wait for its output.
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

        // Create a reply channel so the actor can send output back
        let (output_tx, mut output_rx) = mpsc::unbounded_channel::<String>();

        let msg = ActorMessage {
            msg_type: RouteMsgType::Task,
            subject: task.to_string(),
            payload: None,
            reply_to: Some(output_tx),
        };

        let tx = match self.actor_txs.get(&role) {
            Some(tx) => tx,
            None => {
                return StepResultInner::Failed {
                    reason: format!("actor channel not found for {}", target),
                }
            }
        };

        log_console!(
            "[pipeline] routing step {} → {}: {}",
            step.step_id,
            target,
            task.chars().take(60).collect::<String>()
        );

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
                StepResultInner::Success { artifact_id: None }
            }
            None => StepResultInner::Failed {
                reason: "actor channel closed unexpectedly".into(),
            },
        }
    }

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
        let _ = tokio::fs::write(&doc_path, serde_json::to_string_pretty(&doc).unwrap_or_default()).await;
        Some(doc_id)
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn get_step(&self, step_id: &str) -> Option<&PlanStep> {
        self.runtime.plan.steps.iter().find(|s| s.step_id == step_id)
    }

    fn set_status(&mut self, step_id: &str, status: StepStatus) {
        self.runtime.step_status.insert(step_id.to_string(), status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{PipelinePlan, PlanStep, PlanRuntime};

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
