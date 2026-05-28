#![allow(dead_code)]
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::models::role::Role;

/// Global channel for serialized console output.
/// Lazily initialized on first `log_console!` call — after the tokio runtime is active.
static CONSOLE_TX: OnceLock<mpsc::UnboundedSender<String>> = OnceLock::new();
/// Prevent multiple concurrent initializations.
static INIT_LOCK: Mutex<()> = Mutex::new(());

/// Ensure the console writer task is running. Safe to call multiple times.
fn ensure_console_writer() {
    let _guard = INIT_LOCK.lock().unwrap();
    if CONSOLE_TX.get().is_some() {
        return;
    }
    // Try to get a tokio runtime handle. If we're inside a runtime, spawn the writer.
    // If not, fall back to direct stderr writes via the sender (which will be a no-op).
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        CONSOLE_TX.set(tx).ok();
        handle.spawn(async move {
            use std::io::Write;
            while let Some(line) = rx.recv().await {
                let _ = writeln!(std::io::stderr(), "{}", line);
            }
        });
    }
}

/// Send a line to the console writer (non-blocking, lock-free).
/// Falls back to direct stderr if the writer hasn't started yet.
pub fn console_send(line: String) {
    ensure_console_writer();
    if let Some(tx) = CONSOLE_TX.get() {
        let _ = tx.send(line);
    } else {
        // Fallback: write directly if tokio runtime isn't available yet
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "{}", line);
    }
}

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
    /// Rotates the log file if it exceeds 10 MB.
    async fn append(&self, author: &str, summary: &str) {
        // Rotate if log file exceeds 10 MB
        const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;
        if let Ok(meta) = tokio::fs::metadata(&self.log_path).await {
            if meta.len() > MAX_LOG_SIZE {
                let archive = self.log_path.with_file_name(format!(
                    "activity-{}.log",
                    chrono::Local::now().format("%Y%m%d_%H%M%S")
                ));
                let _ = tokio::fs::rename(&self.log_path, &archive).await;
            }
        }

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
