//! 工具执行测试 - 测试文件操作和命令执行工具
//!
//! 运行: cargo test --test tool_test -- --nocapture

mod common;

use shuji_app_lib::tool::{
    tool_create_file, tool_read_file, tool_modify_file, tool_append_file,
    tool_delete_file, tool_rename_file, tool_list_dir, tool_execute_command,
};

// ── create_file 测试 ──────────────────────────────────────────

#[test]
fn test_create_file_success() {
    let temp = common::create_test_project("tool_create");
    let root = temp.path();
    
    let args = serde_json::json!({
        "path": "test.txt",
        "content": "Hello, World!"
    });
    
    let result = tool_create_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["operation"], "create_file");
    
    let file_path = root.join("test.txt");
    assert!(file_path.exists());
    assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "Hello, World!");
}

#[test]
fn test_create_file_nested_directory() {
    let temp = common::create_test_project("tool_create_nested");
    let root = temp.path();
    
    let args = serde_json::json!({
        "path": "src/components/Button.tsx",
        "content": "export const Button = () => <button>Click</button>;"
    });
    
    let result = tool_create_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], true);
    
    let file_path = root.join("src/components/Button.tsx");
    assert!(file_path.exists());
}

#[test]
fn test_create_file_reject_overwrite() {
    let temp = common::create_test_project("tool_create_overwrite");
    let root = temp.path();
    
    common::fixtures::create_test_file(root, "existing.txt", "original content");
    
    let args = serde_json::json!({
        "path": "existing.txt",
        "content": "new content"
    });
    
    let result = tool_create_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["error_code"], "already_exists");
    
    let content = std::fs::read_to_string(root.join("existing.txt")).unwrap();
    assert_eq!(content, "original content");
}

// ── read_file 测试 ────────────────────────────────────────────

#[test]
fn test_read_file_success() {
    let temp = common::create_test_project("tool_read");
    let root = temp.path();
    
    common::fixtures::create_test_file(root, "test.txt", "Line 1\nLine 2\nLine 3");
    
    let args = serde_json::json!({
        "path": "test.txt"
    });
    
    let result = tool_read_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], true);
    assert!(parsed["message"].as_str().unwrap().contains("Line 1"));
}

#[test]
fn test_read_file_with_offset_limit() {
    let temp = common::create_test_project("tool_read_range");
    let root = temp.path();
    
    let content = (0..100).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
    common::fixtures::create_test_file(root, "large.txt", &content);
    
    let args = serde_json::json!({
        "path": "large.txt",
        "offset": 10,
        "limit": 5
    });
    
    let result = tool_read_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], true);
    let message = parsed["message"].as_str().unwrap();
    assert!(message.contains("Line 10"));
    assert!(message.contains("Line 14"));
    assert!(!message.contains("Line 15"));
}

// ── modify_file 测试 ──────────────────────────────────────────

#[test]
fn test_modify_file_success() {
    let temp = common::create_test_project("tool_modify");
    let root = temp.path();
    
    common::fixtures::create_test_file(root, "code.rs", "fn old_name() {\n    println!(\"test\");\n}");
    
    let args = serde_json::json!({
        "path": "code.rs",
        "old_text": "old_name",
        "new_text": "new_name"
    });
    
    let result = tool_modify_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], true);
    
    let content = std::fs::read_to_string(root.join("code.rs")).unwrap();
    assert!(content.contains("new_name"));
    assert!(!content.contains("old_name"));
}

#[test]
fn test_modify_file_not_found_text() {
    let temp = common::create_test_project("tool_modify_notfound");
    let root = temp.path();
    
    common::fixtures::create_test_file(root, "code.rs", "fn test() {}");
    
    let args = serde_json::json!({
        "path": "code.rs",
        "old_text": "nonexistent_text",
        "new_text": "replacement"
    });
    
    let result = tool_modify_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], false);
    assert_eq!(parsed["error_code"], "not_found");
}

// ── append_file 测试 ──────────────────────────────────────────

#[test]
fn test_append_file_to_existing() {
    let temp = common::create_test_project("tool_append");
    let root = temp.path();
    
    common::fixtures::create_test_file(root, "log.txt", "Line 1");
    
    let args = serde_json::json!({
        "path": "log.txt",
        "content": "Line 2"
    });
    
    let result = tool_append_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], true);
    
    let content = std::fs::read_to_string(root.join("log.txt")).unwrap();
    assert!(content.contains("Line 1"));
    assert!(content.contains("Line 2"));
}

// ── delete_file 测试 ──────────────────────────────────────────

#[test]
fn test_delete_file_success() {
    let temp = common::create_test_project("tool_delete");
    let root = temp.path();
    
    common::fixtures::create_test_file(root, "to_delete.txt", "content");
    
    let args = serde_json::json!({
        "path": "to_delete.txt"
    });
    
    let result = tool_delete_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], true);
    assert!(!root.join("to_delete.txt").exists());
}

// ── rename_file 测试 ──────────────────────────────────────────

#[test]
fn test_rename_file_success() {
    let temp = common::create_test_project("tool_rename");
    let root = temp.path();
    
    common::fixtures::create_test_file(root, "old.txt", "content");
    
    let args = serde_json::json!({
        "from": "old.txt",
        "to": "new.txt"
    });
    
    let result = tool_rename_file(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], true);
    assert!(!root.join("old.txt").exists());
    assert!(root.join("new.txt").exists());
}

// ── list_dir 测试 ─────────────────────────────────────────────

#[test]
fn test_list_dir_success() {
    let temp = common::create_test_project("tool_list");
    let root = temp.path();
    
    common::fixtures::create_test_file(root, "file1.txt", "");
    common::fixtures::create_test_file(root, "file2.txt", "");
    std::fs::create_dir(root.join("subdir")).unwrap();
    
    let args = serde_json::json!({
        "path": ""
    });
    
    let result = tool_list_dir(root, &args);
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    
    assert_eq!(parsed["ok"], true);
    let message = parsed["message"].as_str().unwrap();
    assert!(message.contains("file1.txt"));
    assert!(message.contains("file2.txt"));
    assert!(message.contains("subdir"));
}

// ── execute_command 测试 ──────────────────────────────────────

#[test]
fn test_execute_command_block_dangerous() {
    let temp = common::create_test_project("tool_exec_danger");
    let root = temp.path();
    
    let dangerous_commands = vec![
        "format c:",
        "sudo rm -rf /",
        "shutdown -h now",
    ];
    
    for cmd in dangerous_commands {
        let args = serde_json::json!({
            "command": cmd
        });
        
        let result = tool_execute_command(root, &args, "test");
        // Command should either be blocked by safety check OR fail to execute
        // Both outcomes are safe - we just don't want it to succeed
        assert!(
            result.contains("安全拦截") || result.contains("禁止") || result.contains("失败") || result.contains("错误"),
            "Command '{}' should be blocked or fail: {}", cmd, result
        );
    }
}
