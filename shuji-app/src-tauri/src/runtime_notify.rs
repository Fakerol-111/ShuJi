//! Unified runtime-update notifications for the frontend cockpit.

use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::pipeline::{PlanRuntime, StepStatus};
use crate::round_metrics::RoundMetricState;

/// User-facing runtime state vocabulary.
///
/// These are the primary states the app can be in. UI should show one
/// primary state at a time; developer details remain in expert panels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    /// No pipeline active, no departments running.
    Idle,
    /// 内阁 is forming a pipeline plan.
    Planning,
    /// Pipeline is dispatching to a department.
    Dispatching,
    /// A department is actively executing its task.
    RunningDepartment,
    /// Waiting for emperor to approve a review document.
    AwaitingApproval,
    /// Waiting for the user to answer a question.
    AwaitingUserInput,
    /// Delivery validation is in progress.
    ValidatingDelivery,
    /// A step is being retried after failure.
    Retrying,
    /// User-initiated cancellation in progress.
    Cancelling,
    /// Pipeline was cancelled.
    Cancelled,
    /// A step failed and needs attention.
    Failed,
    /// Pipeline is deadlocked — no executable step found.
    Deadlocked,
    /// Pipeline completed successfully.
    Complete,
}

/// Derive the primary runtime state from pipeline snapshot and active roles.
///
/// Precedence (high → low):
/// 1. AwaitingApproval / AwaitingUserInput
/// 2. Failed (terminal, no roles)
/// 3. Deadlocked (terminal, no roles)
/// 4. Cancelled (terminal, no roles)
/// 5. Complete (terminal, no roles)
/// 6. RunningDepartment (active roles)
/// 7. Planning / Idle (no pipeline)
pub fn derive_runtime_state(
    pipeline: Option<&PipelineSnapshot>,
    active_roles: &[String],
) -> RuntimeState {
    let Some(snap) = pipeline else {
        if active_roles.is_empty() {
            return RuntimeState::Idle;
        }
        return RuntimeState::Planning;
    };

    // Pending approval/user input wins over everything
    if snap.awaiting_approval {
        return RuntimeState::AwaitingApproval;
    }
    if snap.awaiting_user_input {
        return RuntimeState::AwaitingUserInput;
    }

    if active_roles.is_empty() {
        // Terminal states — check specific flags before falling through
        if snap.failed {
            return RuntimeState::Failed;
        }
        if snap.deadlocked {
            return RuntimeState::Deadlocked;
        }
        if snap.cancelled {
            return RuntimeState::Cancelled;
        }
        if snap.steps_done == snap.steps_total && snap.steps_total > 0 {
            return RuntimeState::Complete;
        }
        return RuntimeState::Failed; // pipeline exists but no roles — likely a failed state
    }

    // Active roles present
    RuntimeState::RunningDepartment
}

/// Lightweight pipeline progress for instant UI refresh.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PipelineSnapshot {
    pub current_step: Option<String>,
    pub current_step_label: Option<String>,
    pub steps_done: u32,
    pub steps_total: u32,
    pub awaiting_approval: bool,
    pub awaiting_user_input: bool,
    /// Pipeline has a step with status Failed.
    pub failed: bool,
    /// Pipeline is deadlocked (no executable step, not all done).
    pub deadlocked: bool,
    /// Pipeline was explicitly cancelled/aborted.
    pub cancelled: bool,
    pub plan_summary: String,
}

/// Snapshot pushed to the frontend on each runtime change.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeUpdate {
    pub active_roles: Vec<String>,
    pub round_metrics: Option<RoundMetricState>,
    pub pipeline: Option<PipelineSnapshot>,
    pub runtime_state: RuntimeState,
    pub trigger: String,
    /// Unix timestamp (millis) of the last recorded activity.
    pub last_activity_at: Option<i64>,
    /// Milliseconds since the round started, if a round is active.
    pub elapsed_ms: Option<i64>,
    /// Name of the last tool called, if any.
    pub last_tool: Option<String>,
    /// Path of the last successful write operation, if any.
    pub last_write: Option<String>,
}

static SENDER: Mutex<Option<mpsc::UnboundedSender<RuntimeUpdate>>> = Mutex::new(None);
static PIPELINE: Mutex<Option<PipelineSnapshot>> = Mutex::new(None);

/// Shared heartbeat state updated by tool dispatch.
struct HeartbeatState {
    last_activity_at: i64,
    last_tool: Option<String>,
    last_write: Option<PathBuf>,
}

static HEARTBEAT: Mutex<Option<HeartbeatState>> = Mutex::new(None);

/// Record a tool call for heartbeat tracking.
pub fn record_tool_call(tool_name: &str) {
    if let Ok(mut guard) = HEARTBEAT.lock() {
        let state = guard.get_or_insert_with(|| HeartbeatState {
            last_activity_at: Utc::now().timestamp_millis(),
            last_tool: None,
            last_write: None,
        });
        state.last_activity_at = Utc::now().timestamp_millis();
        state.last_tool = Some(tool_name.to_string());
    }
}

/// Record a successful write operation for heartbeat tracking.
pub fn record_write(path: &std::path::Path) {
    if let Ok(mut guard) = HEARTBEAT.lock() {
        let state = guard.get_or_insert_with(|| HeartbeatState {
            last_activity_at: Utc::now().timestamp_millis(),
            last_tool: None,
            last_write: None,
        });
        state.last_activity_at = Utc::now().timestamp_millis();
        state.last_write = Some(path.to_path_buf());
    }
}

pub fn set_sender(tx: mpsc::UnboundedSender<RuntimeUpdate>) {
    if let Ok(mut guard) = SENDER.lock() {
        *guard = Some(tx);
    }
}

pub fn pipeline_snapshot_from(runtime: &PlanRuntime) -> PipelineSnapshot {
    let steps_total = runtime.plan.steps.len() as u32;
    let steps_done = runtime
        .plan
        .steps
        .iter()
        .filter(|step| {
            matches!(
                runtime.step_status.get(&step.step_id),
                Some(StepStatus::Done) | Some(StepStatus::Skipped)
            )
        })
        .count() as u32;

    let current_step = runtime.current_step.clone();
    let current_step_label = current_step.as_ref().and_then(|id| {
        runtime
            .plan
            .steps
            .iter()
            .find(|s| &s.step_id == id)
            .map(|s| s.description.clone())
    });

    let awaiting_approval = runtime.plan.steps.iter().any(|step| {
        step.action == "approval_gate"
            && matches!(
                runtime.step_status.get(&step.step_id),
                Some(StepStatus::InProgress) | Some(StepStatus::Pending)
            )
    });

    let awaiting_user_input = runtime.plan.steps.iter().any(|step| {
        step.action == "ask_user"
            && matches!(
                runtime.step_status.get(&step.step_id),
                Some(StepStatus::InProgress) | Some(StepStatus::Pending)
            )
    });

    // Terminal state detection from runtime state
    let failed = runtime
        .step_status
        .values()
        .any(|s| *s == StepStatus::Failed);
    let deadlocked = runtime.find_executable_step().is_none() && !runtime.all_done();
    let cancelled = runtime
        .error_log
        .iter()
        .any(|e| e.contains("abort") || e.contains("cancel"))
        || (!failed
            && !deadlocked
            && steps_done < steps_total
            && !awaiting_approval
            && !awaiting_user_input);

    PipelineSnapshot {
        current_step,
        current_step_label,
        steps_done,
        steps_total,
        awaiting_approval,
        awaiting_user_input,
        failed,
        deadlocked,
        cancelled,
        plan_summary: runtime.plan.summary.clone(),
    }
}

pub fn update_pipeline(runtime: &PlanRuntime) {
    let snapshot = pipeline_snapshot_from(runtime);
    if let Ok(mut guard) = PIPELINE.lock() {
        *guard = Some(snapshot);
    }
    notify("pipeline");
}

pub fn clear_pipeline() {
    if let Ok(mut guard) = PIPELINE.lock() {
        *guard = None;
    }
    notify("pipeline_cleared");
}

/// Build and enqueue a runtime snapshot.
pub fn notify(trigger: &str) {
    let pipeline = PIPELINE.lock().ok().and_then(|g| g.clone());
    let active_roles = crate::round_metrics::get_active_roles();

    let (elapsed_ms, last_tool, last_write, last_activity_at) = HEARTBEAT
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref().map(|h| {
                let elapsed = h
                    .last_activity_at
                    .checked_sub(crate::round_metrics::round_started_at().unwrap_or(0))
                    .or(Some(0));
                (
                    elapsed,
                    h.last_tool.clone(),
                    h.last_write
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string()),
                    Some(h.last_activity_at),
                )
            })
        })
        .unwrap_or((Some(0), None, None, None));

    let runtime_state = derive_runtime_state(pipeline.as_ref(), &active_roles);
    let update = RuntimeUpdate {
        active_roles,
        round_metrics: crate::round_metrics::snapshot(),
        pipeline,
        runtime_state,
        trigger: trigger.to_string(),
        last_activity_at,
        elapsed_ms,
        last_tool,
        last_write,
    };
    if let Ok(guard) = SENDER.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(update);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{PipelinePlan, PlanStep};

    fn sample_runtime() -> PlanRuntime {
        let plan = PipelinePlan {
            plan_id: "p1".into(),
            summary: "Test plan".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![
                PlanStep {
                    step_id: "s1".into(),
                    description: "Design".into(),
                    action: "dispatch_to".into(),
                    action_params: serde_json::json!({}),
                    depends_on: vec![],
                    require_approval: false,
                    on_failure: "wake_cabinet".into(),
                    retry: 1,
                },
                PlanStep {
                    step_id: "s2".into(),
                    description: "Approve".into(),
                    action: "approval_gate".into(),
                    action_params: serde_json::json!({}),
                    depends_on: vec!["s1".into()],
                    require_approval: true,
                    on_failure: "wake_cabinet".into(),
                    retry: 1,
                },
            ],
        };
        let mut rt = PlanRuntime::new(plan);
        rt.step_status.insert("s1".into(), StepStatus::Done);
        rt.step_status.insert("s2".into(), StepStatus::InProgress);
        rt.current_step = Some("s2".into());
        rt
    }

    #[test]
    fn pipeline_snapshot_counts_done_steps() {
        let snap = pipeline_snapshot_from(&sample_runtime());
        assert_eq!(snap.steps_done, 1);
        assert_eq!(snap.steps_total, 2);
        assert_eq!(snap.current_step.as_deref(), Some("s2"));
        assert!(snap.awaiting_approval);
    }

    #[test]
    fn runtime_update_serializes_pipeline() {
        let update = RuntimeUpdate {
            active_roles: vec!["Neige".into()],
            round_metrics: None,
            pipeline: Some(pipeline_snapshot_from(&sample_runtime())),
            runtime_state: RuntimeState::RunningDepartment,
            trigger: "test".into(),
            last_activity_at: None,
            elapsed_ms: Some(0),
            last_tool: None,
            last_write: None,
        };
        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("pipeline"));
        assert!(json.contains("steps_done"));
        assert!(json.contains("running_department"));
        assert!(json.contains("last_activity_at"));
        assert!(json.contains("last_tool"));
    }

    // ── derive_runtime_state tests ──────────────────────────────────

    #[test]
    fn no_pipeline_no_roles_is_idle() {
        assert_eq!(derive_runtime_state(None, &[]), RuntimeState::Idle);
    }

    #[test]
    fn no_pipeline_with_roles_is_planning() {
        assert_eq!(
            derive_runtime_state(None, &["Neige".into()]),
            RuntimeState::Planning
        );
    }

    #[test]
    fn awaiting_approval_wins() {
        let mut snap = pipeline_snapshot_from(&sample_runtime());
        snap.awaiting_approval = true;
        snap.awaiting_user_input = false;
        snap.failed = false;
        snap.deadlocked = false;
        assert_eq!(
            derive_runtime_state(Some(&snap), &[]),
            RuntimeState::AwaitingApproval
        );
    }

    #[test]
    fn failed_step_derives_failed() {
        let mut rt = sample_runtime();
        rt.step_status.insert("s2".into(), StepStatus::Failed);
        rt.current_step = None;
        let snap = pipeline_snapshot_from(&rt);
        assert!(snap.failed);
        assert_eq!(derive_runtime_state(Some(&snap), &[]), RuntimeState::Failed);
    }

    #[test]
    fn deadlock_derives_deadlocked() {
        // Two steps: s2 depends on s1, s1 is Pending (blocked by itself shouldn't)
        // Make s1 Done but s2 depend on non-existent step to create deadlock
        let plan = PipelinePlan {
            plan_id: "p1".into(),
            summary: "Test".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![PlanStep {
                step_id: "s1".into(),
                description: "Step1".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({}),
                depends_on: vec!["s_missing".into()], // depends on non-existent
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            }],
        };
        let rt = PlanRuntime::new(plan);
        let snap = pipeline_snapshot_from(&rt);
        // No failed steps, but s1 can never execute (depends on missing step)
        assert!(!snap.failed);
        assert!(snap.deadlocked);
        assert_eq!(
            derive_runtime_state(Some(&snap), &[]),
            RuntimeState::Deadlocked
        );
    }

    #[test]
    fn complete_derives_complete() {
        let mut rt = sample_runtime();
        rt.step_status.insert("s1".into(), StepStatus::Done);
        rt.step_status.insert("s2".into(), StepStatus::Done);
        rt.current_step = None;
        let snap = pipeline_snapshot_from(&rt);
        assert_eq!(
            derive_runtime_state(Some(&snap), &[]),
            RuntimeState::Complete
        );
    }

    #[test]
    fn cancelled_derives_cancelled() {
        // Pipeline with work remaining and abort in error_log, but step in Pending
        // (not deadlocked — executable) — cancelled flag should be set
        let plan = PipelinePlan {
            plan_id: "p1".into(),
            summary: "Test".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![PlanStep {
                step_id: "s1".into(),
                description: "Step".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            }],
        };
        let mut rt = PlanRuntime::new(plan);
        // Keep s1 as Pending so it's still executable (not deadlocked)
        rt.current_step = Some("s1".into());
        rt.error_log.push("Pipeline aborted by user".into());
        let snap = pipeline_snapshot_from(&rt);
        assert!(snap.cancelled);
        assert_eq!(
            derive_runtime_state(Some(&snap), &[]),
            RuntimeState::Cancelled
        );
    }

    #[test]
    fn running_department_with_active_roles() {
        // Use the non-approval runtime to avoid AwaitingApproval winning
        let plan = PipelinePlan {
            plan_id: "p1".into(),
            summary: "Test".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![PlanStep {
                step_id: "s1".into(),
                description: "Step".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            }],
        };
        let mut rt = PlanRuntime::new(plan);
        rt.step_status.insert("s1".into(), StepStatus::InProgress);
        rt.current_step = Some("s1".into());
        let snap = pipeline_snapshot_from(&rt);
        assert_eq!(
            derive_runtime_state(Some(&snap), &["Zhongshuling".into()]),
            RuntimeState::RunningDepartment
        );
    }
}
