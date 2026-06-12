use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use std::fmt;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::agent::r#trait::Agent;
use crate::api::control::RouteMsgType;
use crate::config::RuntimeConfig;
use crate::logging::logger::Logger;
use crate::models::chat::ChatMessage;
use crate::models::role::Role;

mod routing;
mod spawn;
pub use routing::*;
pub use spawn::*;

/// A real-time department log entry emitted to the frontend status panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptLogEntry {
    pub dept: String,
    pub action: String,
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl DeptLogEntry {
    pub fn new(dept: &str, action: &str) -> Self {
        Self {
            dept: dept.to_string(),
            action: action.to_string(),
            ts: chrono::Local::now().format("%H:%M:%S").to_string(),
            detail: None,
        }
    }

    pub fn with_detail(dept: &str, action: &str, detail: &str) -> Self {
        Self {
            dept: dept.to_string(),
            action: action.to_string(),
            ts: chrono::Local::now().format("%H:%M:%S").to_string(),
            detail: Some(detail.to_string()),
        }
    }
}

/// High-priority messages sent via the fast mailbox channel.
/// Each actor gets a dedicated bounded mpsc channel (capacity 16) for
/// interrupt signals that bypass the normal message queue.
#[derive(Debug, Clone)]
pub enum FastMessage {
    /// Immediately stop current tool execution and return.
    Interrupt,
}

/// Messages that actors send to each other.
#[derive(Debug, Clone)]
pub struct ActorMessage {
    pub msg_type: RouteMsgType,
    pub subject: String,
    pub payload: Option<String>,
    pub reply_to: Option<mpsc::UnboundedSender<String>>,
}

impl ActorMessage {
    pub fn new(subject: impl Into<String>, msg_type: RouteMsgType) -> Self {
        Self {
            msg_type,
            subject: subject.into(),
            payload: None,
            reply_to: None,
        }
    }

    pub fn interrupt() -> Self {
        Self {
            msg_type: RouteMsgType::Interrupt,
            subject: String::new(),
            payload: None,
            reply_to: None,
        }
    }

    fn subject(&self) -> &str {
        &self.subject
    }
}

/// Per-actor context passed to `run_actor`.
pub struct ActorContext {
    pub role: Role,
    pub agent: Box<dyn Agent>,
    pub rx: mpsc::UnboundedReceiver<ActorMessage>,
    /// Fast mailbox receiver for high-priority interrupt signals.
    pub fast_rx: tokio::sync::Mutex<mpsc::Receiver<FastMessage>>,
    pub peers: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    pub emperor_tx: mpsc::Sender<ChatMessage>,
    pub dept_log_tx: mpsc::Sender<DeptLogEntry>,
    pub plan_tx: mpsc::Sender<serde_json::Value>,
    pub milestone_tx: mpsc::Sender<String>,
    pub project_dir: PathBuf,
    pub working_dir: PathBuf,
    pub cancel: Arc<AtomicBool>,
    /// Cancel flags for ALL agents. Only populated for 内阁.
    /// 内阁 uses this to interrupt other agents via the `cancel_agent` tool.
    pub cancel_map: Option<crate::CancelMap>,
    pub logger: Logger,
    /// Shared context across all actors — stores last output per role.
    pub shared_context: Arc<Mutex<HashMap<Role, String>>>,
    /// Retry counters for automatic failure fallback, keyed by failing role.
    pub failure_retries: Arc<Mutex<HashMap<Role, u32>>>,
    /// Full conversation history between 内阁 and emperor.
    pub talk_history: Arc<Mutex<Vec<String>>>,
    /// Per-agent task plan (populated by 工部尚书 actor for multi-step execution).
    pub plan: Arc<Mutex<Vec<String>>>,
    /// Current skill name for cross-turn persistence (内阁 only).
    pub current_skill: Arc<Mutex<Option<String>>>,
    /// 文移图 — 部门间任务流转 DAG（共享引用，由 forward_route 写入）
    pub workflow_graph: Option<Arc<tokio::sync::Mutex<crate::workflow::WorkflowGraph>>>,
    /// Runtime configuration
    pub runtime_config: Arc<RuntimeConfig>,
}

/// The central actor system, holding all senders.
/// Created at startup, injected into commands.
pub struct ActorSystem {
    /// Senders for all department actors, keyed by Role.
    pub senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    /// Fast mailbox senders for high-priority interrupt signals.
    pub fast_txs: HashMap<Role, mpsc::Sender<FastMessage>>,
    /// Sender for emperor-facing chat messages.
    pub emperor_tx: mpsc::Sender<ChatMessage>,
    /// Sender for department log entries (→ frontend DeptStatusPanel).
    pub dept_log_tx: mpsc::Sender<DeptLogEntry>,
    /// Per-agent cancel flags, indexed by Role.
    pub cancel_map: crate::CancelMap,
    /// Global cancel flag for the frontend cancel button.
    pub cancel: Arc<AtomicBool>,
    /// 文移图 — 共享引用，send_message 归档和 actor 写入都用同一个 Arc。
    pub workflow_graph: Arc<tokio::sync::Mutex<crate::workflow::WorkflowGraph>>,
}

impl ActorSystem {
    pub fn new(
        senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
        fast_txs: HashMap<Role, mpsc::Sender<FastMessage>>,
        emperor_tx: mpsc::Sender<ChatMessage>,
        dept_log_tx: mpsc::Sender<DeptLogEntry>,
        cancel_map: crate::CancelMap,
        cancel: Arc<AtomicBool>,
        workflow_graph: Arc<tokio::sync::Mutex<crate::workflow::WorkflowGraph>>,
    ) -> Self {
        Self {
            senders,
            fast_txs,
            emperor_tx,
            dept_log_tx,
            cancel_map,
            cancel,
            workflow_graph,
        }
    }

    /// Send a message to a role's actor.
    pub fn send(&self, target: &Role, msg: ActorMessage) -> Result<(), String> {
        match self.senders.get(target) {
            Some(tx) => tx
                .send(msg)
                .map_err(|_| format!("{} actor 已关闭", target.name())),
            None => Err(format!("找不到 {} actor", target.name())),
        }
    }
}

impl Drop for ActorSystem {
    fn drop(&mut self) {
        // 1. Set all per-actor cancel flags
        if let Ok(map) = self.cancel_map.lock() {
            for flag in map.values() {
                flag.store(true, Ordering::SeqCst);
            }
        }

        // 2. Send Interrupt via fast mailboxes to all actors
        for tx in self.fast_txs.values() {
            let _ = tx.try_send(FastMessage::Interrupt);
        }

        // 3. Send Interrupt to all actors via main mailbox
        for tx in self.senders.values() {
            let _ = tx.send(ActorMessage::interrupt());
        }

        log_console!(
            "[actor] ActorSystem dropped — all cancel flags set, FastMessage::Interrupt sent to all actors"
        );
    }
}

impl fmt::Debug for ActorSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorSystem")
            .field("actor_count", &self.senders.len())
            .field("cancel", &self.cancel)
            .finish()
    }
}
