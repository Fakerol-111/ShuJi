use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// A real-time step event emitted during agent execution.
/// Frontend subscribes via `dept-step` Tauri event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptStepEntry {
    pub dept: String,
    pub ts: String,
    pub kind: DeptStepKind,
}

impl DeptStepEntry {
    pub fn new(dept: &str, kind: DeptStepKind) -> Self {
        Self {
            dept: dept.to_string(),
            ts: chrono::Local::now().to_rfc3339(),
            kind,
        }
    }
}

/// Kinds of step events in the agent execution loop.
/// Frontend renders each kind differently in the DeptInspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DeptStepKind {
    #[serde(rename = "iteration")]
    Iteration { n: u32 },
    #[serde(rename = "thinking")]
    Thinking { content: String },
    #[serde(rename = "tool_call")]
    ToolCall {
        tool: String,
        args: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool: String,
        ok: bool,
        summary: String,
    },
    #[serde(rename = "text")]
    Text { content: String },
}

/// Channel sender type for department step events.
pub type DeptStepSender = mpsc::UnboundedSender<DeptStepEntry>;
