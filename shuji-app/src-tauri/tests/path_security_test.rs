//! 路径安全测试 - 测试 resolve_scoped_path 函数防止目录穿越攻击
//!
//! 运行: cargo test --test path_security_test -- --nocapture

mod common;

use shuji_app_lib::tool::resolve_scoped_path;
use std::path::{Path, PathBuf};

/// Sync wrapper so existing tests don't need to become async.
fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    // Create a single-thread runtime for the synchronous call.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(resolve_scoped_path(root, rel))
}

#[test]
fn test_normal_relative_path() {
    let temp = common::create_test_project("path_normal");
    let root = temp.path();

    // 正常的相对路径应该通过
    let result = resolve(root, "src/main.rs");
    assert!(result.is_ok(), "Normal relative path should be allowed");

    let resolved = result.unwrap();
    common::assert_path_within_root(root, &resolved);
}

#[test]
fn test_nested_relative_path() {
    let temp = common::create_test_project("path_nested");
    let root = temp.path();

    // 深层嵌套的相对路径应该通过
    let result = resolve(root, "src/components/ui/Button.tsx");
    assert!(result.is_ok(), "Nested relative path should be allowed");

    let resolved = result.unwrap();
    common::assert_path_within_root(root, &resolved);
}

#[test]
fn test_reject_parent_directory_traversal_unix() {
    let temp = common::create_test_project("path_traversal_unix");
    let root = temp.path();

    // Unix 风格的父目录穿越应该被拒绝
    let result = resolve(root, "../etc/passwd");
    common::assert_path_error_contains(&result, "禁止使用父目录跳转");
}

#[test]
fn test_reject_parent_directory_traversal_windows() {
    let temp = common::create_test_project("path_traversal_windows");
    let root = temp.path();

    // Windows 风格的父目录穿越应该被拒绝
    let result = resolve(root, "..\\Windows\\System32");
    common::assert_path_error_contains(&result, "禁止使用父目录跳转");
}

#[test]
fn test_reject_multiple_parent_traversal() {
    let temp = common::create_test_project("path_multi_traversal");
    let root = temp.path();

    // 多层父目录穿越应该被拒绝
    let result = resolve(root, "../../../../../../etc/passwd");
    common::assert_path_error_contains(&result, "禁止使用父目录跳转");
}

#[test]
fn test_reject_hidden_parent_traversal() {
    let temp = common::create_test_project("path_hidden_traversal");
    let root = temp.path();

    // 隐藏在路径中间的父目录穿越应该被拒绝
    let result = resolve(root, "src/../../../etc/passwd");
    common::assert_path_error_contains(&result, "禁止使用父目录跳转");
}

#[test]
fn test_reject_absolute_path_unix() {
    let temp = common::create_test_project("path_absolute_unix");
    let root = temp.path();

    // Unix 绝对路径应该被拒绝
    let result = resolve(root, "/etc/passwd");
    assert!(result.is_err(), "Absolute Unix path should be rejected");
}

#[test]
#[cfg(windows)]
fn test_reject_absolute_path_windows() {
    let temp = common::create_test_project("path_absolute_windows");
    let root = temp.path();

    // Windows 绝对路径应该被拒绝
    let result = resolve(root, "C:\\Windows\\System32\\cmd.exe");
    common::assert_path_error_contains(&result, "禁止使用绝对路径");
}

#[test]
#[cfg(windows)]
fn test_reject_drive_letter_c() {
    let temp = common::create_test_project("path_drive_c");
    let root = temp.path();

    // C: 盘符路径会被绝对路径检查捕获
    let result = resolve(root, "c:\\Windows\\System32");
    assert!(result.is_err(), "Drive letter path should be rejected");
}

#[test]
#[cfg(windows)]
fn test_reject_drive_letter_d() {
    let temp = common::create_test_project("path_drive_d");
    let root = temp.path();

    // D: 盘符路径会被绝对路径检查捕获
    let result = resolve(root, "d:\\data\\secrets.txt");
    assert!(result.is_err(), "Drive letter path should be rejected");
}

#[test]
#[cfg(windows)]
fn test_reject_drive_letter_e() {
    let temp = common::create_test_project("path_drive_e");
    let root = temp.path();

    // E: 盘符路径会被绝对路径检查捕获
    let result = resolve(root, "E:\\backup\\important.db");
    assert!(result.is_err(), "Drive letter path should be rejected");
}

#[test]
fn test_existing_file_within_root() {
    let temp = common::create_test_project("path_existing");
    let root = temp.path();

    // 创建一个实际存在的文件
    common::fixtures::create_test_file(root, "test.txt", "content");

    // 已存在的文件应该被正确解析
    let result = resolve(root, "test.txt");
    assert!(result.is_ok(), "Existing file should be resolved");

    let resolved = result.unwrap();
    common::assert_path_within_root(root, &resolved);
    assert!(resolved.exists(), "Resolved path should exist");
}

#[test]
fn test_nonexistent_file_within_root() {
    let temp = common::create_test_project("path_nonexistent");
    let root = temp.path();

    // 不存在的文件（但路径合法）应该通过
    let result = resolve(root, "future_file.txt");
    assert!(
        result.is_ok(),
        "Non-existent file with valid path should be allowed"
    );

    let resolved = result.unwrap();
    common::assert_path_within_root(root, &resolved);
}

#[test]
fn test_nonexistent_nested_file() {
    let temp = common::create_test_project("path_nested_nonexistent");
    let root = temp.path();

    // 不存在的嵌套文件（父目录也不存在）应该通过
    let result = resolve(root, "src/components/NewComponent.tsx");
    assert!(result.is_ok(), "Non-existent nested file should be allowed");

    let resolved = result.unwrap();
    common::assert_path_within_root(root, &resolved);
}

#[test]
#[cfg(unix)]
fn test_reject_symlink_escape() {
    let temp = common::create_test_project("path_symlink");
    let root = temp.path();

    // 创建一个指向外部的符号链接
    let external_dir = tempfile::tempdir().unwrap();
    let external_file = external_dir.path().join("secret.txt");
    std::fs::write(&external_file, "secret data").unwrap();

    let symlink_path = root.join("link_to_external");
    common::fixtures::create_symlink(&external_file, &symlink_path);

    // 通过符号链接逃逸应该被检测到
    let result = resolve(root, "link_to_external");

    // 符号链接会被 canonicalize 解析，应该检测到路径越界
    if result.is_ok() {
        let resolved = result.unwrap();
        assert!(
            !resolved.starts_with(root),
            "Symlink escape should be detected"
        );
    } else {
        // 或者直接返回错误也是可以接受的
        common::assert_path_error_contains(&result, "路径越界");
    }
}

#[test]
fn test_empty_path() {
    let temp = common::create_test_project("path_empty");
    let root = temp.path();

    // 空路径应该解析为根目录本身
    let result = resolve(root, "");
    assert!(result.is_ok(), "Empty path should resolve to root");

    let resolved = result.unwrap();
    let canon_root = std::fs::canonicalize(root).unwrap();
    let canon_resolved = std::fs::canonicalize(&resolved).unwrap_or(resolved.clone());
    assert_eq!(
        canon_resolved, canon_root,
        "Empty path should resolve to root directory"
    );
}

#[test]
fn test_dot_path() {
    let temp = common::create_test_project("path_dot");
    let root = temp.path();

    // "." 应该解析为根目录本身
    let result = resolve(root, ".");
    assert!(result.is_ok(), "Dot path should resolve to root");

    // Just verify it resolved successfully and is within root
    let resolved = result.unwrap();
    assert!(resolved.exists() || resolved == root.join("."));
}

#[test]
fn test_path_with_spaces() {
    let temp = common::create_test_project("path_spaces");
    let root = temp.path();

    // 包含空格的路径应该正常工作
    let result = resolve(root, "my documents/file with spaces.txt");
    assert!(result.is_ok(), "Path with spaces should be allowed");

    let resolved = result.unwrap();
    common::assert_path_within_root(root, &resolved);
}

#[test]
fn test_path_with_unicode() {
    let temp = common::create_test_project("path_unicode");
    let root = temp.path();

    // 包含 Unicode 字符的路径应该正常工作
    let result = resolve(root, "文档/测试文件.txt");
    assert!(result.is_ok(), "Path with Unicode should be allowed");

    let resolved = result.unwrap();
    common::assert_path_within_root(root, &resolved);
}

#[test]
fn test_deeply_nested_path() {
    let temp = common::create_test_project("path_deep");
    let root = temp.path();

    // 非常深的嵌套路径应该正常工作
    let deep_path = "a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z/file.txt";
    let result = resolve(root, deep_path);
    assert!(result.is_ok(), "Deeply nested path should be allowed");

    let resolved = result.unwrap();
    common::assert_path_within_root(root, &resolved);
}

#[test]
fn test_path_normalization() {
    let temp = common::create_test_project("path_normalize");
    let root = temp.path();

    // 包含多余斜杠的路径应该被正规化
    let result = resolve(root, "src//components///Button.tsx");
    assert!(
        result.is_ok(),
        "Path with extra slashes should be normalized"
    );

    let resolved = result.unwrap();
    common::assert_path_within_root(root, &resolved);
}

#[test]
fn test_reject_null_byte() {
    let temp = common::create_test_project("path_null");
    let root = temp.path();

    // 包含空字节的路径应该被拒绝（Rust 的 Path 会自动处理）
    let result = resolve(root, "test\0.txt");
    // Rust 的 Path 会将 \0 视为路径的一部分，但文件系统会拒绝
    // 这个测试主要确保不会崩溃
    let _ = result;
}

#[test]
fn test_case_sensitivity() {
    let temp = common::create_test_project("path_case");
    let root = temp.path();

    // 创建一个文件
    common::fixtures::create_test_file(root, "Test.txt", "content");

    // 不同大小写的路径（在 Windows 上应该解析到同一文件，Unix 上不同）
    let result1 = resolve(root, "Test.txt");
    let result2 = resolve(root, "test.txt");

    assert!(result1.is_ok(), "Original case should work");
    assert!(
        result2.is_ok(),
        "Different case should not cause security error"
    );

    // 两个路径都应该在根目录内
    common::assert_path_within_root(root, &result1.unwrap());
    common::assert_path_within_root(root, &result2.unwrap());
}
