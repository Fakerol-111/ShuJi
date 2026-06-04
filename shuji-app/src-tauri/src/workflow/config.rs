//! WorkflowConfig: reads/writes `.shuji/workflow_config.json`.
//!
//! Intent describes the *what* (greenfield, brownfield, bugfix, demo).
//! Governance describes the *how* (full, standard, fast, audit).
//! When no file exists, defaults to `auto` + `standard` for backward compat.

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{Governance, Intent};

const SUBDIR: &str = ".shuji";

/// Per-project workflow configuration persisted at `.shuji/workflow_config.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowConfig {
    /// Primary intent: what kind of task this is.
    #[serde(default)]
    pub intent: Intent,

    /// Governance level: how thorough the process should be.
    #[serde(default)]
    pub governance: Governance,

    /// Single-task override. When set, takes precedence over `intent`
    /// for the current `send_message` call only. Cleared after use.
    pub intent_override: Option<Intent>,
}

impl WorkflowConfig {
    /// Read config from project dir. Returns default (auto + standard) if file
    /// doesn't exist or can't be parsed, ensuring backward compatibility.
    pub async fn load_from(project_dir: &Path) -> Self {
        let path = project_dir.join(SUBDIR).join("workflow_config.json");
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => match serde_json::from_str::<WorkflowConfig>(&content) {
                Ok(cfg) => cfg,
                Err(_) => {
                    log_console!("[workflow] corrupt workflow_config.json, using defaults");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Save config to project dir. Creates `.shuji/` if needed.
    pub async fn save_to(&self, project_dir: &Path) -> Result<(), String> {
        let dir = project_dir.join(SUBDIR);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("创建 .shuji 目录失败: {}", e))?;
        let path = dir.join("workflow_config.json");
        let content =
            serde_json::to_string_pretty(self).map_err(|e| format!("序列化失败: {}", e))?;
        tokio::fs::write(&path, &content)
            .await
            .map_err(|e| format!("写入 workflow_config 失败: {}", e))?;
        log_console!("[workflow] config saved: {:?}", self);
        Ok(())
    }

    /// Resolve effective intent: intent_override > intent > auto default.
    pub fn effective_intent(&self) -> Intent {
        self.intent_override.unwrap_or(self.intent)
    }

    /// Consume the override after reading it.
    pub fn take_override(&mut self) -> Option<Intent> {
        self.intent_override.take()
    }
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            intent: Intent::Auto,
            governance: Governance::Standard,
            intent_override: None,
        }
    }
}
