//! Unified runtime-update notifications for the frontend cockpit.

use std::sync::Mutex;

use serde::Serialize;
use tokio::sync::mpsc;

use crate::pipeline::{PlanRuntime, StepStatus};
use crate::round_metrics::RoundMetricState;

/// Lightweight pipeline progress for instant UI refresh.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PipelineSnapshot {
    pub current_step: Option<String>,
    pub current_step_label: Option<String>,
    pub steps_done: u32,
    pub steps_total: u32,
    pub awaiting_approval: bool,
    pub plan_summary: String,
}

/// Snapshot pushed to the frontend on each runtime change.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeUpdate {
    pub active_roles: Vec<String>,
    pub round_metrics: Option<RoundMetricState>,
    pub pipeline: Option<PipelineSnapshot>,
    pub trigger: String,
}

static SENDER: Mutex<Option<mpsc::UnboundedSender<RuntimeUpdate>>> = Mutex::new(None);
static PIPELINE: Mutex<Option<PipelineSnapshot>> = Mutex::new(None);

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

    PipelineSnapshot {
        current_step,
        current_step_label,
        steps_done,
        steps_total,
        awaiting_approval,
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
    let update = RuntimeUpdate {
        active_roles: crate::round_metrics::get_active_roles(),
        round_metrics: crate::round_metrics::snapshot(),
        pipeline,
        trigger: trigger.to_string(),
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
            trigger: "test".into(),
        };
        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("pipeline"));
        assert!(json.contains("steps_done"));
    }
}
