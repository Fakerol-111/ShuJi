//! Centralized Tauri event name constants and typed emit helpers.
//!
//! Every Tauri event the application emits goes through this module,
//! ensuring event names are never duplicated as raw strings and payload
//! types are enforced at compile time.

use serde::{Deserialize, Serialize};
use tauri::Emitter;

// ── Event name constants ────────────────────────────────────────────

pub const CHAT_MESSAGE: &str = "chat-message";
pub const CHAT_DELTA: &str = "chat-delta";
pub const CHAT_COMPLETE: &str = "chat-complete";
pub const DEPT_LOG: &str = "dept-log";
pub const DEPT_STEP: &str = "dept-step";
pub const PLAN_UPDATE: &str = "plan-update";
pub const PROJECT_UPDATE: &str = "project-update";
pub const USAGE_UPDATE: &str = "usage-update";
pub const RUNTIME_UPDATE: &str = "runtime-update";

// ── PlanUpdate structs (typed replacement for untyped serde_json::Value) ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanBatchEvent {
    pub name: String,
    pub goal: String,
    pub status: PlanBatchStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlanBatchStatus {
    Done,
    Current,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanUpdate {
    pub batches: Vec<PlanBatchEvent>,
    pub current: usize,
    pub complete: bool,
}

impl PlanUpdate {
    /// Parse from the JSON string produced by `agent.plan_display()`.
    /// Returns `None` for `"null"` or malformed JSON.
    pub fn from_json_string(s: &str) -> Option<Self> {
        if s.trim() == "null" {
            return None;
        }
        serde_json::from_str(s).ok()
    }
}

// ── Typed emit helpers ──────────────────────────────────────────────

pub fn emit_chat_message(
    app: &impl Emitter<tauri::Wry>,
    payload: &crate::models::chat::ChatMessage,
) -> tauri::Result<()> {
    app.emit(CHAT_MESSAGE, payload)
}

pub fn emit_chat_delta(
    app: &impl Emitter<tauri::Wry>,
    payload: &crate::api::stream::ChatDeltaEvent,
) -> tauri::Result<()> {
    app.emit(CHAT_DELTA, payload)
}

pub fn emit_chat_complete(
    app: &impl Emitter<tauri::Wry>,
    payload: &crate::models::chat::ChatMessage,
) -> tauri::Result<()> {
    app.emit(CHAT_COMPLETE, payload)
}

pub fn emit_dept_log(
    app: &impl Emitter<tauri::Wry>,
    payload: &crate::actor::DeptLogEntry,
) -> tauri::Result<()> {
    app.emit(DEPT_LOG, payload)
}

pub fn emit_dept_step(
    app: &impl Emitter<tauri::Wry>,
    payload: &crate::models::dept_step::DeptStepEntry,
) -> tauri::Result<()> {
    app.emit(DEPT_STEP, payload)
}

pub fn emit_plan_update(app: &impl Emitter<tauri::Wry>, payload: &PlanUpdate) -> tauri::Result<()> {
    app.emit(PLAN_UPDATE, payload)
}

pub fn emit_project_update(
    app: &impl Emitter<tauri::Wry>,
    payload: &crate::models::project::Project,
) -> tauri::Result<()> {
    app.emit(PROJECT_UPDATE, payload)
}

pub fn emit_usage_update(
    app: &impl Emitter<tauri::Wry>,
    payload: &crate::usage_notify::UsageUpdate,
) -> tauri::Result<()> {
    app.emit(USAGE_UPDATE, payload)
}

pub fn emit_runtime_update(
    app: &impl Emitter<tauri::Wry>,
    payload: &crate::runtime_notify::RuntimeUpdate,
) -> tauri::Result<()> {
    app.emit(RUNTIME_UPDATE, payload)
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_name_constants_match_expected_values() {
        assert_eq!(CHAT_MESSAGE, "chat-message");
        assert_eq!(CHAT_DELTA, "chat-delta");
        assert_eq!(CHAT_COMPLETE, "chat-complete");
        assert_eq!(DEPT_LOG, "dept-log");
        assert_eq!(DEPT_STEP, "dept-step");
        assert_eq!(PLAN_UPDATE, "plan-update");
        assert_eq!(PROJECT_UPDATE, "project-update");
        assert_eq!(USAGE_UPDATE, "usage-update");
        assert_eq!(RUNTIME_UPDATE, "runtime-update");
    }

    #[test]
    fn plan_update_parses_valid_json() {
        let json = r#"{"batches":[{"name":"a","goal":"do a","status":"done"}],"current":1,"complete":true}"#;
        let parsed = PlanUpdate::from_json_string(json).unwrap();
        assert_eq!(parsed.batches.len(), 1);
        assert_eq!(parsed.batches[0].status, PlanBatchStatus::Done);
        assert_eq!(parsed.current, 1);
        assert!(parsed.complete);
    }

    #[test]
    fn plan_update_parses_complete_false() {
        let json = r#"{"batches":[],"current":0,"complete":false}"#;
        let parsed = PlanUpdate::from_json_string(json).unwrap();
        assert!(parsed.batches.is_empty());
        assert!(!parsed.complete);
    }

    #[test]
    fn plan_update_rejects_null() {
        assert!(PlanUpdate::from_json_string("null").is_none());
    }

    #[test]
    fn plan_update_rejects_malformed() {
        assert!(PlanUpdate::from_json_string("not json").is_none());
    }

    #[test]
    fn plan_update_round_trips() {
        let update = PlanUpdate {
            batches: vec![
                PlanBatchEvent {
                    name: "batch 1".into(),
                    goal: "do thing".into(),
                    status: PlanBatchStatus::Current,
                },
                PlanBatchEvent {
                    name: "batch 2".into(),
                    goal: "do other".into(),
                    status: PlanBatchStatus::Pending,
                },
            ],
            current: 1,
            complete: false,
        };
        let json = serde_json::to_string(&update).unwrap();
        let reparsed = PlanUpdate::from_json_string(&json).unwrap();
        assert_eq!(reparsed, update);
    }

    #[test]
    fn plan_batch_status_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&PlanBatchStatus::Done).unwrap(),
            "\"done\""
        );
        assert_eq!(
            serde_json::to_string(&PlanBatchStatus::Current).unwrap(),
            "\"current\""
        );
        assert_eq!(
            serde_json::to_string(&PlanBatchStatus::Pending).unwrap(),
            "\"pending\""
        );
    }
}
