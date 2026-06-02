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

    crate::audit::append(
        working_dir,
        "checkpoint",
        role,
        "",
        &format!("commit={}", &commit_hash[..8]),
    )
    .await;

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

/// Find a checkpoint entry in the index by commit hash.
/// Returns (role_name, entry) if found.
pub async fn find_checkpoint(
    working_dir: &Path,
    commit_hash: &str,
) -> Option<(String, CheckpointEntry)> {
    let entries = load_index(working_dir).await;
    entries.into_iter().find_map(|e| {
        if e.commit == commit_hash {
            Some((e.role.clone(), e))
        } else {
            None
        }
    })
}

/// Load the session snapshot for a specific checkpoint.
pub async fn load_snapshot(
    working_dir: &Path,
    role: &str,
    commit_hash: &str,
) -> Option<SessionSnapshot> {
    let snapshot_path = working_dir
        .join(".shuji")
        .join("checkpoints")
        .join(role)
        .join(format!("{}.json", commit_hash));
    let content = tokio::fs::read_to_string(&snapshot_path).await.ok()?;
    let data: CheckpointData = serde_json::from_str(&content).ok()?;
    Some(SessionSnapshot::from_messages(data.session))
}

// ── Git helpers for isolated .shuji/.git repo ─────────────────

/// Build a git Command pre-configured with --git-dir and --work-tree
/// pointing to the isolated `.shuji/.git` repo with project root as worktree.
fn git_cmd(working_dir: &Path) -> tokio::process::Command {
    let git_dir = working_dir.join(".shuji/.git");
    let git_dir_str = git_dir.to_string_lossy().to_string();
    let work_tree = working_dir.to_string_lossy().to_string();
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(["--git-dir", &git_dir_str, "--work-tree", &work_tree]);
    cmd.current_dir(working_dir);
    cmd
}

// ── Internal helpers ─────────────────────────────────────

async fn git_checkpoint(working_dir: &Path, role: &str, description: &str) -> Option<String> {
    // git add -A
    let add = git_cmd(working_dir)
        .args(["add", "-A"])
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
    let diff = git_cmd(working_dir)
        .args(["diff-index", "--cached", "--quiet", "HEAD"])
        .output()
        .await
        .ok()?;
    if diff.status.success() {
        return None; // nothing to commit
    }

    // git commit
    let msg = format!("shuji: checkpoint {} — {}", role, description);
    let commit = git_cmd(working_dir)
        .args(["commit", "-m", &msg])
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
    let rev = git_cmd(working_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .await
        .ok()?;
    let hash = String::from_utf8_lossy(&rev.stdout).trim().to_string();
    if hash.is_empty() {
        return None;
    }
    Some(hash)
}

/// Save a final checkpoint after agent execution completes.
/// Unlike periodic checkpoints, this uses an empty session snapshot
/// to ensure at least one checkpoint exists even for short runs.
pub async fn save_final(
    working_dir: &Path,
    role: &str,
    description: &str,
) -> Option<String> {
    let commit_hash = git_checkpoint(working_dir, role, description).await?;

    // Write snapshot with empty session (no session context available at actor level)
    let snapshot_path = working_dir
        .join(".shuji")
        .join("checkpoints")
        .join(role)
        .join(format!("{}.json", commit_hash));
    let ts = chrono::Local::now().to_rfc3339();
    let data = CheckpointData {
        ts: ts.clone(),
        role: role.to_string(),
        description: description.to_string(),
        commit: commit_hash.clone(),
        session: vec![],
    };
    if let Some(parent) = snapshot_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        let _ = tokio::fs::write(&snapshot_path, json).await;
    }

    let entry = CheckpointEntry {
        ts,
        role: role.to_string(),
        description: description.to_string(),
        commit: commit_hash.clone(),
    };
    append_index(working_dir, &entry).await;

    log_console!(
        "[checkpoint] final saved: {} — {} (commit {})",
        role,
        description,
        &commit_hash[..8]
    );

    crate::audit::append(
        working_dir,
        "checkpoint",
        role,
        "",
        &format!("commit={}", &commit_hash[..8]),
    )
    .await;

    Some(commit_hash)
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
