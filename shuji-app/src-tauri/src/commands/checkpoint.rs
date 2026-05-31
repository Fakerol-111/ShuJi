use std::path::Path;

use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::storage::checkpoint::{CheckpointEntry, find_checkpoint, load_index, load_snapshot};

/// List all checkpoints in the current project.
/// Optionally filter by role, limit the number of results.
#[tauri::command]
pub async fn list_checkpoints(
    state: tauri::State<'_, AppState>,
    role: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<CheckpointEntry>, String> {
    let dir = state.current_dir.lock().await.clone().ok_or("没有打开的项目")?;
    let dir_path = Path::new(&dir);
    let mut entries = load_index(dir_path).await;
    // 按时间倒序（最新的在前）
    entries.reverse();

    if let Some(r) = role {
        entries.retain(|e| e.role == r);
    }

    if let Some(l) = limit {
        entries.truncate(l);
    }

    Ok(entries)
}

/// Restore a checkpoint: stash uncommitted changes → git checkout → load snapshot.
/// Returns the commit hash and the session snapshot messages.
#[tauri::command]
pub async fn restore_checkpoint(
    state: tauri::State<'_, AppState>,
    commit_hash: String,
) -> Result<String, String> {
    let dir = {
        let guard = state.current_dir.lock().await;
        guard.clone().ok_or("没有打开的项目")?
    };
    let dir_path = Path::new(&dir);

    // 1. Find the checkpoint in the index
    let (_role, _entry) = find_checkpoint(dir_path, &commit_hash)
        .await
        .ok_or_else(|| format!("未找到 checkpoint: {}", commit_hash))?;

    // 2. Load the session snapshot (before git checkout)
    let _snapshot = load_snapshot(dir_path, &_role, &commit_hash)
        .await
        .ok_or_else(|| format!("无法加载 checkpoint 快照: {}", commit_hash))?;

    // 3. Check for uncommitted changes
    let status = tokio::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(friendly_error)?;

    let has_changes = !status.stdout.is_empty();
    if has_changes {
        // Stash uncommitted changes
        let stash_msg = format!("shuji: before restore {}", &commit_hash[..8]);
        let stash = tokio::process::Command::new("git")
            .args(["stash", "push", "-m", &stash_msg])
            .current_dir(dir_path)
            .output()
            .await
            .map_err(friendly_error)?;
        if !stash.status.success() {
            return Err(format!("暂存变更失败: {}", String::from_utf8_lossy(&stash.stderr)));
        }
    }

    // 4. git checkout (detached HEAD)
    let checkout = tokio::process::Command::new("git")
        .args(["checkout", "--detach", &commit_hash])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(friendly_error)?;
    if !checkout.status.success() {
        return Err(format!("恢复失败: {}", String::from_utf8_lossy(&checkout.stderr)));
    }

    let summary = if has_changes {
        format!(
            "已恢复到 {}（detached HEAD）。未提交的变更已暂存，可通过 git stash pop 取回。",
            &commit_hash[..8]
        )
    } else {
        format!("已恢复到 {}（detached HEAD）。", &commit_hash[..8])
    };

    Ok(summary)
}
