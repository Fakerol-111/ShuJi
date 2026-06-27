//! Unified runtime-update notifications for the frontend cockpit.
//!
//! Emitted as Tauri `runtime-update` events when active roles, round metrics,
//! or other live workflow state changes. Frontend hooks subscribe to reduce polling.

use std::sync::Mutex;

use serde::Serialize;
use tokio::sync::mpsc;

use crate::round_metrics::RoundMetricState;

/// Snapshot pushed to the frontend on each runtime change.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeUpdate {
    pub active_roles: Vec<String>,
    pub round_metrics: Option<RoundMetricState>,
    /// Lightweight trigger hint for frontend diagnostics.
    pub trigger: String,
}

static SENDER: Mutex<Option<mpsc::UnboundedSender<RuntimeUpdate>>> = Mutex::new(None);

pub fn set_sender(tx: mpsc::UnboundedSender<RuntimeUpdate>) {
    if let Ok(mut guard) = SENDER.lock() {
        *guard = Some(tx);
    }
}

/// Build and enqueue a runtime snapshot.
pub fn notify(trigger: &str) {
    let update = RuntimeUpdate {
        active_roles: crate::round_metrics::get_active_roles(),
        round_metrics: crate::round_metrics::snapshot(),
        trigger: trigger.to_string(),
    };
    if let Ok(guard) = SENDER.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(update);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_update_serializes() {
        let update = RuntimeUpdate {
            active_roles: vec!["Neige".into()],
            round_metrics: None,
            trigger: "test".into(),
        };
        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("active_roles"));
        assert!(json.contains("Neige"));
    }
}
