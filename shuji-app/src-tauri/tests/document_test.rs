//! 文档系统测试 - 测试文档创建、修改、查找功能
//!
//! 运行: cargo test --test document_test -- --nocapture

mod common;

use shuji_app_lib::tool::documents::{
    tool_append_document, tool_create_document, tool_find_document, tool_modify_document,
};

/// Sync wrapper for async tool functions in tests.
fn block_on<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
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
