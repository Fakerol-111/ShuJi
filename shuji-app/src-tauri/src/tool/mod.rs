use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::actor::ActorMessage;
use crate::api::client::LlmClient;
use crate::models::role::Role;
use crate::workflow::WorkflowGraph;
use tokio::sync::mpsc;

pub mod audit_tools;
pub mod cache;
pub mod command_ops;
pub mod dispatch;
pub mod documents;
pub mod editor;
pub mod file_ops;
pub mod lint_ops;
pub mod neige_special;
pub mod output;
pub mod path;
pub mod python_cmd;
pub mod registry;
pub mod shangshuling_special;
pub mod test_env;
mod tool_log;

pub use audit_tools::*;
pub use cache::*;
pub use command_ops::*;
pub use dispatch::*;
pub use file_ops::*;
pub use neige_special::*;
pub use output::*;
pub use path::*;
pub use shangshuling_special::*;

// ── Tool context for special tools ─────────────────────────────────

/// Optional context passed to special tools that need access to system
/// resources beyond the filesystem (API client, cancel map, etc.).
/// Currently used by 内阁's special tools (cancel_agent, update_soul,
/// expand_requirements, create_skill).
pub struct ToolContext {
    pub working_dir: PathBuf,
    pub cancel_map: Option<crate::CancelMap>,
    pub client: Option<Arc<LlmClient>>,
    pub model: Option<String>,
    /// Fast mailbox senders for interrupting departments immediately.
    pub fast_txs: Option<crate::FastTxMap>,
    /// Department channel map (used by Shangshuling assign_task)
    pub peers: Option<HashMap<Role, mpsc::UnboundedSender<ActorMessage>>>,
    /// Workflow graph reference (used by Shangshuling assign_task to record edges)
    pub workflow_graph: Option<Arc<tokio::sync::Mutex<WorkflowGraph>>>,
}
