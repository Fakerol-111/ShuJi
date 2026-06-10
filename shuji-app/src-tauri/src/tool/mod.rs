use std::path::PathBuf;
use std::sync::Arc;

use crate::api::client::AnthropicClient;

pub mod audit_tools;
pub mod cache;
pub mod command_ops;
pub mod dispatch;
pub mod documents;
pub mod file_ops;
pub mod neige_special;
pub mod output;
pub mod path;
pub mod registry;
mod tool_log;

pub use audit_tools::*;
pub use cache::*;
pub use command_ops::*;
pub use dispatch::*;
pub use file_ops::*;
pub use neige_special::*;
pub use output::*;
pub use path::*;

// ── Tool context for special tools ─────────────────────────────────

/// Optional context passed to special tools that need access to system
/// resources beyond the filesystem (API client, cancel map, etc.).
/// Currently used by 内阁's special tools (cancel_agent, update_soul,
/// expand_requirements, create_skill).
pub struct ToolContext {
    pub working_dir: PathBuf,
    pub cancel_map: Option<crate::CancelMap>,
    pub client: Option<Arc<AnthropicClient>>,
    pub model: Option<String>,
    /// Fast mailbox senders for interrupting departments immediately.
    pub fast_txs: Option<crate::FastTxMap>,
}
