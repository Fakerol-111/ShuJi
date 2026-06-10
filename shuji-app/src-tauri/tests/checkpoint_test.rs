//! Checkpoint 模块集成测试
//!
//! 覆盖 checkpoint::save / load_index / find_checkpoint / load_snapshot / save_final
//!
//! 注意：checkpoint 依赖 .shuji/.git/ 隔离仓库。测试在 TempDir 中初始化，
//! 不影响项目的真实 git 仓库。
//!
//! 运行: cargo test --test checkpoint_test -- --nocapture

mod common;

use shuji_app_lib::api::session::SessionSnapshot;
use shuji_app_lib::storage::checkpoint::{self};
use shuji_app_lib::storage::shuji_dir::ShujiDir;
use std::path::Path;

// ── Helper: sync wrapper ──────────────────────────────────────

fn block_on<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

/// Initialize a temporary project with .shuji directory and git repo.
fn init_temp_project(name: &str) -> tempfile::TempDir {
    let dir = common::create_temp_dir(name);
    let s = ShujiDir::new(&dir.path().to_string_lossy());
    block_on(s.init()).expect("ShujiDir::init should succeed");
    dir
}

/// Manually commit all current files to the .shuji/.git repo.
/// After this, a checkpoint `save()` with no new changes should return None.
#[allow(dead_code)]
fn commit_all(working_dir: &Path) {
    let add = block_on(async {
        checkpoint::git_cmd(working_dir)
            .args(["add", "-A"])
            .output()
            .await
    });
    assert!(
        add.is_ok() && add.as_ref().unwrap().status.success(),
        "git add -A failed: {:?}",
        add.as_ref().map(|o| String::from_utf8_lossy(&o.stderr))
    );

    let commit = block_on(async {
        checkpoint::git_cmd(working_dir)
            .args(["commit", "-m", "test: commit all"])
            .output()
            .await
    });
    assert!(
        commit.is_ok() && commit.as_ref().unwrap().status.success(),
        "git commit failed: {:?}",
        commit.as_ref().map(|o| String::from_utf8_lossy(&o.stderr))
    );
}

/// Count the number of commits in the .shuji/.git repo.
#[allow(dead_code)]
fn count_commits(working_dir: &Path) -> usize {
    let out = block_on(async {
        checkpoint::git_cmd(working_dir)
            .args(["rev-list", "--count", "HEAD"])
            .output()
            .await
    });
    let stdout = out
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    stdout.parse().unwrap_or(0)
}

/// Create a simple SessionSnapshot for testing.
fn test_snapshot() -> SessionSnapshot {
    SessionSnapshot::from_messages(vec![
        serde_json::json!({"role": "user", "content": "Hello from test"}),
        serde_json::json!({"role": "assistant", "content": "Hello from ShuJi"}),
    ])
}

/// Create an empty SessionSnapshot.
fn empty_snapshot() -> SessionSnapshot {
    SessionSnapshot::from_messages(vec![])
}

// ═══════════════════════════════════════════════════════════════
// 1. save with no changes (returns None)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_save_no_changes() {
    let dir = init_temp_project("save_none");

    // After init, commit everything so the repo is clean
    commit_all(dir.path());

    // Now save() with a session should find nothing new to commit
    let snap = test_snapshot();
    let hash = block_on(checkpoint::save(
        dir.path(),
        "内阁",
        "test checkpoint with no changes",
        &snap,
    ));

    assert!(
        hash.is_none(),
        "save() should return None when there are no changes to commit"
    );
}

// ═══════════════════════════════════════════════════════════════
// 2. save + load_index + find_checkpoint round-trip
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_save_and_find_checkpoint() {
    let dir = init_temp_project("save_find");

    let snap = test_snapshot();

    // First save should find new .shuji files to commit → returns a hash
    let hash = block_on(checkpoint::save(dir.path(), "工部", "完成编码任务", &snap));
    assert!(hash.is_some(), "First save should produce a commit hash");
    let hash = hash.unwrap();
    assert!(!hash.is_empty(), "Commit hash should not be empty");

    // Load index and verify
    let index = block_on(checkpoint::load_index(dir.path()));
    assert_eq!(index.len(), 1, "Index should have one entry");
    assert_eq!(index[0].role, "工部");
    assert_eq!(index[0].description, "完成编码任务");
    assert_eq!(index[0].commit, hash);

    // Find checkpoint by hash
    let found = block_on(checkpoint::find_checkpoint(dir.path(), &hash));
    assert!(found.is_some(), "Should find checkpoint by commit hash");
    let (role, entry) = found.unwrap();
    assert_eq!(role, "工部");
    assert_eq!(entry.commit, hash);
    assert_eq!(entry.description, "完成编码任务");
}

// ═══════════════════════════════════════════════════════════════
// 3. load_snapshot — restore session from checkpoint
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_load_snapshot() {
    let dir = init_temp_project("snapshot");

    let snap = test_snapshot();
    let hash = block_on(checkpoint::save(dir.path(), "内阁", "snapshot test", &snap));
    assert!(hash.is_some(), "save() should succeed");
    let hash = hash.unwrap();

    // Find the checkpoint role
    let (role, _entry) = block_on(checkpoint::find_checkpoint(dir.path(), &hash))
        .expect("Checkpoint should be findable");

    // Load the snapshot
    let loaded = block_on(checkpoint::load_snapshot(dir.path(), &role, &hash));
    assert!(loaded.is_some(), "Should load session snapshot");

    // Verify messages were restored by checking the snapshot file on disk
    let snapshot_file = dir
        .path()
        .join(".shuji/checkpoints")
        .join(&role)
        .join(format!("{}.json", hash));
    let snapshot_content =
        std::fs::read_to_string(&snapshot_file).expect("Snapshot file should exist");
    assert!(
        snapshot_content.contains("Hello from test"),
        "Should contain original user message"
    );
    assert!(
        snapshot_content.contains("Hello from ShuJi"),
        "Should contain original assistant message"
    );
}

#[test]
fn test_load_snapshot_empty_session() {
    let dir = init_temp_project("snapshot_empty");

    let snap = empty_snapshot();
    let hash = block_on(checkpoint::save(
        dir.path(),
        "工部尚书",
        "empty snapshot",
        &snap,
    ));
    assert!(hash.is_some());
    let hash = hash.unwrap();

    let (role, _entry) =
        block_on(checkpoint::find_checkpoint(dir.path(), &hash)).expect("Should be findable");

    let loaded = block_on(checkpoint::load_snapshot(dir.path(), &role, &hash));
    assert!(loaded.is_some(), "Should load even with empty session");
}

// ═══════════════════════════════════════════════════════════════
// 4. save_final (actor-level checkpoint with empty session)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_save_final() {
    let dir = init_temp_project("final_ckpt");

    let hash = block_on(checkpoint::save_final(
        dir.path(),
        "兵部尚书",
        "测试执行完成",
    ));
    assert!(hash.is_some(), "save_final should return a commit hash");

    // Verify index has the entry
    let index = block_on(checkpoint::load_index(dir.path()));
    assert!(
        !index.is_empty(),
        "Index should contain the final checkpoint"
    );
    assert_eq!(index[0].role, "兵部尚书");
    assert_eq!(index[0].description, "测试执行完成");

    // Verify snapshot file exists (even if empty)
    let hash = hash.unwrap();
    let snapshot_path = dir
        .path()
        .join(".shuji/checkpoints/兵部尚书")
        .join(format!("{}.json", hash));
    assert!(snapshot_path.exists(), "Snapshot file should exist");
}

// ═══════════════════════════════════════════════════════════════
// 5. Multiple checkpoints and index ordering
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_multiple_checkpoints() {
    let dir = init_temp_project("multi_ckpt");

    // Save checkpoint 1
    let snap1 = SessionSnapshot::from_messages(vec![
        serde_json::json!({"role": "user", "content": "First"}),
    ]);
    let hash1 = block_on(checkpoint::save(dir.path(), "内阁", "first", &snap1));
    assert!(hash1.is_some());

    // Create a new file on disk so the next save has something to commit
    std::fs::write(dir.path().join("new_file.txt"), "some content").unwrap();

    // Save checkpoint 2
    let snap2 = SessionSnapshot::from_messages(vec![
        serde_json::json!({"role": "user", "content": "Second"}),
    ]);
    let hash2 = block_on(checkpoint::save(dir.path(), "工部", "second", &snap2));
    assert!(hash2.is_some());

    let index = block_on(checkpoint::load_index(dir.path()));
    assert_eq!(index.len(), 2, "Should have two index entries");
    assert_ne!(
        index[0].commit, index[1].commit,
        "Commits should be different"
    );

    // Both should be findable
    let found1 = block_on(checkpoint::find_checkpoint(dir.path(), &hash1.unwrap()));
    let found2 = block_on(checkpoint::find_checkpoint(dir.path(), &hash2.unwrap()));
    assert!(found1.is_some(), "First checkpoint should be findable");
    assert!(found2.is_some(), "Second checkpoint should be findable");
}

// ═══════════════════════════════════════════════════════════════
// 6. find_checkpoint returns None for nonexistent hash
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_find_checkpoint_nonexistent() {
    let dir = init_temp_project("find_nonexist");
    let found = block_on(checkpoint::find_checkpoint(
        dir.path(),
        "deadbeefdeadbeefdeadbeef",
    ));
    assert!(found.is_none(), "Should not find a nonexistent checkpoint");
}

// ═══════════════════════════════════════════════════════════════
// 7. load_index for empty/uninit project
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_load_index_empty() {
    let dir = common::create_temp_dir("empty_index");
    // No .shuji directory at all
    let index = block_on(checkpoint::load_index(dir.path()));
    assert!(index.is_empty(), "Index for empty project should be empty");
}
