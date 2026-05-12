#![allow(dead_code)]
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

use crate::models::role::Role;

/// Global mutex for console output serialization across all tokio actors.
/// Prevents interleaved `eprintln!` from concurrent tasks.
pub static CONSOLE_LOCK: Mutex<()> = Mutex::new(());

/// Single-file activity log. All departments append to the same file,
/// so entries are naturally chronological. Entry format:
/// `{"ts":"...","author":"...","summary":"..."}`
///
/// Records two types of events:
/// 1. Agent actions (a department completed something)
/// 2. Routing events (a department routed to another)
#[derive(Debug)]
pub struct Logger {
    log_path: PathBuf,
}

impl Logger {
    pub fn new(shuji_root: &PathBuf) -> Self {
        Self {
            log_path: shuji_root.join("logs").join("activity.log"),
        }
    }

    /// Append a log entry. Thread-safe via tokio append.
    async fn append(&self, author: &str, summary: &str) {
        let entry = serde_json::json!({
            "ts": chrono::Local::now().to_rfc3339(),
            "author": author,
            "summary": summary,
        });
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .await
        {
            let _ = file.write_all(entry.to_string().as_bytes()).await;
            let _ = file.write_all(b"\n").await;
        }
    }

    /// Log a state transition.
    pub async fn log_transition(&self, summary: &str) {
        self.append("系统", summary).await;
    }

    /// Log an agent execution result.
    pub async fn log_agent(&self, role: Role, summary: &str) {
        self.append(role.name(), summary).await;
    }

    /// Log a cross-department routing event.
    pub async fn log_route(&self, from: &str, to: &str, subject: &str) {
        self.append(from, &format!("路由到 {}: {}", to, subject)).await;
    }
}
