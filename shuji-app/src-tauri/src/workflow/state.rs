//! WorkflowState: tracks the current workflow profile + stage.
//!
//! Persisted at `.shuji/workflow_state.json`. Updated on key milestones:
//! - `send_message` starts: writes profile_id + chain_id
//! - 内阁 route_to 尚书令: current_stage = execution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const SUBDIR: &str = ".shuji";

/// Current workflow state, written during send_message lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    /// Active profile id (e.g. "brownfield_optimize").
    pub profile_id: String,
    /// Governance level.
    pub governance: String,
    /// Execution chain id (e.g. "brownfield_patch").
    pub execution_chain_id: String,
    /// Current stage: "init", "planning", "approval", "execution", "summary", "done".
    pub current_stage: String,
    /// Optional artifacts created during the workflow.
    #[serde(default)]
    pub artifacts: HashMap<String, String>,
}

impl WorkflowState {
    pub fn new(profile_id: &str, governance: &str, execution_chain_id: &str) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            governance: governance.to_string(),
            execution_chain_id: execution_chain_id.to_string(),
            current_stage: "init".to_string(),
            artifacts: HashMap::new(),
        }
    }

    /// Write state to project dir.
    pub async fn save_to(&self, project_dir: &Path) {
        let dir = project_dir.join(SUBDIR);
        let _ = tokio::fs::create_dir_all(&dir).await;
        let path = dir.join("workflow_state.json");
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = tokio::fs::write(&path, &content).await;
        }
    }

    /// Read state from project dir.
    pub async fn load_from(project_dir: &Path) -> Option<Self> {
        let path = project_dir.join(SUBDIR).join("workflow_state.json");
        let content = tokio::fs::read_to_string(&path).await.ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Transition to a new stage.
    pub fn transition(&mut self, stage: &str) {
        self.current_stage = stage.to_string();
    }

    /// Add an artifact reference.
    pub fn add_artifact(&mut self, key: &str, value: &str) {
        self.artifacts.insert(key.to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_save_and_load() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = WorkflowState::new("brownfield_optimize", "standard", "brownfield_patch");
        state.save_to(tmp.path()).await;

        let loaded = WorkflowState::load_from(tmp.path()).await.unwrap();
        assert_eq!(loaded.profile_id, "brownfield_optimize");
        assert_eq!(loaded.governance, "standard");
        assert_eq!(loaded.current_stage, "init");
    }

    #[tokio::test]
    async fn test_transition() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut state = WorkflowState::new("greenfield_standard", "standard", "greenfield_full");
        state.transition("execution");
        state.save_to(tmp.path()).await;

        let loaded = WorkflowState::load_from(tmp.path()).await.unwrap();
        assert_eq!(loaded.current_stage, "execution");
    }

    #[tokio::test]
    async fn test_no_file_returns_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        let loaded = WorkflowState::load_from(tmp.path()).await;
        assert!(loaded.is_none());
    }
}
