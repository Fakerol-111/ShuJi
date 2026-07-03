use std::future::Future;
use std::pin::Pin;

use crate::api::session::SessionSnapshot;
use crate::models::dept_step::DeptStepKind;
use crate::models::role::Role;

pub type ToolFuture = Pin<Box<dyn Future<Output = String> + Send + 'static>>;

/// Callback for periodic checkpoint saves.
/// Receives an owned SessionSnapshot (cloned inside the controller),
/// so the async block does not borrow the caller's session.
pub type CheckpointFn =
    Box<dyn Fn(SessionSnapshot) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Callback for mid-run context compaction.
/// Takes the flat messages array and persists a compacted version to disk.
/// Does NOT modify the in-memory session -- the compressed context is loaded
/// automatically on the next execute() call. This avoids disrupting the
/// running conversation mid-turn.
pub type CompactFn =
    Box<dyn Fn(Vec<serde_json::Value>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Callback for real-time step events emitted during AgentController::run().
/// Receives a DeptStepKind to emit for each iteration, thinking, tool call, etc.
pub type DeptStepCallback = Box<dyn Fn(DeptStepKind) + Send + Sync>;

/// Type of a cross-department routing message.
#[derive(Debug, Clone, Copy)]
pub enum RouteMsgType {
    Task,
    Replace,
    Interrupt,
}

/// Structured routing instruction produced by the LLM calling `route_to`.
#[derive(Debug, Clone)]
pub struct RouteTo {
    pub target: Role,
    pub msg_type: RouteMsgType,
    pub subject: String,
    /// Optional inline payload for short instructions (bypasses document write).
    pub payload: Option<String>,
    /// Document IDs from upstream (dispatch_to subject when it is a doc id).
    pub doc_ids: Vec<String>,
}

/// Outcome of one AgentController::run() call.
#[derive(Debug, Clone)]
pub enum RunResult {
    /// Agent completed normally (text-only response).
    Done(String),
    /// Agent issued a route_to instruction -- forward the route.
    Routed { text: String, route: RouteTo },
    /// Agent was interrupted / force-stopped / consecutive errors.
    Stopped(String),
}

impl RunResult {
    /// Extract text regardless of variant.
    pub fn text(&self) -> &str {
        match self {
            RunResult::Done(t) | RunResult::Stopped(t) => t,
            RunResult::Routed { text, .. } => text,
        }
    }

    /// Consume and return text.
    pub fn into_text(self) -> String {
        match self {
            RunResult::Done(t) | RunResult::Stopped(t) => t,
            RunResult::Routed { text, .. } => text,
        }
    }

    /// Extract RouteTo if present.
    pub fn into_route(self) -> Option<RouteTo> {
        match self {
            RunResult::Routed { route, .. } => Some(route),
            _ => None,
        }
    }

    /// Consume and return the legacy `(String, Option<RouteTo>)` tuple.
    /// This is a migration helper — new code should match on the enum directly.
    /// Only `request_reauth` (via audit_tools) actively produces `Routed` today;
    /// all other agents discard the route component.
    pub fn into_tuple(self) -> (String, Option<RouteTo>) {
        match self {
            RunResult::Done(text) => (text, None),
            RunResult::Routed { text, route } => (text, Some(route)),
            RunResult::Stopped(text) => (text, None),
        }
    }
}

pub(super) fn route_msg_type_from_str(s: &str) -> Option<RouteMsgType> {
    match s {
        "task" => Some(RouteMsgType::Task),
        "replace" => Some(RouteMsgType::Replace),
        "interrupt" => Some(RouteMsgType::Interrupt),
        _ => None,
    }
}

pub fn role_from_name(s: &str) -> Option<Role> {
    Role::from_name(s)
}
