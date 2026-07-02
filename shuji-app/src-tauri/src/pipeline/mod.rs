//! Pipeline engine: machine-executable workflow plans.
//!
//! 内阁 outputs a JSON plan → PipelineEngine executes it mechanically.
//! Departments run without `route_to` — the engine handles all routing.

pub mod artifacts;
pub mod engine;
pub mod handlers;
pub mod schema;
pub mod supervisor;
pub mod templates;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Deserialized from 内阁's JSON `submit_plan` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelinePlan {
    pub plan_id: String,
    pub summary: String,
    #[serde(default)]
    pub estimated_complexity: String, // "low" | "medium" | "high"
    #[serde(default)]
    pub created: String,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_id: String,
    pub description: String,
    pub action: String, // ask_user | dispatch_to | parallel | approval_gate | self_execute
    pub action_params: serde_json::Value,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub require_approval: bool,
    #[serde(default = "default_on_failure")]
    pub on_failure: String, // wake_cabinet | skip | abort
    #[serde(default = "default_retry")]
    pub retry: u32,
}

fn default_on_failure() -> String {
    "wake_cabinet".into()
}
fn default_retry() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "skipped")]
    Skipped,
}

/// Runtime state of plan execution — persisted to `.shuji/pipeline/runtime.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRuntime {
    pub plan: PipelinePlan,
    pub step_status: HashMap<String, StepStatus>,
    pub current_step: Option<String>,
    /// step_id → primary artifact doc_id
    pub artifacts: HashMap<String, String>,
    pub error_log: Vec<String>,
}

impl PlanRuntime {
    pub fn new(plan: PipelinePlan) -> Self {
        let mut step_status = HashMap::new();
        for step in &plan.steps {
            step_status.insert(step.step_id.clone(), StepStatus::Pending);
        }
        Self {
            plan,
            step_status,
            current_step: None,
            artifacts: HashMap::new(),
            error_log: Vec::new(),
        }
    }

    /// Find the first pending step whose depends_on are all Done or Skipped.
    pub fn find_executable_step(&self) -> Option<String> {
        for step in &self.plan.steps {
            if self.step_status.get(&step.step_id) != Some(&StepStatus::Pending) {
                continue;
            }
            let deps_met = step.depends_on.iter().all(|dep_id| {
                matches!(
                    self.step_status.get(dep_id),
                    Some(StepStatus::Done) | Some(StepStatus::Skipped)
                )
            });
            if deps_met {
                return Some(step.step_id.clone());
            }
        }
        None
    }

    pub fn all_done(&self) -> bool {
        self.plan.steps.iter().all(|s| {
            matches!(
                self.step_status.get(&s.step_id),
                Some(StepStatus::Done) | Some(StepStatus::Skipped)
            )
        })
    }
}

/// Outcome of pipeline engine execution — consumed by `commands/workflow.rs`.
#[derive(Debug, Clone)]
pub enum PipelineResult {
    /// All steps completed normally.
    Complete { runtime: PlanRuntime },
    /// Waiting for user reply (ask_user step).
    AwaitingUserInput {
        step_id: String,
        question: String,
        runtime: PlanRuntime,
    },
    /// Waiting for emperor approval (approval_gate step).
    AwaitingApproval {
        doc_id: String,
        step_id: String,
        runtime: PlanRuntime,
    },
    /// Step failed and on_failure=wake_cabinet — need to wake 内阁.
    StepFailed {
        step_id: String,
        reason: String,
        runtime: PlanRuntime,
    },
    /// Pipeline aborted by user or on_failure=abort.
    Aborted { runtime: PlanRuntime },
    /// Deadlock: all remaining steps blocked but none executable.
    Deadlock { runtime: PlanRuntime },
}

impl PlanRuntime {
    /// Persist to .shuji/pipeline/runtime.json.
    pub async fn save_to(&self, project_dir: &std::path::Path) -> Result<(), String> {
        let dir = project_dir.join(".shuji").join("pipeline");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("create pipeline dir: {}", e))?;
        let path = dir.join("runtime.json");
        let content =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize runtime: {}", e))?;
        tokio::fs::write(&path, &content)
            .await
            .map_err(|e| format!("write runtime: {}", e))?;
        crate::runtime_notify::update_pipeline(self);
        Ok(())
    }

    /// Load from .shuji/pipeline/runtime.json.
    /// Migrates legacy `"route_to"` actions to `"dispatch_to"` on load.
    pub async fn load_from(project_dir: &std::path::Path) -> Option<Self> {
        let path = project_dir
            .join(".shuji")
            .join("pipeline")
            .join("runtime.json");
        let content = tokio::fs::read_to_string(&path).await.ok()?;
        let migrated = content.replace(r#""action": "route_to""#, r#""action": "dispatch_to""#);
        serde_json::from_str(&migrated).ok()
    }

    /// Delete runtime file (cleanup after completion).
    pub async fn cleanup(project_dir: &std::path::Path) {
        let path = Self::runtime_file_path(project_dir);
        let _ = tokio::fs::remove_file(&path).await;
        crate::runtime_notify::clear_pipeline();
    }

    /// Path to persisted runtime (`.shuji/pipeline/runtime.json`).
    pub fn runtime_file_path(project_dir: &std::path::Path) -> std::path::PathBuf {
        project_dir
            .join(".shuji")
            .join("pipeline")
            .join("runtime.json")
    }
}

/// Routing decision for `send_message`: resume pipeline when disk holds a paused
/// runtime and the supervisor is not already executing a plan.
pub fn should_resume_from_disk(has_paused_runtime: bool, supervisor_running: bool) -> bool {
    has_paused_runtime && !supervisor_running
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_resume_from_disk_when_paused_and_supervisor_idle() {
        assert!(should_resume_from_disk(true, false));
    }

    #[test]
    fn should_not_resume_without_runtime_on_disk() {
        assert!(!should_resume_from_disk(false, false));
        assert!(!should_resume_from_disk(false, true));
    }

    #[test]
    fn should_not_resume_while_supervisor_running() {
        assert!(!should_resume_from_disk(true, true));
    }

    #[test]
    fn test_find_executable_step_no_deps() {
        let plan = PipelinePlan {
            plan_id: "test".into(),
            summary: "test".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![PlanStep {
                step_id: "s1".into(),
                description: "step1".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "工部", "task": "do"}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            }],
        };
        let rt = PlanRuntime::new(plan);
        assert_eq!(rt.find_executable_step(), Some("s1".into()));
    }

    #[test]
    fn test_find_executable_step_blocked() {
        let plan = PipelinePlan {
            plan_id: "test".into(),
            summary: "test".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![
                PlanStep {
                    step_id: "s1".into(),
                    description: "step1".into(),
                    action: "dispatch_to".into(),
                    action_params: serde_json::json!({"target": "工部", "task": "do"}),
                    depends_on: vec![],
                    require_approval: false,
                    on_failure: "wake_cabinet".into(),
                    retry: 1,
                },
                PlanStep {
                    step_id: "s2".into(),
                    description: "step2".into(),
                    action: "dispatch_to".into(),
                    action_params: serde_json::json!({"target": "刑部", "task": "test"}),
                    depends_on: vec!["s1".into()],
                    require_approval: false,
                    on_failure: "wake_cabinet".into(),
                    retry: 1,
                },
            ],
        };
        let rt = PlanRuntime::new(plan);
        // s1 done, s2 should be executable
        let mut rt2 = rt.clone();
        rt2.step_status.insert("s1".into(), StepStatus::Done);
        assert_eq!(rt2.find_executable_step(), Some("s2".into()));
        // s1 not done, only s1 executable
        assert_eq!(rt.find_executable_step(), Some("s1".into()));
    }

    #[test]
    fn test_all_done() {
        let plan = PipelinePlan {
            plan_id: "test".into(),
            summary: "test".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![PlanStep {
                step_id: "s1".into(),
                description: "step1".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "工部", "task": "do"}),
                depends_on: vec![],
                require_approval: false,
                on_failure: "wake_cabinet".into(),
                retry: 1,
            }],
        };
        let mut rt = PlanRuntime::new(plan);
        assert!(!rt.all_done());
        rt.step_status.insert("s1".into(), StepStatus::Done);
        assert!(rt.all_done());
    }

    #[test]
    fn test_deadlock_detection() {
        // Two steps depend on each other → deadlock
        let plan = PipelinePlan {
            plan_id: "test".into(),
            summary: "test".into(),
            estimated_complexity: "low".into(),
            created: "2026-01-01".into(),
            steps: vec![
                PlanStep {
                    step_id: "s1".into(),
                    description: "step1".into(),
                    action: "dispatch_to".into(),
                    action_params: serde_json::json!({"target": "工部"}),
                    depends_on: vec!["s2".into()],
                    require_approval: false,
                    on_failure: "wake_cabinet".into(),
                    retry: 1,
                },
                PlanStep {
                    step_id: "s2".into(),
                    description: "step2".into(),
                    action: "dispatch_to".into(),
                    action_params: serde_json::json!({"target": "刑部"}),
                    depends_on: vec!["s1".into()],
                    require_approval: false,
                    on_failure: "wake_cabinet".into(),
                    retry: 1,
                },
            ],
        };
        let rt = PlanRuntime::new(plan);
        assert!(!rt.all_done());
        assert_eq!(rt.find_executable_step(), None); // deadlock
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let plan = PipelinePlan {
            plan_id: "p1".into(),
            summary: "test".into(),
            estimated_complexity: "medium".into(),
            created: "2026-01-01".into(),
            steps: vec![PlanStep {
                step_id: "s1".into(),
                description: "do thing".into(),
                action: "dispatch_to".into(),
                action_params: serde_json::json!({"target": "工部", "task": "code"}),
                depends_on: vec![],
                require_approval: true,
                on_failure: "wake_cabinet".into(),
                retry: 2,
            }],
        };
        let mut rt = PlanRuntime::new(plan);
        rt.step_status.insert("s1".into(), StepStatus::InProgress);

        let tmp = tempfile::TempDir::new().unwrap();
        rt.save_to(tmp.path()).await.unwrap();

        let loaded = PlanRuntime::load_from(tmp.path()).await.unwrap();
        assert_eq!(loaded.plan.plan_id, "p1");
        assert_eq!(loaded.step_status.get("s1"), Some(&StepStatus::InProgress));
    }
}
