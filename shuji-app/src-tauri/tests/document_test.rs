//! 文档系统测试 + 朱批门禁测试
//!
//! 运行: cargo test --test document_test -- --nocapture

mod common;

use shuji_app_lib::tool::documents::{
    add_pending_approval, check_doc_refs_approved_for_route, get_first_pending_approval,
    remove_pending_approval, tool_append_document, tool_create_document, tool_find_document,
    tool_modify_document, tool_set_document_status,
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

    let args = serde_json::json!({
        "type": "dsgn",
        "refs": [1, 3]
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

/// 1. plan 文档创建后进入 in_review 状态
#[test]
fn test_plan_created_in_review() {
    let temp = common::create_test_project("plan_in_review");
    let root = temp.path();

    let args = serde_json::json!({"type": "plan", "refs": [1]});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["doc_id"]
        .as_str()
        .unwrap_or(parsed["path"].as_str().unwrap_or(""));
    // Extract bare ID from path like ".shuji/designs/plan_1.md"
    let bare_id = doc_id
        .split('/')
        .last()
        .unwrap_or(doc_id)
        .trim_end_matches(".md");

    let status = read_doc_status(root, bare_id);
    assert_eq!(status.as_deref(), Some("in_review"));
}

/// 2. revw 文档创建后进入 in_review 状态
#[test]
fn test_revw_created_in_review() {
    let temp = common::create_test_project("revw_in_review");
    let root = temp.path();

    let args = serde_json::json!({"type": "revw", "refs": [1]});
    let result = block_on(tool_create_document(root, &args, "menxiashizhong"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_path = parsed["path"].as_str().unwrap();
    let bare_id = doc_path
        .split('/')
        .last()
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

    let args = serde_json::json!({"type": "dsgn", "refs": [1]});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_path = parsed["path"].as_str().unwrap();
    let bare_id = doc_path
        .split('/')
        .last()
        .unwrap_or(doc_path)
        .trim_end_matches(".md");

    let status = read_doc_status(root, bare_id);
    // dsgn docs should NOT have in_review status
    assert!(status.is_none() || status.as_deref() == Some(""));
}

/// 4. plan 文档创建后自动加入 pending_approvals
#[test]
fn test_plan_adds_pending_approval() {
    let temp = common::create_test_project("plan_pending_add");
    let root = temp.path();

    let args = serde_json::json!({"type": "plan", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let pending = read_pending_approvals(root);
    assert!(
        !pending.is_empty(),
        "pending_approvals should not be empty after creating a plan doc"
    );

    let doc_id = parsed["doc_id"].as_str().unwrap_or("");
    // The pending list should contain the doc_id
    if !doc_id.is_empty() {
        assert!(
            pending.contains(&doc_id.to_string()),
            "pending_approvals should contain {}",
            doc_id
        );
    }
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

    // Create a plan doc
    let args = serde_json::json!({"type": "plan", "refs": [1]});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .last()
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

/// 7. set_document_status(rejected) 保存皇帝意见
#[test]
fn test_set_rejected_saves_emperor_note() {
    let temp = common::create_test_project("reject_saves_note");
    let root = temp.path();

    let args = serde_json::json!({"type": "plan", "refs": [1]});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Reject with note
    let reject_args = serde_json::json!({
        "id": doc_id,
        "status": "rejected",
        "emperor_note": "缺少 API 定义"
    });
    let reject_result = block_on(tool_set_document_status(root, &reject_args));
    let reject_parsed: serde_json::Value = serde_json::from_str(&reject_result).unwrap();
    assert_eq!(reject_parsed["ok"], true, "Rejection should succeed");

    // Verify status
    let status = read_doc_status(root, doc_id);
    assert_eq!(status.as_deref(), Some("rejected"));

    // Verify emperor note was saved
    let notes = read_doc_notes(root, doc_id);
    assert_eq!(notes.as_deref(), Some("缺少 API 定义"));
}

/// 8. set_document_status(rejected) 也移除 pending approval
#[test]
fn test_set_rejected_removes_pending() {
    let temp = common::create_test_project("reject_removes_pending");
    let root = temp.path();

    let args = serde_json::json!({"type": "plan", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ok"], true);

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let reject_args = serde_json::json!({"id": doc_id, "status": "rejected"});
    let _ = block_on(tool_set_document_status(root, &reject_args));

    let pending = read_pending_approvals(root);
    assert!(
        !pending.contains(&doc_id.to_string()),
        "pending should not contain rejected doc"
    );
}

/// 9. set_document_status 拒绝非 plan/revw 类型
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
            .split('/')
            .last()
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

    let args = serde_json::json!({"type": "plan", "refs": []});
    let result = block_on(tool_create_document(root, &args, "zhongshuling"));
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    let doc_id = parsed["doc_id"].as_str().unwrap_or(
        parsed["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let bad_args = serde_json::json!({"id": doc_id, "status": "in_review"});
    let bad_result = block_on(tool_set_document_status(root, &bad_args));
    let bad_parsed: serde_json::Value = serde_json::from_str(&bad_result).unwrap();
    assert_eq!(bad_parsed["ok"], false);
}

// ── 门禁 gate 测试 ──────────────────────────────────────────

/// 12. 未批准的 plan ref 会阻止检查 (check_doc_refs_approved_for_route)
///
/// 计数器从 0 开始，第一个文档 plan → plan_0，引用 refs=[0] 才能指向 plan_0
#[test]
fn test_gate_blocks_unapproved_ref() {
    let temp = common::create_test_project("gate_block_unapproved");
    let root = temp.path();

    // Create a plan document (will be in_review, gets id_num=0 → plan_0)
    let plan_args = serde_json::json!({"type": "plan", "refs": [0]});
    let plan_result = block_on(tool_create_document(root, &plan_args, "zhongshuling"));
    let plan_parsed: serde_json::Value = serde_json::from_str(&plan_result).unwrap();
    assert_eq!(plan_parsed["ok"], true);

    // Create a dsgn document that references plan_0 via refs=[0]
    // Gets id_num=1 → dsgn_1, but refs=[0] means it references plan_0/revw_0
    let dsgn_args = serde_json::json!({"type": "dsgn", "refs": [0]});
    let dsgn_result = block_on(tool_create_document(root, &dsgn_args, "zhongshuling"));
    let dsgn_parsed: serde_json::Value = serde_json::from_str(&dsgn_result).unwrap();
    assert_eq!(dsgn_parsed["ok"], true);

    let dsgn_doc_id = dsgn_parsed["doc_id"].as_str().unwrap_or(
        dsgn_parsed["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // The gate should block because dsgn_1 references plan_0 which is in_review
    let gate_result = block_on(check_doc_refs_approved_for_route(root, dsgn_doc_id));
    assert!(
        gate_result.is_err(),
        "Gate should block reference to unapproved plan doc"
    );
    let err_msg = gate_result.unwrap_err();
    assert!(
        err_msg.contains("尚在待皇帝御批"),
        "Error should mention pending approval: {}",
        err_msg
    );
}

/// 13. approved ref 通过 gate 检查
#[test]
fn test_gate_passes_approved_ref() {
    let temp = common::create_test_project("gate_pass_approved");
    let root = temp.path();

    // Create a plan doc (gets plan_0)
    let plan_args = serde_json::json!({"type": "plan", "refs": []});
    let plan_result = block_on(tool_create_document(root, &plan_args, "zhongshuling"));
    let plan_parsed: serde_json::Value = serde_json::from_str(&plan_result).unwrap();
    assert_eq!(plan_parsed["ok"], true);

    let plan_doc_id = plan_parsed["doc_id"].as_str().unwrap_or(
        plan_parsed["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Approve it
    let approve_args = serde_json::json!({"id": plan_doc_id, "status": "approved"});
    let _ = block_on(tool_set_document_status(root, &approve_args));

    // Create a doc that references plan_0 via refs=[0]
    let dsgn_args = serde_json::json!({"type": "dsgn", "refs": [0]});
    let dsgn_result = block_on(tool_create_document(root, &dsgn_args, "zhongshuling"));
    let dsgn_parsed: serde_json::Value = serde_json::from_str(&dsgn_result).unwrap();
    assert_eq!(dsgn_parsed["ok"], true);

    let dsgn_doc_id = dsgn_parsed["doc_id"].as_str().unwrap_or(
        dsgn_parsed["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Gate should pass since plan is approved
    let gate_result = block_on(check_doc_refs_approved_for_route(root, dsgn_doc_id));
    assert!(
        gate_result.is_ok(),
        "Gate should pass when refs are approved: {:?}",
        gate_result.err()
    );
}

/// 14. 被驳回的 ref 也会被 gate 阻止
#[test]
fn test_gate_blocks_rejected_ref() {
    let temp = common::create_test_project("gate_block_rejected");
    let root = temp.path();

    // Create a plan doc (plan_0)
    let plan_args = serde_json::json!({"type": "plan", "refs": []});
    let plan_result = block_on(tool_create_document(root, &plan_args, "zhongshuling"));
    let plan_parsed: serde_json::Value = serde_json::from_str(&plan_result).unwrap();
    assert_eq!(plan_parsed["ok"], true);

    let plan_doc_id = plan_parsed["doc_id"].as_str().unwrap_or(
        plan_parsed["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Reject it
    let reject_args = serde_json::json!({"id": plan_doc_id, "status": "rejected"});
    let _ = block_on(tool_set_document_status(root, &reject_args));

    // Create a doc that references plan_0 via refs=[0]
    let dsgn_args = serde_json::json!({"type": "dsgn", "refs": [0]});
    let dsgn_result = block_on(tool_create_document(root, &dsgn_args, "zhongshuling"));
    let dsgn_parsed: serde_json::Value = serde_json::from_str(&dsgn_result).unwrap();
    assert_eq!(dsgn_parsed["ok"], true);

    let dsgn_doc_id = dsgn_parsed["doc_id"].as_str().unwrap_or(
        dsgn_parsed["path"]
            .as_str()
            .unwrap_or("")
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    // Gate should block rejected refs
    let gate_result = block_on(check_doc_refs_approved_for_route(root, dsgn_doc_id));
    assert!(gate_result.is_err(), "Gate should block rejected ref");
    assert!(
        gate_result.unwrap_err().contains("被驳回"),
        "Error should mention rejection"
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
            .split('/')
            .last()
            .unwrap_or("")
            .trim_end_matches(".md"),
    );

    let gate_result = block_on(check_doc_refs_approved_for_route(root, doc_id));
    assert!(
        gate_result.is_ok(),
        "Gate should pass for docs with no refs"
    );
}

/// 16. add/remove_pending_approval API 基本功能
#[test]
fn test_pending_approval_add_remove() {
    let temp = common::create_test_project("pending_add_remove");
    let root = temp.path();

    // Initially empty
    let before = block_on(get_first_pending_approval(root));
    assert!(before.is_none(), "Should start with no pending approvals");

    // Add a pending approval
    let add_result = block_on(add_pending_approval(root, "plan_5"));
    assert!(add_result.is_ok());

    let after_add = block_on(get_first_pending_approval(root));
    assert_eq!(after_add.as_deref(), Some("plan_5"));

    // Remove it
    let remove_result = block_on(remove_pending_approval(root, "plan_5"));
    assert!(remove_result.is_ok());

    let after_remove = block_on(get_first_pending_approval(root));
    assert!(after_remove.is_none(), "Should be empty after removal");
}
