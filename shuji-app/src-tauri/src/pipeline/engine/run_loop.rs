//! run — main execution loop driving all pipeline steps according to plan.

use std::sync::atomic::Ordering;

use super::super::{PipelineResult, StepStatus};
use super::result::StepResultInner;
use super::PipelineEngine;

impl PipelineEngine {
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
                        let run_id = self.runtime.plan.plan_id.clone();
                        let step_id = next.clone();
                        crate::storage::checkpoint::save_semantic(
                            &self.project_dir,
                            "pipeline",
                            &format!("朱批前: {}", doc_id),
                            crate::storage::checkpoint::CheckpointKind::BeforeApproval,
                            crate::storage::checkpoint::CheckpointMeta {
                                run_id: Some(run_id.clone()),
                                step_id: Some(step_id.clone()),
                                doc_id: Some(doc_id.clone()),
                                reason: Some("approval_gate".into()),
                                ..Default::default()
                            },
                            None,
                        )
                        .await;
                        crate::audit::append_line_event(
                            &self.project_dir,
                            &run_id,
                            "approval_gate",
                            &doc_id,
                            serde_json::json!({"step_id": step_id, "status": "awaiting"}),
                        )
                        .await;
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
}
