use std::time::Instant;

use crate::api::session::{Session, SessionSnapshot};
use crate::models::dept_step::DeptStepEntry;

// ── Sub-modules ──────────────────────────────────────────────────────────────
mod iterations;
mod lifecycle;
mod loop_runner;
mod routing;
mod tool_exec;
mod types;
mod watchdog;
mod wrap_up;

// ── Re-export public API (same as original `pub` items in control.rs) ───────
pub use types::{
    role_from_name, CheckpointFn, CompactFn, DeptStepCallback, RouteMsgType, RouteTo, RunResult,
    ToolFuture,
};

const INTERRUPT_RESPONSE: &str = "\n\n[System] Current processing has been interrupted by 皇帝";

/// Control layer for tool-use agents.
///
/// Owns the tool-iteration loop, cancel/interrupt/restart lifecycle,
/// watchdog diagnostics, and anything related to "how" the LLM is
/// driven. The LLM itself is a `Session` — this struct controls it.
pub struct AgentController {
    saved: Option<SessionSnapshot>,
    checkpoint_fn: Option<CheckpointFn>,
    last_checkpoint: Instant,
    compact_handler: Option<CompactFn>,
    compact_iter_interval: u32,
    step_emit: Option<DeptStepCallback>,
    created_doc_ids: Vec<String>,
}

impl Default for AgentController {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentController {
    pub fn new() -> Self {
        Self {
            saved: None,
            checkpoint_fn: None,
            last_checkpoint: Instant::now(),
            compact_handler: None,
            compact_iter_interval: 0,
            step_emit: None,
            created_doc_ids: Vec::new(),
        }
    }

    fn collect_document_from_tool(&mut self, tool_name: &str, result: &str) {
        if !matches!(tool_name, "create_document" | "append_document") {
            return;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(result) else {
            return;
        };
        if v.get("ok").and_then(|o| o.as_bool()) != Some(true) {
            return;
        }
        if let Some(id) = v
            .get("path")
            .and_then(|p| p.as_str())
            .filter(|s| !s.is_empty())
        {
            if !self.created_doc_ids.iter().any(|x| x == id) {
                self.created_doc_ids.push(id.to_string());
            }
        }
    }

    /// Resolve collected document IDs into chat-card metadata.
    pub async fn take_documents(
        &mut self,
        working_dir: &std::path::Path,
    ) -> Vec<crate::models::chat::ChatDocument> {
        let ids = std::mem::take(&mut self.created_doc_ids);
        let mut docs: Vec<crate::models::chat::ChatDocument> = Vec::new();
        for id in ids {
            if let Some(doc) = crate::tool::documents::chat_document_from_id(working_dir, &id).await
            {
                if !docs.iter().any(|d| d.id == doc.id) {
                    docs.push(doc);
                }
            }
        }
        docs
    }

    /// Register a handler for real-time step events.
    /// Called at each iteration, thinking block, tool call, and tool result.
    pub fn set_step_emitter(&mut self, emitter: DeptStepCallback) {
        self.step_emit = Some(emitter);
    }

    /// Register a handler for periodic checkpoint saves.
    /// Called at suspension points when `config.checkpoint.interval_secs` has elapsed.
    pub fn set_checkpoint_handler(&mut self, handler: CheckpointFn) {
        self.checkpoint_fn = Some(handler);
    }

    /// Register a handler for mid-run context compaction.
    /// `interval` controls how many tool-call iterations between compactions.
    /// The handler receives the flat session messages and persists a compressed
    /// version to disk. The running session is NOT modified 鈥?the compressed
    /// context is loaded automatically on the next execute() call.
    /// Helps prevent unbounded context growth during long-running agent sessions.
    pub fn set_compact_handler(&mut self, handler: CompactFn, interval: u32) {
        self.compact_handler = Some(handler);
        self.compact_iter_interval = interval;
    }
}

/// Convenience helper: configure step_emit on a controller from AgentInput's dept_step_tx.
/// Call this after creating the controller and before calling run().
pub fn setup_agent_step_emitter(
    controller: &mut AgentController,
    dept_step_tx: &Option<crate::models::dept_step::DeptStepSender>,
    dept: &str,
) {
    if let Some(ref tx) = dept_step_tx {
        let tx = tx.clone();
        let dept = dept.to_string();
        controller.set_step_emitter(Box::new(move |kind| {
            let _ = tx.send(DeptStepEntry::new(&dept, kind));
        }));
    }
}

impl AgentController {
    /// Interrupt the current session.
    ///
    /// Save a snapshot of the current conversation state for
    /// potential resume. Does NOT make an API call 鈥?the LLM
    /// acknowledges the interruption naturally on the next
    /// user message.
    pub async fn interrupt(&mut self, session: &mut Session) {
        self.saved = Some(session.snapshot());
        log_console!("[control] interrupt: snapshot saved");
    }

    /// Restart from a saved snapshot with a new instruction.
    pub fn restart_with(&mut self, session: &mut Session, new_instruction: &str) {
        if let Some(snap) = self.saved.take() {
            session.restore(&snap);
            session.inject(&format!(
                "System: Previous operation was interrupted. 鐨囧笣 has given a new instruction: {}",
                new_instruction
            ));
            log_console!("[control] restart_with: snapshot restored, new instruction injected");
        } else {
            log_console!(
                "[control] restart_with: no saved snapshot 鈥?injecting as new instruction"
            );
            session.inject(&format!(
                "System: 鐨囧笣 has given a new instruction, please start processing: {}",
                new_instruction
            ));
        }
    }

    /// Take the saved snapshot (for external inspection), leaving None.
    pub fn take_snapshot(&mut self) -> Option<SessionSnapshot> {
        self.saved.take()
    }
}
