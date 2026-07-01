//! Public types for the Session module.
//!
//! Extracted from `mod.rs` as a mechanical move. The types are re-exported
//! from `mod.rs` via `pub use` so all external paths
//! (`crate::api::session::SessionSnapshot`, etc.) remain unchanged.

/// Information about a single tool call returned by the LLM.
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

/// Outcome of one Session::step().
pub enum StepResult {
    Text(String),
    ToolCalls {
        calls: Vec<ToolCallInfo>,
        /// Text content from the assistant message that also contained tool_calls.
        /// The actor layer uses this to display text alongside tool execution results.
        text: String,
    },
}

/// Opaque snapshot of Session internals, used for interrupt/restore.
#[derive(Clone)]
pub struct SessionSnapshot {
    pub(crate) messages: Vec<serde_json::Value>,
}

impl SessionSnapshot {
    pub fn from_messages(messages: Vec<serde_json::Value>) -> Self {
        Self { messages }
    }

    /// Access the messages for inspection (testing, debugging).
    pub fn messages(&self) -> &[serde_json::Value] {
        &self.messages
    }
}
