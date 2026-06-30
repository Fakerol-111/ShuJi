//! 文档系统测试 + 朱批门禁测试
//!
//! 运行: cargo test --test document_test -- --nocapture

mod common;

use shuji_app_lib::tool::documents::{
    check_doc_refs_approved_for_route, get_first_pending_approval, tool_append_document,
    tool_create_document, tool_find_document, tool_modify_document, tool_set_document_status,
};
use std::path::Path;

/// Sync wrapper for async tool functions in tests.
fn block_on<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

/// Helper: read a .shuji document's frontmatter status field.
fn read_doc_status(root: &Path, doc_id: &str) -> Option<String> {
    // Determine the file path based on doc prefix
    let prefix = doc_id.split('_').next()?;
    let dir = match prefix {
        "plan" | "dsgn" => "designs",
        "revw" => "reviews",
        "ctrt" => "contracts",
        _ => "",
    };
    let path = root.join(".shuji").join(dir).join(format!("{}.md", doc_id));
    let content = std::fs::read_to_string(&path).ok()?;
    // Parse simple frontmatter to find status field
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("status: ") {
            return Some(val.trim().to_string());
        }
    }
    None
}

/// Helper: read a .shuji document's `notes` field from frontmatter.
fn read_doc_notes(root: &Path, doc_id: &str) -> Option<String> {
    let prefix = doc_id.split('_').next()?;
    let dir = match prefix {
        "plan" | "dsgn" => "designs",
        "revw" => "reviews",
        "ctrt" => "contracts",
        _ => "",
    };
    let path = root.join(".shuji").join(dir).join(format!("{}.md", doc_id));
    let content = std::fs::read_to_string(&path).ok()?;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("notes: ") {
            return Some(val.trim().to_string());
        }
    }
    None
}

/// Helper: read a .shuji document's `author` field from frontmatter.
fn read_doc_author(root: &Path, doc_id: &str) -> Option<String> {
    let prefix = doc_id.split('_').next()?;
    let path = if prefix == "rprt" {
        let reports_dir = root.join(".shuji/reports");
        std::fs::read_dir(&reports_dir)
            .ok()?
            .filter_map(|e| e.ok())
            .find_map(|entry| {
                let candidate = entry.path().join(format!("{doc_id}.md"));
                candidate.exists().then_some(candidate)
            })?
    } else {
        let dir = match prefix {
            "plan" | "dsgn" | "pdsg" => "designs",
            "revw" => "reviews",
            "ctrt" => "contracts",
            "task" => "tasks",
            "reqs" => "requirements",
            "anls" => "analysis",
            _ => "",
        };
        if dir.is_empty() {
            root.join(".shuji").join(format!("{doc_id}.md"))
        } else {
            root.join(".shuji").join(dir).join(format!("{doc_id}.md"))
        }
    };
    let content = std::fs::read_to_string(&path).ok()?;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("author: ") {
            return Some(val.trim().to_string());
        }
    }
    None
}

/// Helper: read pending_approvals.json as a Vec<String>.
fn read_pending_approvals(root: &Path) -> Vec<String> {
    let path = root.join(".shuji/pending_approvals.json");
    if !path.exists() {
        return vec![];
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&content).unwrap_or_default()
}

// ── create_document 测试 ──────────────────────────────────────

#[test]
fn test_create_document_design() {
    let temp = common::create_test_project("doc_create_design");
    let root = temp.path();

    // Create referenced docs first (counter starts at 0 → dsgn_0, dsgn_1, dsgn_2)
    for _ in 0..3 {
        let prep = serde_json::json!({"type": "dsgn", "refs": []});
        block_on(tool_create_document(root, &prep, "zhongshuling"));
    }

    let args = serde_json::json!({
        "type": "dsgn",
        "refs": [0, 2]
    });

    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["operation"], "create_document");

    // Verify the document was created
    let designs_dir = root.join(".shuji/designs");
    assert!(designs_dir.exists());

    // Check that at least one design file was created
    let entries: Vec<_> = std::fs::read_dir(&designs_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        !entries.is_empty(),
        "At least one design document should be created"
    );
}

#[test]
fn test_create_document_different_types() {
    let temp = common::create_test_project("doc_types");
    let root = temp.path();

    let types_and_dirs = vec![
        ("dsgn", ".shuji/designs"),
        ("plan", ".shuji/designs"),
        ("revw", ".shuji/reviews"),
        ("ctrt", ".shuji/contracts"),
    ];

    for (doc_type, expected_dir) in types_and_dirs {
        let args = serde_json::json!({"type": doc_type, "refs": []});
        let result = block_on(tool_create_document(root, &args, "test"));
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert_eq!(
            parsed["ok"], true,
            "Creating {} document should succeed",
            doc_type
        );

        // Verify directory exists
        let dir_path = root.join(expected_dir);
        assert!(dir_path.exists(), "Directory {} should exist", expected_dir);
    }
}

#[test]
fn test_create_document_invalid_type() {
    let temp = common::create_test_project("doc_invalid");
    let root = temp.path();

    let args = serde_json::json!({
        "type": "invalid_type",
        "refs": []
    });

    let result = block_on(tool_create_document(root, &args, "test"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["error_code"], "invalid_type");
}

// ── append_document 测试 ──────────────────────────────────────

#[test]
fn test_append_document_basic() {
    let temp = common::create_test_project("doc_append_basic");
    let root = temp.path();

    // Create a document first
    let create_args = serde_json::json!({"type": "dsgn", "refs": []});
    let create_result = block_on(tool_create_document(root, &create_args, "zhongshuling"));
    let create_parsed: serde_json::Value = serde_json::from_str(&create_result).unwrap();
    assert_eq!(create_parsed["ok"], true);

    // Extract document ID from path field
    if let Some(doc_id) = create_parsed["path"].as_str() {
        let append_args = serde_json::json!({
            "id": doc_id,
            "content": "## Test Content\n\nThis is a test."
        });

        let append_result = block_on(tool_append_document(root, &append_args, "zhongshuling"));
        let append_parsed: serde_json::Value = serde_json::from_str(&append_result).unwrap();

        assert_eq!(append_parsed["ok"], true);
    }
}

#[test]
fn test_append_document_not_found() {
    let temp = common::create_test_project("doc_append_notfound");
    let root = temp.path();

    let args = serde_json::json!({
        "id": "dsgn_999999",
        "content": "Test content"
    });

    let result = block_on(tool_append_document(root, &args, "test"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["ok"], false);
}

// ── modify_document 测试 ──────────────────────────────────────

#[test]
fn test_modify_document_basic() {
    let temp = common::create_test_project("doc_modify_basic");
    let root = temp.path();

    // Create and append content
    let create_args = serde_json::json!({"type": "dsgn", "refs": []});
    let create_result = block_on(tool_create_document(root, &create_args, "zhongshuling"));
    let create_parsed: serde_json::Value = serde_json::from_str(&create_result).unwrap();

    if let Some(doc_id) = create_parsed["path"].as_str() {
        // Append some content
        let append_args = serde_json::json!({
            "id": doc_id,
            "content": "Original text here"
        });
        block_on(tool_append_document(root, &append_args, "zhongshuling"));

        // Modify the content
        let modify_args = serde_json::json!({
            "id": doc_id,
            "old_text": "Original",
            "new_text": "Modified"
        });

        let modify_result = block_on(tool_modify_document(root, &modify_args, "zhongshuling"));
        let modify_parsed: serde_json::Value = serde_json::from_str(&modify_result).unwrap();

        assert_eq!(modify_parsed["ok"], true);
    }
}

// ── find_document 测试 ────────────────────────────────────────

#[test]
fn test_find_document_basic() {
    let temp = common::create_test_project("doc_find_basic");
    let root = temp.path();

    let create_args = serde_json::json!({"type": "dsgn", "refs": []});
    let create_result = block_on(tool_create_document(root, &create_args, "zhongshuling"));
    let create_parsed: serde_json::Value = serde_json::from_str(&create_result).unwrap();

    if let Some(doc_id) = create_parsed["path"].as_str() {
        let find_args = serde_json::json!({"id": doc_id});
        let find_result = block_on(tool_find_document(root, &find_args));
        let find_parsed: serde_json::Value = serde_json::from_str(&find_result).unwrap();

        assert_eq!(find_parsed["ok"], true);
        // The document content is returned in the "message" field
        assert!(find_parsed["message"].is_string() || find_parsed["content"].is_string());
    }
}

#[test]
fn test_find_document_not_found() {
    let temp = common::create_test_project("doc_find_notfound");
    let root = temp.path();

    let args = serde_json::json!({"id": "dsgn_999999"});
    let result = block_on(tool_find_document(root, &args));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["ok"], false);
}

// ═══════════════════════════════════════════════════════════════
// 朱批与门禁测试 (P0-2)
// ═══════════════════════════════════════════════════════════════

/// 1. plan 文档创建后不进入 in_review 状态
#[test]
fn test_plan_not_in_review() {
    let temp = common::create_test_project("plan_not_in_review");
    let root = temp.path();

    let args = serde_json::json!({"type": "plan", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["doc_id"]
        .as_str()
        .unwrap_or(parsed["path"].as_str().unwrap_or(""));
    let bare_id = doc_id
        .rsplit('/')
        .next()
        .unwrap_or(doc_id)
        .trim_end_matches(".md");

    let status = read_doc_status(root, bare_id);
    assert!(status.is_none() || status.as_deref() == Some(""));
}

/// 2. revw 文档创建后进入 in_review 状态
#[test]
fn test_revw_created_in_review() {
    let temp = common::create_test_project("revw_in_review");
    let root = temp.path();

    let args = serde_json::json!({"type": "revw", "refs": []});
    let result = block_on(tool_create_document(root, &args, "menxiashizhong"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_path = parsed["path"].as_str().unwrap();
    let bare_id = doc_path
        .rsplit('/')
        .next()
        .unwrap_or(doc_path)
        .trim_end_matches(".md");

    let status = read_doc_status(root, bare_id);
    assert_eq!(status.as_deref(), Some("in_review"));
}

/// 3. dsgn 文档不会被标记为 in_review
#[test]
fn test_design_not_in_review() {
    let temp = common::create_test_project("design_not_review");
    let root = temp.path();

    let args = serde_json::json!({"type": "dsgn", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_path = parsed["path"].as_str().unwrap();
    let bare_id = doc_path
        .rsplit('/')
        .next()
        .unwrap_or(doc_path)
        .trim_end_matches(".md");

    let status = read_doc_status(root, bare_id);
    // dsgn docs should NOT have in_review status
    assert!(status.is_none() || status.as_deref() == Some(""));
}

/// 4. plan 文档不会加入 pending_approvals
#[test]
fn test_plan_not_adds_pending_approval() {
    let temp = common::create_test_project("plan_no_pending");
    let root = temp.path();

    let args = serde_json::json!({"type": "plan", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let pending = read_pending_approvals(root);
    assert!(
        pending.is_empty(),
        "plan docs should not create pending approvals"
    );
}

/// 5. dsgn 文档不会加入 pending_approvals
#[test]
fn test_design_does_not_add_pending() {
    let temp = common::create_test_project("design_no_pending");
    let root = temp.path();

    let args = serde_json::json!({"type": "dsgn", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let pending = read_pending_approvals(root);
    assert!(
        pending.is_empty(),
        "dsgn docs should not create pending approvals"
    );
}

/// 6. set_document_status(approved) 移除 pending approval 并放行
#[test]
fn test_set_approved_removes_pending() {
    let temp = common::create_test_project("approve_removes_pending");
    let root = temp.path();

    let args = serde_json::json!({"type": "revw", "refs": []});
    let result = block_on(tool_create_document(root, &args, "menxiashizhong"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Verify pending exists
    let pending_before = read_pending_approvals(root);
    assert!(pending_before.contains(&doc_id.to_string()));

    // Approve it
    let approve_args = serde_json::json!({"id": doc_id, "status": "approved"});
    let approve_result = block_on(tool_set_document_status(root, &approve_args));
    let approve_parsed: serde_json::Value = serde_json::from_str(&approve_result).unwrap();
    assert_eq!(
        approve_parsed["ok"], true,
        "Approval should succeed: {}",
        approve_result
    );

    // Verify pending removed
    let pending_after = read_pending_approvals(root);
    assert!(
        !pending_after.contains(&doc_id.to_string()),
        "pending should not contain approved doc"
    );

    // Verify document status changed
    let status = read_doc_status(root, doc_id);
    assert_eq!(status.as_deref(), Some("approved"));
}

/// 6b. 已批准 revw 被修改后回到 in_review
#[test]
fn test_approved_revw_modify_reverts_to_in_review() {
    let temp = common::create_test_project("revw_modify_reapprove");
    let root = temp.path();

    let args = serde_json::json!({"type": "revw", "refs": []});
    let result = block_on(tool_create_document(root, &args, "menxiashizhong"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let approve_args = serde_json::json!({"id": doc_id, "status": "approved"});
    let _ = block_on(tool_set_document_status(root, &approve_args));

    let modify_args = serde_json::json!({
        "id": doc_id,
        "content": "更新审查结论"
    });
    let _ = block_on(tool_modify_document(root, &modify_args, "menxiashizhong"));

    let status = read_doc_status(root, doc_id);
    assert_eq!(status.as_deref(), Some("in_review"));
    let pending = read_pending_approvals(root);
    assert!(pending.contains(&doc_id.to_string()));
}

/// 7. set_document_status(rejected) 被拒绝
#[test]
fn test_set_rejected_fails() {
    let temp = common::create_test_project("reject_fails");
    let root = temp.path();

    let args = serde_json::json!({"type": "revw", "refs": []});
    let result = block_on(tool_create_document(root, &args, "menxiashizhong"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let reject_args = serde_json::json!({
        "id": doc_id,
        "status": "rejected",
        "emperor_note": "缺少 API 定义"
    });
    let reject_result = block_on(tool_set_document_status(root, &reject_args));
    let reject_parsed: serde_json::Value = serde_json::from_str(&reject_result).unwrap();
    assert_eq!(reject_parsed["ok"], false);
    assert_eq!(reject_parsed["error_code"], "invalid_status");

    let status = read_doc_status(root, doc_id);
    assert_eq!(status.as_deref(), Some("in_review"));
}

/// 7b. 多条朱批 notes 用 | 分隔，round-trip 不丢失
#[test]
fn test_multiple_notes_roundtrip() {
    let temp = common::create_test_project("multi_notes");
    let root = temp.path();

    let args = serde_json::json!({"type": "revw", "refs": []});
    let result = block_on(tool_create_document(root, &args, "menxiashizhong"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["path"].as_str().unwrap_or(
        parsed["doc_id"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let approve_args = serde_json::json!({
        "id": doc_id,
        "status": "approved",
        "emperor_note": "第一遍准奏"
    });
    let _ = block_on(tool_set_document_status(root, &approve_args));

    let modify_args = serde_json::json!({
        "id": doc_id,
        "content": "更新后的审查内容"
    });
    let _ = block_on(tool_modify_document(root, &modify_args, "menxiashizhong"));

    let approve_args2 = serde_json::json!({
        "id": doc_id,
        "status": "approved",
        "emperor_note": "第二遍准奏"
    });
    let _ = block_on(tool_set_document_status(root, &approve_args2));

    let notes = read_doc_notes(root, doc_id).unwrap_or_default();
    assert!(
        notes.contains("第一遍准奏"),
        "first note should survive: {notes}"
    );
    assert!(
        notes.contains("第二遍准奏"),
        "second note should survive: {notes}"
    );
    assert!(
        notes.contains(" | "),
        "notes should use pipe separator: {notes}"
    );
}

/// 8. set_document_status(plan, approved) 失败
#[test]
fn test_set_status_plan_fails() {
    let temp = common::create_test_project("set_status_plan_fails");
    let root = temp.path();

    let args = serde_json::json!({"type": "plan", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let approve_args = serde_json::json!({"id": doc_id, "status": "approved"});
    let approve_result = block_on(tool_set_document_status(root, &approve_args));
    let approve_parsed: serde_json::Value = serde_json::from_str(&approve_result).unwrap();
    assert_eq!(approve_parsed["ok"], false);
    assert_eq!(approve_parsed["error_code"], "wrong_type");
}

/// 9. set_document_status 拒绝非 revw 类型
#[test]
fn test_set_status_wrong_type_fails() {
    let temp = common::create_test_project("set_status_wrong_type");
    let root = temp.path();

    let args = serde_json::json!({"type": "dsgn", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Try to approve a dsgn doc
    let approve_args = serde_json::json!({"id": doc_id, "status": "approved"});
    let approve_result = block_on(tool_set_document_status(root, &approve_args));
    let approve_parsed: serde_json::Value = serde_json::from_str(&approve_result).unwrap();
    assert_eq!(
        approve_parsed["ok"], false,
        "Should reject setting status on design doc"
    );
    assert_eq!(approve_parsed["error_code"], "wrong_type");
}

/// 10. set_document_status 拒绝不存在的文档
#[test]
fn test_set_status_nonexistent_fails() {
    let temp = common::create_test_project("set_status_nonexistent");
    let root = temp.path();

    let args = serde_json::json!({"id": "plan_99999", "status": "approved"});
    let result = block_on(tool_set_document_status(root, &args));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], false);
}

/// 11. set_document_status 拒绝无效 status 值
#[test]
fn test_set_status_invalid_value_fails() {
    let temp = common::create_test_project("set_status_invalid");
    let root = temp.path();

    let args = serde_json::json!({"type": "revw", "refs": []});
    let result = block_on(tool_create_document(root, &args, "menxiashizhong"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let bad_args = serde_json::json!({"id": doc_id, "status": "in_review"});
    let bad_result = block_on(tool_set_document_status(root, &bad_args));
    let bad_parsed: serde_json::Value = serde_json::from_str(&bad_result).unwrap();
    assert_eq!(bad_parsed["ok"], false);
}

// ── 门禁 gate 测试 ──────────────────────────────────────────

/// Extract numeric suffix from doc id (e.g. plan_3 → 3).
fn doc_num(id: &str) -> u64 {
    id.rsplit('_').next().unwrap_or("0").parse().unwrap_or(0)
}

/// 12. 未批准的 revw ref 会阻止检查 (check_doc_refs_approved_for_route)
#[test]
fn test_gate_blocks_unapproved_ref() {
    let temp = common::create_test_project("gate_block_unapproved");
    let root = temp.path();

    // Create a revw document (will be in_review)
    let revw_args = serde_json::json!({"type": "revw", "refs": []});
    let revw_result = block_on(tool_create_document(root, &revw_args, "menxiashizhong"));
    let revw_parsed: serde_json::Value = serde_json::from_str(&revw_result).unwrap();
    assert_eq!(revw_parsed["ok"], true);

    let revw_doc_id = revw_parsed["doc_id"].as_str().unwrap_or(
        revw_parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );
    let revw_num = doc_num(revw_doc_id);

    // Create a dsgn document that references the revw
    let dsgn_args = serde_json::json!({"type": "dsgn", "refs": [revw_num]});
    let dsgn_result = block_on(tool_create_document(root, &dsgn_args, "zhongshuling"));
    let dsgn_parsed: serde_json::Value = serde_json::from_str(&dsgn_result).unwrap();
    assert_eq!(dsgn_parsed["ok"], true);

    let dsgn_doc_id = dsgn_parsed["doc_id"].as_str().unwrap_or(
        dsgn_parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // The gate should block because dsgn references revw which is in_review
    let gate_result = block_on(check_doc_refs_approved_for_route(root, dsgn_doc_id));
    assert!(
        gate_result.is_err(),
        "Gate should block reference to unapproved revw doc"
    );
    let err_msg = gate_result.unwrap_err();
    assert!(
        err_msg.contains("not approved yet"),
        "Error should mention pending approval: {}",
        err_msg
    );
}

/// 13. approved ref 通过 gate 检查
#[test]
fn test_gate_passes_approved_ref() {
    let temp = common::create_test_project("gate_pass_approved");
    let root = temp.path();

    // Create a revw doc
    let revw_args = serde_json::json!({"type": "revw", "refs": []});
    let revw_result = block_on(tool_create_document(root, &revw_args, "menxiashizhong"));
    let revw_parsed: serde_json::Value = serde_json::from_str(&revw_result).unwrap();
    assert_eq!(revw_parsed["ok"], true);

    let revw_doc_id = revw_parsed["doc_id"].as_str().unwrap_or(
        revw_parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Approve it
    let approve_args = serde_json::json!({"id": revw_doc_id, "status": "approved"});
    let _ = block_on(tool_set_document_status(root, &approve_args));

    let revw_num = doc_num(revw_doc_id);
    // Create a doc that references the approved revw
    let dsgn_args = serde_json::json!({"type": "dsgn", "refs": [revw_num]});
    let dsgn_result = block_on(tool_create_document(root, &dsgn_args, "zhongshuling"));
    let dsgn_parsed: serde_json::Value = serde_json::from_str(&dsgn_result).unwrap();
    assert_eq!(dsgn_parsed["ok"], true);

    let dsgn_doc_id = dsgn_parsed["doc_id"].as_str().unwrap_or(
        dsgn_parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Gate should pass since revw is approved
    let gate_result = block_on(check_doc_refs_approved_for_route(root, dsgn_doc_id));
    assert!(
        gate_result.is_ok(),
        "Gate should pass when refs are approved: {:?}",
        gate_result.err()
    );
}

/// 14. 历史 rejected revw 也会被 gate 阻止
#[test]
fn test_gate_blocks_rejected_ref() {
    let temp = common::create_test_project("gate_block_rejected");
    let root = temp.path();

    let revw_args = serde_json::json!({"type": "revw", "refs": []});
    let revw_result = block_on(tool_create_document(root, &revw_args, "menxiashizhong"));
    let revw_parsed: serde_json::Value = serde_json::from_str(&revw_result).unwrap();
    assert_eq!(revw_parsed["ok"], true);

    let revw_doc_id = revw_parsed["doc_id"].as_str().unwrap_or(
        revw_parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Simulate legacy rejected status by editing frontmatter directly
    let revw_path = root.join(format!(".shuji/reviews/{revw_doc_id}.md"));
    let content = std::fs::read_to_string(&revw_path).unwrap();
    let updated = content.replace("status: in_review", "status: rejected");
    std::fs::write(&revw_path, updated).unwrap();

    let revw_num = doc_num(revw_doc_id);
    let dsgn_args = serde_json::json!({"type": "dsgn", "refs": [revw_num]});
    let dsgn_result = block_on(tool_create_document(root, &dsgn_args, "zhongshuling"));
    let dsgn_parsed: serde_json::Value = serde_json::from_str(&dsgn_result).unwrap();
    assert_eq!(dsgn_parsed["ok"], true);

    let dsgn_doc_id = dsgn_parsed["doc_id"].as_str().unwrap_or(
        dsgn_parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let gate_result = block_on(check_doc_refs_approved_for_route(root, dsgn_doc_id));
    assert!(
        gate_result.is_err(),
        "Gate should block legacy rejected ref"
    );
    assert!(
        gate_result.unwrap_err().contains("not approved yet"),
        "Error should mention not approved"
    );
}

/// 15. 无 refs 的文档不会触发门禁
#[test]
fn test_gate_no_refs_passes() {
    let temp = common::create_test_project("gate_no_refs");
    let root = temp.path();

    let args = serde_json::json!({"type": "dsgn", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let gate_result = block_on(check_doc_refs_approved_for_route(root, doc_id));
    assert!(
        gate_result.is_ok(),
        "Gate should pass for docs with no refs"
    );
}

#[test]
fn test_gate_blocks_missing_subject_doc() {
    let temp = common::create_test_project("gate_missing_subject");
    let root = temp.path();

    let gate_result = block_on(check_doc_refs_approved_for_route(root, "dsgn_999"));
    assert!(
        gate_result.is_err(),
        "Gate should fail closed on missing subject"
    );
    assert!(
        gate_result.unwrap_err().contains("does not exist"),
        "Error should explain missing subject"
    );
}

#[test]
fn test_gate_blocks_unparseable_subject_doc() {
    let temp = common::create_test_project("gate_bad_subject");
    let root = temp.path();
    let doc_dir = root.join(".shuji/designs");
    std::fs::create_dir_all(&doc_dir).unwrap();
    std::fs::write(doc_dir.join("dsgn_404.md"), "not valid frontmatter").unwrap();

    let gate_result = block_on(check_doc_refs_approved_for_route(root, "dsgn_404"));
    assert!(
        gate_result.is_err(),
        "Gate should fail closed on unparseable subject"
    );
    assert!(
        gate_result.unwrap_err().contains("cannot be parsed"),
        "Error should explain parse failure"
    );
}

#[test]
fn test_gate_blocks_unknown_revw_status() {
    let temp = common::create_test_project("gate_unknown_status");
    let root = temp.path();

    let revw_args = serde_json::json!({"type": "revw", "refs": []});
    let revw_result = block_on(tool_create_document(root, &revw_args, "menxiashizhong"));
    let revw_parsed: serde_json::Value = serde_json::from_str(&revw_result).unwrap();
    let revw_doc_id = revw_parsed["path"].as_str().unwrap();

    let revw_path = root.join(format!(".shuji/reviews/{revw_doc_id}.md"));
    let content = std::fs::read_to_string(&revw_path).unwrap();
    let updated = content.replace("status: in_review", "status: unknown");
    std::fs::write(&revw_path, updated).unwrap();

    let revw_num = doc_num(revw_doc_id);
    let dsgn_args = serde_json::json!({"type": "dsgn", "refs": [revw_num]});
    let dsgn_result = block_on(tool_create_document(root, &dsgn_args, "zhongshuling"));
    let dsgn_parsed: serde_json::Value = serde_json::from_str(&dsgn_result).unwrap();
    let dsgn_doc_id = dsgn_parsed["path"].as_str().unwrap();

    let gate_result = block_on(check_doc_refs_approved_for_route(root, dsgn_doc_id));
    assert!(
        gate_result.is_err(),
        "Gate should fail closed on unknown revw status"
    );
    assert!(
        gate_result.unwrap_err().contains("not approved yet"),
        "Unknown status should be treated as not approved"
    );
}

/// 16. pending approval 扫描与移除（基于 status 真相源）
#[test]
fn test_pending_approval_add_remove() {
    let temp = common::create_test_project("pending_add_remove");
    let root = temp.path();

    // Initially empty
    let before = block_on(get_first_pending_approval(root));
    assert!(before.is_none(), "Should start with no pending approvals");

    // Create revw doc → auto in_review
    let args = serde_json::json!({"type": "revw", "refs": []});
    let result = block_on(tool_create_document(root, &args, "menxiashizhong"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);
    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let after_add = block_on(get_first_pending_approval(root));
    assert_eq!(after_add.as_deref(), Some(doc_id));

    // Approve → no longer pending
    let approve_args = serde_json::json!({"id": doc_id, "status": "approved"});
    let _ = block_on(tool_set_document_status(root, &approve_args));

    let after_remove = block_on(get_first_pending_approval(root));
    assert!(after_remove.is_none(), "Should be empty after approval");
}

#[test]
fn test_read_document_accepts_md_suffix() {
    use shuji_app_lib::tool::documents::tool_read_document;

    let temp = common::create_test_project("doc_read_md_suffix");
    let root = temp.path();

    let create_args = serde_json::json!({
        "type": "dsgn",
        "refs": "[-1]",
        "content": "## Scope\nTest design body"
    });
    let created = block_on(tool_create_document(root, &create_args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&created).unwrap();
    assert!(parsed["ok"].as_bool().unwrap());
    let doc_id = parsed["path"].as_str().unwrap();

    let read_args = serde_json::json!({ "id": format!("{}.md", doc_id) });
    let result = block_on(tool_read_document(root, &read_args));
    let read_parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(
        read_parsed["ok"].as_bool().unwrap(),
        "read_document should accept .md suffix: {}",
        result
    );
}

// ── 作者归属 & 部门权限闸门 ──────────────────────────────────────

#[test]
fn test_dept_to_author_case_insensitive() {
    let temp = common::create_test_project("doc_author_case");
    let root = temp.path();

    let cases: [(&str, &str, &str); 4] = [
        ("Zhongshuling", "dsgn", "中书令"),
        ("GONGBUSHANGSHU", "rprt", "工部"),
        ("neige", "task", "内阁"),
        ("requirements_agent", "reqs", "内阁"),
    ];

    for (dept, doc_type, expected_author) in cases {
        let args = serde_json::json!({"type": doc_type, "refs": []});
        let result = block_on(tool_create_document(root, &args, dept));
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            parsed["ok"], true,
            "dept {dept} should create {doc_type}: {}",
            parsed["message"]
        );
        let doc_id = parsed["path"].as_str().unwrap();
        assert_eq!(
            read_doc_author(root, doc_id).as_deref(),
            Some(expected_author),
            "author for dept {dept}"
        );
    }
}

#[test]
fn test_neige_cannot_create_plan() {
    let temp = common::create_test_project("doc_neige_plan_forbidden");
    let root = temp.path();

    let args = serde_json::json!({"type": "plan", "refs": []});
    let result = block_on(tool_create_document(root, &args, "neige"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["error_code"], "forbidden_type");
    assert!(
        parsed["message"]
            .as_str()
            .unwrap()
            .contains("submit_pipeline_plan"),
        "Should hint submit_pipeline_plan: {}",
        parsed["message"]
    );

    let designs_dir = root.join(".shuji/designs");
    let plan_files: Vec<_> = std::fs::read_dir(&designs_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("plan_") && n.ends_with(".md"))
                .unwrap_or(false)
        })
        .collect();
    assert!(plan_files.is_empty(), "No plan file should be created");
    assert!(
        read_pending_approvals(root).is_empty(),
        "Should not enter pending approval queue"
    );
}

#[test]
fn test_zhongshuling_can_create_plan() {
    let temp = common::create_test_project("doc_zhongshuling_plan");
    let root = temp.path();

    let args = serde_json::json!({"type": "plan", "refs": []});
    let result = block_on(tool_create_document(root, &args, "Zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["ok"], true, "{}", parsed["message"]);
    assert_eq!(
        read_doc_author(root, parsed["path"].as_str().unwrap()),
        Some("中书令".to_string())
    );
}

#[test]
fn test_neige_can_create_task() {
    let temp = common::create_test_project("doc_neige_task");
    let root = temp.path();

    let args = serde_json::json!({"type": "task", "refs": []});
    let result = block_on(tool_create_document(root, &args, "neige"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["ok"], true, "{}", parsed["message"]);
    let doc_id = parsed["path"].as_str().unwrap();
    assert_eq!(read_doc_author(root, doc_id), Some("内阁".to_string()));
    assert!(root
        .join(".shuji/tasks")
        .join(format!("{doc_id}.md"))
        .exists());
}
