//! Lightweight usage-update notifications for the frontend sidebar.
//!
//! Emitted as Tauri `usage-update` events when token usage is recorded or
//! persisted context is saved. The frontend coalesces bursts (e.g. parallel
//! agents) into a single refresh.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageUpdateKind {
    Token,
    Context,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageUpdate {
    pub role: String,
    pub kind: UsageUpdateKind,
}

static SENDER: Mutex<Option<mpsc::UnboundedSender<UsageUpdate>>> = Mutex::new(None);

/// Register the global forwarder installed at app startup.
pub fn set_sender(tx: mpsc::UnboundedSender<UsageUpdate>) {
    if let Ok(mut lock) = SENDER.lock() {
        *lock = Some(tx);
    }
}

/// Fire-and-forget notify. Safe to call from any thread; drops if unset.
pub fn notify(role: &str, kind: UsageUpdateKind) {
    let tx = match SENDER.lock() {
        Ok(lock) => lock.clone(),
        Err(_) => return,
    };
    if let Some(tx) = tx {
        let _ = tx.send(UsageUpdate {
            role: role.to_string(),
            kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_without_sender_does_not_panic() {
        let prev = SENDER.lock().ok().and_then(|mut l| l.take());
        notify("Zhongshuling", UsageUpdateKind::Token);
        if let Ok(mut lock) = SENDER.lock() {
            *lock = prev;
        }
    }
}
