//! Tests for the role-based soul / learning system.

mod common;

use std::sync::Mutex;

use shuji_app_lib::api::session::PersistedContext;
use shuji_app_lib::learning::entry::LearningEntry;
use shuji_app_lib::learning::{
    normalize_role_name, set_test_home_dir, LearningKind, LearningScope, SoulStore,
    MAX_ENTRY_CHARS, PROJECT_INJECT_BUDGET,
};

/// Global soul tests mutate process-wide home override — serialize them only.
static GLOBAL_HOME_TEST_LOCK: Mutex<()> = Mutex::new(());

struct TestHomeGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    _home: tempfile::TempDir,
}

impl TestHomeGuard {
    fn new() -> Self {
        let lock = GLOBAL_HOME_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let home = tempfile::tempdir().expect("temp home");
        set_test_home_dir(Some(home.path().to_path_buf()));
        Self {
            _lock: lock,
            _home: home,
        }
    }
}

impl Drop for TestHomeGuard {
    fn drop(&mut self) {
        set_test_home_dir(None);
    }
}

#[tokio::test]
async fn new_path_preferred_over_legacy_neige() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    let legacy = wd.join(".shuji").join("soul").join("neige.md");
    tokio::fs::create_dir_all(legacy.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&legacy, "legacy soul content")
        .await
        .unwrap();

    let canonical = SoulStore::project_soul_path(wd, "Neige");
    tokio::fs::write(&canonical, "canonical soul content")
        .await
        .unwrap();

    let loaded = SoulStore::load_project_markdown(wd, "Neige").await;
    assert_eq!(loaded, "canonical soul content");
}

#[tokio::test]
async fn legacy_neige_path_is_readable() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    let legacy = wd.join(".shuji").join("soul").join("neige.md");
    tokio::fs::create_dir_all(legacy.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&legacy, "legacy only").await.unwrap();

    let loaded = SoulStore::load_project_markdown(wd, "Neige").await;
    assert_eq!(loaded, "legacy only");

    let migrated = SoulStore::project_soul_path(wd, "Neige");
    assert!(migrated.exists());
}

#[tokio::test]
async fn chinese_section_maps_to_english_heading() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    SoulStore::append_entry(
        wd,
        "Neige",
        LearningKind::from_section_or_kind(Some("经验"), None).unwrap(),
        LearningScope::Project,
        "test experience",
        vec![],
        vec![],
    )
    .await
    .unwrap();

    let content = SoulStore::read_project_soul(wd, "Neige").await;
    assert!(content.contains("## Experience"));
    assert!(content.contains("- test experience"));
}

#[tokio::test]
async fn update_soul_entry_length_limit() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();
    let long_content = "x".repeat(MAX_ENTRY_CHARS + 1);

    let err = SoulStore::append_entry(
        wd,
        "Neige",
        LearningKind::Experience,
        LearningScope::Project,
        &long_content,
        vec![],
        vec![],
    )
    .await
    .unwrap_err();

    assert!(err.contains("500") || err.contains(&MAX_ENTRY_CHARS.to_string()));
}

#[tokio::test]
async fn direct_global_write_rejected() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    let err = SoulStore::append_entry(
        wd,
        "Neige",
        LearningKind::Preference,
        LearningScope::Global,
        "should fail",
        vec![],
        vec![],
    )
    .await
    .unwrap_err();

    assert!(err.contains("global_candidate") || err.contains("not allowed"));
}

#[tokio::test]
async fn global_candidate_requires_evidence() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    let err = SoulStore::append_entry(
        wd,
        "Neige",
        LearningKind::Preference,
        LearningScope::GlobalCandidate,
        "candidate without evidence",
        vec![],
        vec![],
    )
    .await
    .unwrap_err();

    assert!(err.contains("evidence"));
}

#[tokio::test]
async fn global_candidate_approve_moves_to_global_soul() {
    let _home = TestHomeGuard::new();
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    SoulStore::append_entry(
        wd,
        "Neige",
        LearningKind::Preference,
        LearningScope::GlobalCandidate,
        "likes concise reports",
        vec!["user:explicit".into()],
        vec![],
    )
    .await
    .unwrap();

    let candidates = SoulStore::list_global_candidates().await.unwrap();
    assert_eq!(candidates.len(), 1);
    let id = candidates[0].id.clone();

    SoulStore::approve_global_candidate(&id).await.unwrap();

    let remaining = SoulStore::list_global_candidates().await.unwrap();
    assert!(remaining.is_empty());

    let global = SoulStore::load_global_markdown("Neige")
        .await
        .unwrap_or_default();
    assert!(global.contains("likes concise reports"));
}

#[test]
fn persisted_context_keeps_base_soul_context_order() {
    let ctx = PersistedContext {
        base_prompt: "base".into(),
        soul_prompt: Some("[soul: Neige]\nsoul body".into()),
        context_messages: vec![serde_json::json!({"role": "user", "content": "task"})],
    };
    let msgs = ctx.to_messages();
    assert_eq!(msgs[0]["content"], "base");
    assert_eq!(msgs[1]["content"], "[soul: Neige]\nsoul body");
    assert_eq!(msgs[2]["content"], "task");
}

#[test]
fn refreshed_soul_overrides_stale_persisted_soul() {
    let ctx = PersistedContext {
        base_prompt: "base".into(),
        soul_prompt: Some("[soul: Neige]\nstale".into()),
        context_messages: vec![],
    };
    let refreshed = ctx.with_refreshed_soul("Neige", "fresh from disk");
    let msgs = refreshed.to_messages();
    assert_eq!(msgs[1]["content"], "[soul: Neige]\nfresh from disk");
}

#[tokio::test]
async fn injection_truncates_project_budget() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();
    let huge = "a".repeat(PROJECT_INJECT_BUDGET + 500);
    let path = SoulStore::project_soul_path(wd, "Gongbushangshu");
    tokio::fs::create_dir_all(path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&path, &huge).await.unwrap();

    let injected = SoulStore::load_for_injection(wd, "Gongbushangshu", false)
        .await
        .unwrap();
    assert!(injected.len() <= PROJECT_INJECT_BUDGET);
}

#[tokio::test]
async fn injection_preserves_global_when_project_is_huge() {
    let _home = TestHomeGuard::new();
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    let project_path = SoulStore::project_soul_path(wd, "Neige");
    tokio::fs::create_dir_all(project_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&project_path, "p".repeat(5000))
        .await
        .unwrap();

    let global_path = SoulStore::global_soul_path("Neige").unwrap();
    tokio::fs::create_dir_all(global_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(
        &global_path,
        "## Preferences\n\n- GLOBAL_MARKER_PREFERENCE\n",
    )
    .await
    .unwrap();

    let injected = SoulStore::load_for_injection(wd, "Neige", true)
        .await
        .unwrap();
    assert!(injected.contains("GLOBAL_MARKER_PREFERENCE"));
}

#[tokio::test]
async fn dedupe_updates_index_not_duplicate_lines() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    SoulStore::append_entry(
        wd,
        "Neige",
        LearningKind::Lesson,
        LearningScope::Project,
        "same lesson",
        vec![],
        vec![],
    )
    .await
    .unwrap();
    SoulStore::append_entry(
        wd,
        "Neige",
        LearningKind::Lesson,
        LearningScope::Project,
        "same lesson",
        vec![],
        vec![],
    )
    .await
    .unwrap();

    let index_path = SoulStore::project_index_path(wd);
    let index = tokio::fs::read_to_string(&index_path).await.unwrap();
    let lines: Vec<_> = index.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 1);

    let entry: LearningEntry = serde_json::from_str(lines[0]).unwrap();
    assert!(entry.confidence >= 0.75);

    let markdown = SoulStore::read_project_soul(wd, "Neige").await;
    assert_eq!(markdown.matches("- same lesson").count(), 1);
}

#[test]
fn normalize_role_rejects_path_traversal() {
    assert!(normalize_role_name(Some("../../escape")).is_err());
}

#[test]
fn normalize_role_maps_chinese_name() {
    assert_eq!(normalize_role_name(Some("工部")).unwrap(), "Gongbushangshu");
}

#[tokio::test]
async fn append_rejects_unknown_role() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();
    let err = SoulStore::append_entry(
        wd,
        "../../escape",
        LearningKind::Lesson,
        LearningScope::Project,
        "bad role",
        vec![],
        vec![],
    )
    .await
    .unwrap_err();
    assert!(err.contains("Unknown role"));
}

#[tokio::test]
async fn approve_global_candidate_dedupes_markdown() {
    let _home = TestHomeGuard::new();
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    for _ in 0..2 {
        SoulStore::append_entry(
            wd,
            "Neige",
            LearningKind::Preference,
            LearningScope::GlobalCandidate,
            "shared preference",
            vec!["evidence:1".into()],
            vec![],
        )
        .await
        .unwrap();
    }

    let candidates = SoulStore::list_global_candidates().await.unwrap();
    assert_eq!(candidates.len(), 2);

    SoulStore::approve_global_candidate(&candidates[0].id.clone())
        .await
        .unwrap();
    SoulStore::approve_global_candidate(&candidates[1].id.clone())
        .await
        .unwrap();

    let global = SoulStore::load_global_markdown("Neige")
        .await
        .unwrap_or_default();
    assert_eq!(global.matches("- shared preference").count(), 1);
}

#[test]
fn list_soul_roles_returns_nine_actors() {
    let roles = SoulStore::list_soul_roles();
    assert_eq!(roles.len(), 9);
    assert!(roles.contains(&"Neige".to_string()));
    assert!(roles.contains(&"Gongbushangshu".to_string()));
}

#[tokio::test]
async fn pipeline_extract_records_validation_failures() {
    let dir = common::create_test_project("learning");
    let wd = dir.path();

    let validate_dir = wd.join(".shuji").join("validate");
    tokio::fs::create_dir_all(&validate_dir).await.unwrap();
    let report = serde_json::json!({
        "ts": "2026-01-01T00:00:00Z",
        "project_type": "rust",
        "overall_pass": false,
        "checks": [{
            "name": "tests",
            "pass": false,
            "summary": "cargo test failed on auth module",
            "details": {}
        }],
        "ctrt_id": null
    });
    tokio::fs::write(
        validate_dir.join("latest.json"),
        serde_json::to_string(&report).unwrap(),
    )
    .await
    .unwrap();

    let count = shuji_app_lib::learning::LearningExtractor::from_pipeline_complete(wd)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let soul = SoulStore::read_project_soul(wd, "Xingbushangshu").await;
    assert!(soul.contains("cargo test failed"));
}
