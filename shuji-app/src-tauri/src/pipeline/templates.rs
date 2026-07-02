//! Pipeline plan templates generated from workflow profiles.
//!
//! These provide standard plan structures that 内阁 can reference
//! when constructing `submit_pipeline_plan` calls.

use super::{PipelinePlan, PlanStep};

/// Generate a PipelinePlan from a profile identifier.
///
/// Supported profiles:
/// - `demo`: init → route 工部 → route 刑部 → validate
/// - `greenfield_standard`: expand → design → review → approval → execution → validate → summary
/// - `bugfix`: route 尚书令 → validate
pub fn pipeline_from_profile(
    profile_id: &str,
    plan_id: &str,
    summary: &str,
) -> Option<PipelinePlan> {
    match profile_id {
        "demo" => Some(demo_plan(plan_id, summary)),
        "greenfield_standard" => Some(greenfield_standard_plan(plan_id, summary)),
        "bugfix" => Some(bugfix_plan(plan_id, summary)),
        _ => None,
    }
}

/// Returns recommended validate_config lint settings for a profile.
pub fn profile_lint_recommendation(profile_id: &str) -> (bool, bool) {
    match profile_id {
        "demo" => (false, false),
        "greenfield_standard" => (true, true),
        "bugfix" => (true, false),
        _ => (false, false),
    }
}

fn demo_plan(plan_id: &str, summary: &str) -> PipelinePlan {
    PipelinePlan {
        plan_id: plan_id.to_string(),
        summary: summary.to_string(),
        estimated_complexity: "low".to_string(),
        created: chrono::Local::now().to_rfc3339(),
        steps: vec![
            PlanStep {
                step_id: "init".into(),
                description: "init environment".into(),
                action: "self_execute".into(),
                action_params: serde_json::json!({"handler": "noop"}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "gongbu".into(),
                description: "Works Ministry coding".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "尚书令", "task": summary}),
                depends_on: vec!["init".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "xingbu".into(),
                description: "Justice Ministry testing".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "尚书令", "task": format!("验证: {}", summary)}),
                depends_on: vec!["gongbu".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "validate".into(),
                description: "automated validation".into(),
                action: "self_execute".into(),
                action_params: serde_json::json!({"handler": "validate_delivery", "run_lint": false}),
                depends_on: vec!["xingbu".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
        ],
    }
}

fn greenfield_standard_plan(plan_id: &str, summary: &str) -> PipelinePlan {
    PipelinePlan {
        plan_id: plan_id.to_string(),
        summary: summary.to_string(),
        estimated_complexity: "high".to_string(),
        created: chrono::Local::now().to_rfc3339(),
        steps: vec![
            PlanStep {
                step_id: "expand".into(),
                description: "expand requirements".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "内阁", "task": "expand_requirements"}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "design".into(),
                description: "solution design".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "中书令", "task": summary}),
                depends_on: vec!["expand".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "review".into(),
                description: "design review".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({
                    "target": "门下侍中",
                    "task": "Review the design document and create a revw report"
                }),
                depends_on: vec!["design".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "approval".into(),
                description: "emperor approval".into(),
                action: "approval_gate".into(),
                action_params: serde_json::json!({}),
                depends_on: vec!["review".into()],
                require_approval: false,
                on_failure: "abort".into(),
                retry: 0,
            },
            PlanStep {
                step_id: "execution".into(),
                description: "executor dispatch".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "尚书令", "task": summary}),
                depends_on: vec!["approval".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "validate".into(),
                description: "automated validation".into(),
                action: "self_execute".into(),
                action_params: serde_json::json!({"handler": "validate_delivery", "run_lint": true}),
                depends_on: vec!["execution".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "summary".into(),
                description: "summary report".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "内阁", "task": "generate summary"}),
                depends_on: vec!["validate".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
        ],
    }
}

fn bugfix_plan(plan_id: &str, summary: &str) -> PipelinePlan {
    PipelinePlan {
        plan_id: plan_id.to_string(),
        summary: summary.to_string(),
        estimated_complexity: "low".to_string(),
        created: chrono::Local::now().to_rfc3339(),
        steps: vec![
            PlanStep {
                step_id: "diagnose".into(),
                description: "diagnose".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "中书令", "task": format!("Diagnose: {}", summary)}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "shangshuling".into(),
                description: "executor dispatch fix".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "尚书令", "task": summary}),
                depends_on: vec!["diagnose".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
            PlanStep {
                step_id: "validate".into(),
                description: "validate fix".into(),
                action: "self_execute".into(),
                action_params: serde_json::json!({"handler": "validate_delivery", "run_lint": false}),
                depends_on: vec!["shangshuling".into()],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::schema::validate_plan_json;

    fn validate_profile_plan(plan: &PipelinePlan) {
        let json = serde_json::to_string(plan).unwrap();
        let result = validate_plan_json(&json);
        assert!(
            result.is_ok(),
            "profile plan should pass validation: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_demo_profile_passes_validation() {
        let plan = demo_plan("plan-demo-001", "demo task");
        validate_profile_plan(&plan);
        assert_eq!(plan.steps.len(), 4);
    }

    #[test]
    fn test_greenfield_standard_passes_validation() {
        let plan = greenfield_standard_plan("plan-gs-001", "new feature");
        validate_profile_plan(&plan);
        assert_eq!(plan.steps.len(), 7);
    }

    #[test]
    fn test_bugfix_passes_validation() {
        let plan = bugfix_plan("plan-bf-001", "fix bug");
        validate_profile_plan(&plan);
        assert_eq!(plan.steps.len(), 3);
    }

    #[test]
    fn test_unknown_profile_returns_none() {
        let result = pipeline_from_profile("nonexistent", "p1", "test");
        assert!(result.is_none());
    }
}
