//! execute_route_to_step — route pipeline steps to department actors.

use crate::actor::ActorMessage;
use crate::api::control::RouteMsgType;
use crate::config::ApprovalMode;
use crate::models::role::Role;
use tokio::sync::mpsc;

use super::super::artifacts::{collect_upstream_doc_ids, extract_artifact_from_output};
use super::super::PlanStep;
use super::result::StepResultInner;
use super::PipelineEngine;

/// Execution departments that require pre-route checkpoints.
const EXEC_DEPTS: [&str; 6] = ["尚书令", "吏部", "兵部", "工部", "刑部", "礼部"];

impl PipelineEngine {
    /// Route a step to a department actor and wait for its output.
    /// Records edges in the workflow graph for 文移图 visualization.
    pub(crate) async fn execute_route_to_step(&self, step: &PlanStep) -> StepResultInner {
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

        if EXEC_DEPTS.contains(&target) {
            let run_id = self.runtime.plan.plan_id.clone();
            crate::storage::checkpoint::save_semantic(
                &self.project_dir,
                target,
                &format!("执行前: {}", step.description),
                crate::storage::checkpoint::CheckpointKind::BeforeExecution,
                crate::storage::checkpoint::CheckpointMeta {
                    run_id: Some(run_id.clone()),
                    step_id: Some(step.step_id.clone()),
                    doc_id: upstream_doc_ids.first().cloned(),
                    reason: Some(format!("route_to:{}", target)),
                    ..Default::default()
                },
                None,
            )
            .await;
            crate::audit::append_line_event(
                &self.project_dir,
                &run_id,
                "before_execution",
                &step.step_id,
                serde_json::json!({"target": target, "doc_ids": upstream_doc_ids}),
            )
            .await;
        }

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

    /// Filesystem fallback when agent output omits the document ID.
    async fn infer_artifact_fallback(&self, target_dept: &str) -> Option<String> {
        match Role::from_name(target_dept) {
            Some(Role::Zhongshuling) => {
                crate::tool::documents::find_latest_design_doc_id(&self.project_dir).await
            }
            Some(Role::MenxiaShizhong) => {
                // Only pick up non-empty revw docs — skip empty shells created but never written to
                crate::tool::documents::find_latest_non_empty_doc_with_prefixes(
                    &self.project_dir,
                    &["revw"],
                )
                .await
            }
            _ => None,
        }
    }
}
