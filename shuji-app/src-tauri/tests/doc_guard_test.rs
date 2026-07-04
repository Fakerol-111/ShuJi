//! Documentation guard tests.
//!
//! These tests verify that documentation files (AGENTS.md, README.md) stay
//! in sync with the actual codebase structure. They prevent documentation
//! drift by checking that key file paths mentioned in docs actually exist.
//!
//! Also guards against reintroduction of deprecated `AnthropicClient` usage
//! in test files (should use `LlmClient` instead).

use std::path::{Path, PathBuf};

fn project_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is shuji-app/src-tauri/, go up two levels to repo root
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("manifest has grandparent")
        .to_path_buf()
}

/// Key source files that must exist for the project to function.
/// These are derived from the AGENTS.md "Key File Locations" section.
const REQUIRED_SOURCE_FILES: &[&str] = &[
    "shuji-app/src-tauri/src/lib.rs",
    "shuji-app/src-tauri/src/actor/mod.rs",
    "shuji-app/src-tauri/src/agent/trait.rs",
    "shuji-app/src-tauri/src/agent/runner.rs",
    "shuji-app/src-tauri/src/agent/neige/mod.rs",
    "shuji-app/src-tauri/src/agent/zhongshuling/mod.rs",
    "shuji-app/src-tauri/src/agent/menxiashizhong/mod.rs",
    "shuji-app/src-tauri/src/agent/shangshuling/mod.rs",
    "shuji-app/src-tauri/src/agent/libushangshu/mod.rs",
    "shuji-app/src-tauri/src/agent/bingbushangshu/mod.rs",
    "shuji-app/src-tauri/src/agent/gongbushangshu/mod.rs",
    "shuji-app/src-tauri/src/agent/xingbushangshu/mod.rs",
    "shuji-app/src-tauri/src/agent/liburshangshu/mod.rs",
    "shuji-app/src-tauri/src/api/client.rs",
    "shuji-app/src-tauri/src/api/session/mod.rs",
    "shuji-app/src-tauri/src/api/control/mod.rs",
    "shuji-app/src-tauri/src/api/compact/mod.rs",
    "shuji-app/src-tauri/src/tool/registry.rs",
    "shuji-app/src-tauri/src/tool/dispatch/mod.rs",
    "shuji-app/src-tauri/src/pipeline/mod.rs",
    "shuji-app/src-tauri/src/audit/mod.rs",
    "shuji-app/src-tauri/src/config/mod.rs",
    "shuji-app/src-tauri/src/models/role.rs",
    "shuji-app/src-tauri/src/scenario/mod.rs",
];

/// Key prompt files that must exist for each agent.
const REQUIRED_PROMPT_FILES: &[&str] = &[
    "shuji-app/src-tauri/src/agent/neige/prompt.md",
    "shuji-app/src-tauri/src/agent/zhongshuling/prompt.md",
    "shuji-app/src-tauri/src/agent/menxiashizhong/prompt.md",
    "shuji-app/src-tauri/src/agent/shangshuling/prompt.md",
    "shuji-app/src-tauri/src/agent/libushangshu/prompt.md",
    "shuji-app/src-tauri/src/agent/bingbushangshu/prompt.md",
    "shuji-app/src-tauri/src/agent/gongbushangshu/prompt.md",
    "shuji-app/src-tauri/src/agent/xingbushangshu/prompt.md",
    "shuji-app/src-tauri/src/agent/liburshangshu/prompt.md",
];

/// Key frontend files that must exist.
const REQUIRED_FRONTEND_FILES: &[&str] = &[
    "shuji-app/src/api.ts",
    "shuji-app/src/types.ts",
    "shuji-app/src/hooks/useChat.ts",
    "shuji-app/src/components/ChatBubble.tsx",
    "shuji-app/src/components/ChatPanel.tsx",
    "shuji-app/src/components/DocPreview.tsx",
];

/// Verify all required source files exist.
#[test]
fn required_source_files_exist() {
    let root = project_root();
    let mut missing = Vec::new();
    for rel_path in REQUIRED_SOURCE_FILES {
        let full = root.join(rel_path);
        if !full.exists() {
            missing.push(rel_path.to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "Required source files are missing (docs may be out of sync):\n  {}",
        missing.join("\n  ")
    );
}

/// Verify all required prompt files exist.
#[test]
fn required_prompt_files_exist() {
    let root = project_root();
    let mut missing = Vec::new();
    for rel_path in REQUIRED_PROMPT_FILES {
        let full = root.join(rel_path);
        if !full.exists() {
            missing.push(rel_path.to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "Required prompt files are missing:\n  {}",
        missing.join("\n  ")
    );
}

/// Verify all required frontend files exist.
#[test]
fn required_frontend_files_exist() {
    let root = project_root();
    let mut missing = Vec::new();
    for rel_path in REQUIRED_FRONTEND_FILES {
        let full = root.join(rel_path);
        if !full.exists() {
            missing.push(rel_path.to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "Required frontend files are missing:\n  {}",
        missing.join("\n  ")
    );
}

/// Verify the scenario fixture exists and is loadable.
#[test]
fn scenario_fixture_exists_and_valid() {
    let root = project_root();
    let fixture = root.join("shuji-app/src-tauri/assets/scenarios/todo-cli-demo.json");
    assert!(
        fixture.exists(),
        "Scenario fixture missing: {}",
        fixture.display()
    );

    let content = std::fs::read_to_string(&fixture).expect("should read fixture");
    let scenario = shuji_app_lib::scenario::load_scenario(&content).expect("fixture should parse");
    assert!(!scenario.steps.is_empty(), "fixture should have steps");
    assert!(
        shuji_app_lib::scenario::validate_scenario(&scenario).is_ok(),
        "fixture should be valid"
    );
}

/// Guard against reintroduction of deprecated `AnthropicClient` in test files.
/// All tests should use `LlmClient` directly.
#[test]
fn no_deprecated_anthropic_client_in_tests() {
    let root = project_root();
    let tests_dir = root.join("shuji-app/src-tauri/tests");
    let mut violations = Vec::new();

    if let Ok(entries) = walk_dir(&tests_dir) {
        for path in entries {
            // Skip this test file itself
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if filename == "doc_guard_test.rs" {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "rs" {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            if let Ok(content) = std::fs::read_to_string(&path) {
                for (line_num, line) in content.lines().enumerate() {
                    // Skip comments
                    let trimmed = line.trim();
                    if trimmed.starts_with("//") || trimmed.starts_with("///") {
                        continue;
                    }
                    if line.contains("AnthropicClient") {
                        violations.push(format!("{}:{}", rel, line_num + 1));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found deprecated 'AnthropicClient' usage in test files (use 'LlmClient' instead):\n  {}",
        violations.join("\n  ")
    );
}

/// Recursively collect all files in a directory tree.
fn walk_dir(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(current) = dirs.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name != "target" && name != "node_modules" {
                    dirs.push(path);
                }
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}
