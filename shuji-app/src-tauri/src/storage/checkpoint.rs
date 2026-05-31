use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::api::session::SessionSnapshot;

/// A single checkpoint entry stored in index.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointEntry {
    pub ts: String,
    pub role: String,
    pub description: String,
    pub commit: String,
}

/// Full checkpoint data saved to disk (session + metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckpointData {
    ts: String,
    role: String,
    description: String,
    commit: String,
    session: Vec<serde_json::Value>,
}

/// Save a checkpoint: git commit + persist session snapshot + update index.
/// Returns the commit hash, or None if there was nothing to commit.
pub async fn save(
    working_dir: &Path,
    role: &str,
    description: &str,
    session: &SessionSnapshot,
) -> Option<String> {
    // 1. git add -A && git commit
    let commit_hash = git_checkpoint(working_dir, role, description).await?;

    // 2. Write session snapshot to disk
    let snapshot_path = working_dir
        .join(".shuji")
        .join("checkpoints")
        .join(role)
        .join(format!("{}.json", commit_hash));

    let data = CheckpointData {
        ts: chrono::Local::now().to_rfc3339(),
        role: role.to_string(),
        description: description.to_string(),
        commit: commit_hash.clone(),
        session: session.messages.clone(),
    };

    if let Some(parent) = snapshot_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        let _ = tokio::fs::write(&snapshot_path, json).await;
    }

    // 3. Append to index
    let entry = CheckpointEntry {
        ts: data.ts,
        role: role.to_string(),
        description: description.to_string(),
        commit: commit_hash.clone(),
    };
    append_index(working_dir, &entry).await;

    log_console!(
        "[checkpoint] saved: {} — {} (commit {})",
        role,
        description,
        &commit_hash[..8]
    );

    Some(commit_hash)
}

/// Read the checkpoint index.
pub async fn load_index(working_dir: &Path) -> Vec<CheckpointEntry> {
    let path = working_dir
        .join(".shuji")
        .join("checkpoints")
        .join("index.json");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    if content.is_empty() {
        return vec![];
    }
    serde_json::from_str(&content).ok().unwrap_or_default()
}

// ── Internal helpers ─────────────────────────────────────

async fn git_checkpoint(working_dir: &Path, role: &str, description: &str) -> Option<String> {
    // git add -A
    let add = tokio::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(working_dir)
        .output()
        .await
        .ok()?;
    if !add.status.success() {
        log_console!(
            "[checkpoint] git add failed: {}",
            String::from_utf8_lossy(&add.stderr)
        );
        return None;
    }

    // git diff-index --cached --quiet HEAD → 0 means nothing changed
    let diff = tokio::process::Command::new("git")
        .args(["diff-index", "--cached", "--quiet", "HEAD"])
        .current_dir(working_dir)
        .output()
        .await
        .ok()?;
    if diff.status.success() {
        return None; // nothing to commit
    }

    // git commit
    let msg = format!("shuji: checkpoint {} — {}", role, description);
    let commit = tokio::process::Command::new("git")
        .args(["commit", "-m", &msg])
        .current_dir(working_dir)
        .output()
        .await
        .ok()?;
    if !commit.status.success() {
        log_console!(
            "[checkpoint] git commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
        return None;
    }

    // git rev-parse HEAD
    let rev = tokio::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(working_dir)
        .output()
        .await
        .ok()?;
    let hash = String::from_utf8_lossy(&rev.stdout).trim().to_string();
    if hash.is_empty() {
        return None;
    }
    Some(hash)
}

async fn append_index(working_dir: &Path, entry: &CheckpointEntry) {
    let path = working_dir
        .join(".shuji")
        .join("checkpoints")
        .join("index.json");
    let mut entries = load_index(working_dir).await;

    // Cap index to last 500 entries to keep it manageable
    if entries.len() >= 500 {
        entries.remove(0);
    }
    entries.push(entry.clone());

    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        let _ = tokio::fs::write(&path, json).await;
    }
}
