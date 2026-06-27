use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::api::session::SessionSnapshot;

/// Semantic checkpoint kinds (audit anchor points).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
    BeforeApproval,
    AfterApproval,
    BeforeExecution,
    DeliveryComplete,
    /// Periodic or actor-end snapshots — hidden in default UI.
    WorkspaceOnly,
}

impl CheckpointKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BeforeApproval => "before_approval",
            Self::AfterApproval => "after_approval",
            Self::BeforeExecution => "before_execution",
            Self::DeliveryComplete => "delivery_complete",
            Self::WorkspaceOnly => "workspace_only",
        }
    }
}

/// Optional metadata linking a checkpoint to pipeline / document context.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckpointMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A single checkpoint entry stored in index.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointEntry {
    pub ts: String,
    pub role: String,
    pub description: String,
    pub commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Full checkpoint data saved to disk (session + metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckpointData {
    ts: String,
    role: String,
    description: String,
    commit: String,
    session: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    doc_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn entry_from_data(data: &CheckpointData, commit: &str) -> CheckpointEntry {
    CheckpointEntry {
        ts: data.ts.clone(),
        role: data.role.clone(),
        description: data.description.clone(),
        commit: commit.to_string(),
        kind: data.kind.clone(),
        run_id: data.run_id.clone(),
        step_id: data.step_id.clone(),
        doc_id: data.doc_id.clone(),
        reason: data.reason.clone(),
    }
}

async fn persist_checkpoint(
    working_dir: &Path,
    role: &str,
    description: &str,
    session: Vec<serde_json::Value>,
    meta: CheckpointMeta,
) -> Option<String> {
    let commit_hash = git_checkpoint(working_dir, role, description).await?;

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
        session,
        kind: meta.kind.clone(),
        run_id: meta.run_id.clone(),
        step_id: meta.step_id.clone(),
        doc_id: meta.doc_id.clone(),
        reason: meta.reason.clone(),
    };

    if let Some(parent) = snapshot_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        let _ = tokio::fs::write(&snapshot_path, json).await;
    }

    let entry = entry_from_data(&data, &commit_hash);
    append_index(working_dir, &entry).await;

    let kind_label = meta.kind.as_deref().unwrap_or("checkpoint");
    log_console!(
        "[checkpoint] {} saved: {} — {} (commit {})",
        kind_label,
        role,
        description,
        &commit_hash[..8]
    );

    crate::audit::append(
        working_dir,
        "checkpoint",
        role,
        meta.doc_id.as_deref().unwrap_or(""),
        &format!(
            "kind={}; commit={}",
            kind_label,
            &commit_hash[..8.min(commit_hash.len())]
        ),
    )
    .await;

    Some(commit_hash)
}

/// Save a periodic checkpoint (workspace_only).
pub async fn save(
    working_dir: &Path,
    role: &str,
    description: &str,
    session: &SessionSnapshot,
) -> Option<String> {
    persist_checkpoint(
        working_dir,
        role,
        description,
        session.messages.clone(),
        CheckpointMeta {
            kind: Some(CheckpointKind::WorkspaceOnly.as_str().into()),
            reason: Some("periodic".into()),
            ..Default::default()
        },
    )
    .await
}

/// Save a semantic audit-anchor checkpoint.
pub async fn save_semantic(
    working_dir: &Path,
    role: &str,
    description: &str,
    kind: CheckpointKind,
    meta: CheckpointMeta,
    session: Option<&SessionSnapshot>,
) -> Option<String> {
    let mut meta = meta;
    meta.kind = Some(kind.as_str().into());
    let messages = session.map(|s| s.messages.clone()).unwrap_or_default();
    persist_checkpoint(working_dir, role, description, messages, meta).await
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

pub fn git_cmd(working_dir: &Path) -> tokio::process::Command {
    let git_dir = working_dir.join(".shuji/.git");
    let git_dir_str = git_dir.to_string_lossy().to_string();
    let work_tree = working_dir.to_string_lossy().to_string();
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(["--git-dir", &git_dir_str, "--work-tree", &work_tree]);
    cmd.current_dir(working_dir);
    cmd
}

async fn git_checkpoint(working_dir: &Path, role: &str, description: &str) -> Option<String> {
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

    let diff = git_cmd(working_dir)
        .args(["diff-index", "--cached", "--quiet", "HEAD"])
        .output()
        .await
        .ok()?;
    if diff.status.success() {
        return None;
    }

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

/// Deprecated: actor-end snapshots are no longer saved by default.
#[allow(dead_code)]
pub async fn save_final(working_dir: &Path, role: &str, description: &str) -> Option<String> {
    save_semantic(
        working_dir,
        role,
        description,
        CheckpointKind::WorkspaceOnly,
        CheckpointMeta {
            reason: Some("actor_final".into()),
            ..Default::default()
        },
        None,
    )
    .await
}

async fn append_index(working_dir: &Path, entry: &CheckpointEntry) {
    let path = working_dir
        .join(".shuji")
        .join("checkpoints")
        .join("index.json");
    let mut entries = load_index(working_dir).await;

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
