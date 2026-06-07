use std::path::Path;

use crate::api::session::PersistedContext;
use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::storage::checkpoint::{find_checkpoint, git_cmd, load_index, load_snapshot, CheckpointEntry};

/// List all checkpoints in the current project.
/// Optionally filter by role, limit the number of results.
#[tauri::command]
pub async fn list_checkpoints(
    state: tauri::State<'_, AppState>,
    role: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<CheckpointEntry>, String> {
    let dir = state
        .current_dir
        .lock()
        .await
        .clone()
        .ok_or("没有打开的项目")?;
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
    let (role, _entry) = find_checkpoint(dir_path, &commit_hash)
        .await
        .ok_or_else(|| format!("未找到 checkpoint: {}", commit_hash))?;

    // 2. Load the session snapshot (before git checkout, so we can write it back after)
    let snapshot = load_snapshot(dir_path, &role, &commit_hash)
        .await
        .ok_or_else(|| format!("无法加载 checkpoint 快照: {}", commit_hash))?;

    // 3. Check for uncommitted changes in the isolated .shuji/.git repo
    let status = git_cmd(dir_path)
        .args(["status", "--porcelain"])
        .output()
        .await
        .map_err(friendly_error)?;

    let has_changes = !status.stdout.is_empty();
    if has_changes {
        // Stash uncommitted changes
        let stash_msg = format!("shuji: before restore {}", &commit_hash[..8]);
        let stash = git_cmd(dir_path)
            .args(["stash", "push", "-m", &stash_msg])
            .output()
            .await
            .map_err(friendly_error)?;
        if !stash.status.success() {
            return Err(format!(
                "暂存变更失败: {}",
                String::from_utf8_lossy(&stash.stderr)
            ));
        }
    }

    // 4. git checkout (detached HEAD) using the isolated .shuji/.git repo
    let checkout = git_cmd(dir_path)
        .args(["checkout", "--detach", &commit_hash])
        .output()
        .await
        .map_err(friendly_error)?;
    if !checkout.status.success() {
        return Err(format!(
            "恢复失败: {}",
            String::from_utf8_lossy(&checkout.stderr)
        ));
    }

    // 5. Write session snapshot back to .shuji/context/{role}.json
    //    so the agent can continue from the checkpoint state.
    let ctx = PersistedContext {
        base_prompt: String::new(),
        soul_prompt: None,
        context_messages: snapshot.messages.clone(),
    };
    ctx.save_to(dir_path, &role).await;

    let summary = if has_changes {
        format!(
            "已恢复到 {}（detached HEAD）。上下文已回滚，未提交的变更已暂存（git stash pop 可恢复）。",
            &commit_hash[..8]
        )
    } else {
        format!(
            "已恢复到 {}（detached HEAD）。上下文已回滚。",
            &commit_hash[..8]
        )
    };

    Ok(summary)
}
