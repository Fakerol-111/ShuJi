//! execute_step — action dispatcher for pipeline steps.

use super::super::artifacts::approval_doc_from_upstream;
use super::super::PlanStep;
use super::result::StepResultInner;
use super::PipelineEngine;

impl PipelineEngine {
    /// Execute a single step according to its action type.
    pub(crate) async fn execute_step(&self, step: &PlanStep) -> StepResultInner {
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
                    Some(id) => {
                        // Reject empty revw documents — they should never be presented
                        // to the user for approval. This prevents the bug where an empty
                        // revw (created but never appended to) reaches the approval gate.
                        if id.starts_with("revw_") {
                            let body =
                                crate::tool::documents::get_document_body(&self.project_dir, &id)
                                    .await;
                            match body {
                                Some(b) if !b.trim().is_empty() => {}
                                _ => {
                                    return StepResultInner::Failed {
                                        reason: format!(
                                            "approval_gate requires a non-empty revw document body, but {} is empty or missing. This indicates the reviewer did not write content before the pipeline reached this step.",
                                            id
                                        ),
                                    };
                                }
                            }
                        }
                        StepResultInner::ApprovalRequired { doc_id: id }
                    }
                    None => StepResultInner::Failed {
                        reason:
                            "approval_gate requires an upstream revw document, but none was found."
                                .into(),
                    },
                }
            }
            "dispatch_to" => self.execute_dispatch_step(step).await,
            "parallel" => {
                // Execute multiple dispatch steps concurrently
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
                        action: "dispatch_to".into(),
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
                    .map(|s| self.execute_dispatch_step(s))
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
}
