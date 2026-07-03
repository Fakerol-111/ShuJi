//! Guardrail tests to prevent reintroduction of deprecated patterns.
//!
//! These tests scan source and prompt files for markers that should only
//! appear in explicitly allowlisted locations. If a new `deprecated`,
//! `legacy`, or `route_to(` occurrence appears outside the allowlist,
//! these tests fail.
//!
//! Also scans for mojibake (GBK decode errors) in source code and prompt
//! files that could corrupt LLM interaction prompts. Known bad-text markers
//! are listed in MOJIBAKE_MARKERS; the only exceptions are this test file
//! and the cleanup plan document itself.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn project_root() -> PathBuf {
    // Walk up from CARGO_MANIFEST_DIR to find the repo root
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .expect("manifest has parent")
        .to_path_buf()
}

fn load_allowlist(root: &Path) -> Vec<String> {
    let path = root.join("scripts").join("DEPRECATED_ALLOWLIST");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    content
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            // Extract the file:marker part (before the first |)
            l.split('|').next().unwrap_or(l).trim().to_string()
        })
        .collect()
}

fn scan_prompt_files_for_route_to(root: &Path) -> Vec<String> {
    let agent_dir = root
        .join("shuji-app")
        .join("src-tauri")
        .join("src")
        .join("agent");
    let mut hits = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&agent_dir) {
        for entry in entries.flatten() {
            let dir_path = entry.path();
            if !dir_path.is_dir() {
                continue;
            }
            // Scan prompt.md and skills/*.md
            let mut paths_to_check = Vec::new();
            let prompt_md = dir_path.join("prompt.md");
            if prompt_md.exists() {
                paths_to_check.push(prompt_md);
            }
            let skills_dir = dir_path.join("skills");
            if skills_dir.is_dir() {
                if let Ok(skill_entries) = std::fs::read_dir(&skills_dir) {
                    for skill_entry in skill_entries.flatten() {
                        let p = skill_entry.path();
                        if p.extension().map(|e| e == "md").unwrap_or(false) {
                            paths_to_check.push(p);
                        }
                    }
                }
            }

            for path in &paths_to_check {
                if let Ok(content) = std::fs::read_to_string(path) {
                    for (line_num, line) in content.lines().enumerate() {
                        // Skip comment lines and lines that say "Do not call route_to"
                        if line.trim_start().starts_with('#')
                            || line.contains("Do not call route_to")
                            || line.contains("不要调用 route_to")
                        {
                            continue;
                        }
                        if line.contains("route_to(") {
                            let rel = path.strip_prefix(root).unwrap_or(path);
                            hits.push(format!("{}:{}", rel.display(), line_num + 1));
                        }
                    }
                }
            }
        }
    }
    hits
}

/// Assert that no prompt files contain actionable `route_to(` instructions.
/// These would tell agents to call a tool that no longer exists.
#[test]
fn no_stale_route_to_in_prompts() {
    let root = project_root();
    let hits = scan_prompt_files_for_route_to(&root);
    assert!(
        hits.is_empty(),
        "Prompt files still contain actionable route_to( instructions:\n  {}",
        hits.join("\n  ")
    );
}

/// Assert that no Rust source files contain active `route_to(` usage outside
/// the explicitly allowlisted compatibility files.
///
/// This prevents the legacy route_to pattern from spreading back into normal
/// agent orchestration code, pipeline steps, or new tool implementations.
#[test]
fn no_active_route_to_in_source_outside_allowlist() {
    let root = project_root();
    let allowlist = load_allowlist(&root);

    // Extract file patterns from allowlist entries that contain "route_to"
    let mut allowed_files: Vec<String> = Vec::new();
    for entry in &allowlist {
        if entry.contains("route_to") {
            if let Some(file_marker) = entry.split(':').next() {
                let file = file_marker.split(':').next().unwrap_or(file_marker);
                if !allowed_files.contains(&file.to_string()) {
                    allowed_files.push(file.to_string());
                }
            }
        }
    }

    // Hardcoded allowlist for files that reference "route_to" only in comments
    let comment_only_files = [
        "actor/spawn/output.rs",
        "commands/workflow/bootstrap.rs",
        "api/control/wrap_up.rs",
        "api/control/loop_runner.rs",
        "api/session/mod.rs",
        "api/control/routing.rs",
        "api/control/tool_exec.rs",
        "models/role.rs",
        "pipeline/artifacts.rs",
        "tool/audit_tools.rs",
        "workflow/graph.rs",
        "api/intent.rs",
        "api/session/response.rs",
    ];

    let src_dir = root.join("shuji-app").join("src-tauri").join("src");
    let mut violations = Vec::new();

    if let Ok(entries) = walk_dir(&src_dir) {
        for path in entries {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "rs" {
                continue;
            }

            let rel = path
                .strip_prefix(&src_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            // Skip allowlisted files
            if allowed_files.iter().any(|f| rel.contains(f)) {
                continue;
            }
            if comment_only_files.iter().any(|f| rel == *f) {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(&path) {
                for (line_num, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    // Skip comments and doc comments
                    if trimmed.starts_with("//")
                        || trimmed.starts_with("///")
                        || trimmed.starts_with("//!")
                        || trimmed.starts_with("*")
                    {
                        continue;
                    }
                    if line.contains("route_to(") || line.contains("route_to ") {
                        violations.push(format!("{}:{}", rel, line_num + 1));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found active route_to usage outside allowlist:\n  {}",
        violations.join("\n  ")
    );
}

/// Recursively collect all .rs files in a directory tree.
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

/// Assert that no `deprecated` or `legacy` markers appear in source code
/// outside explicitly allowlisted locations.
#[test]
fn deprecated_legacy_markers_are_allowlisted() {
    let root = project_root();
    let allowlist = load_allowlist(&root);

    // Build a map of file_pattern -> markers from allowlist
    let mut allowed: HashMap<String, Vec<String>> = HashMap::new();
    for entry in &allowlist {
        if let Some((file_marker, _rest)) = entry.split_once(':') {
            let parts: Vec<&str> = file_marker.splitn(2, ':').collect();
            if parts.len() == 2 {
                allowed
                    .entry(parts[0].to_string())
                    .or_default()
                    .push(parts[1].to_string());
            }
        }
    }

    // Scan source directories
    let scan_dirs = [
        root.join("shuji-app").join("src-tauri").join("src"),
        root.join("shuji-app").join("src"),
    ];

    let mut violations = Vec::new();

    for dir in &scan_dirs {
        if !dir.is_dir() {
            continue;
        }
        scan_dir_recursive(dir, &root, &allowed, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "Found non-allowlisted deprecated/legacy markers:\n  {}",
        violations.join("\n  ")
    );
}

fn scan_dir_recursive(
    dir: &Path,
    root: &Path,
    allowed: &HashMap<String, Vec<String>>,
    violations: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip target, node_modules, .shuji
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name == "node_modules" || name == ".shuji" || name == ".git" {
                continue;
            }
            scan_dir_recursive(&path, root, allowed, violations);
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "rs" && ext != "ts" && ext != "tsx" && ext != "md" {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        for (line_num, line) in content.lines().enumerate() {
            let lower = line.to_lowercase();
            if !lower.contains("deprecated") && !lower.contains("legacy") {
                continue;
            }

            // Skip comments and doc comments
            let trimmed = line.trim();
            if trimmed.starts_with("//")
                || trimmed.starts_with("///")
                || trimmed.starts_with("//!")
                || trimmed.starts_with("*")
                || trimmed.starts_with("#")
            {
                continue;
            }

            // Check if this file has any allowlist entries
            let has_allowlist = allowed.keys().any(|pattern| rel.contains(pattern));
            if has_allowlist {
                continue; // File is fully allowlisted (e.g., dispatch.rs)
            }

            violations.push(format!("{}:{}", rel, line_num + 1));
        }
    }
}

/// Known mojibake markers (GBK decode errors seen in this codebase).
/// These must NOT appear in source or prompt files outside the explicit
/// allowlist (this test file and the cleanup plan document).
const MOJIBAKE_MARKERS: &[&str] = &[
    "�",
    "鐨囧笣",
    "灏氫功浠",
    "涓功",
    "闂ㄤ笅",
    "鍐呴榿",
    "鈥?",
    "鈫?",
    "鈹€",
    "锛",
    "銆",
];

/// Files that are explicitly allowed to contain mojibake markers
/// (the test itself, the cleanup plan).
const MOJIBAKE_ALLOWLIST_SUFFIXES: &[&str] = &["pattern_guard_test.rs", "mojibake-cleanup-plan.md"];

/// Assert that no known mojibake markers appear outside the allowlist.
#[test]
fn no_mojibake_markers_in_source_and_prompts() {
    let root = project_root();
    let mut violations = Vec::new();

    let scan_subtrees = [
        "shuji-app/src-tauri/src",
        "shuji-app/src-tauri/tests",
        "shuji-app/src",
        "shuji-app/docs",
    ];

    for sub in &scan_subtrees {
        let dir = root.join(sub);
        if !dir.is_dir() {
            continue;
        }
        scan_mojibake_dir(&dir, &root, &mut violations);
    }

    // Root-level markdown files
    for fname in &["README.md", "CONTRIBUTING.md", "AGENTS.md"] {
        let path = root.join(fname);
        if path.exists() {
            scan_mojibake_file(&path, &root, &mut violations);
        }
    }

    assert!(
        violations.is_empty(),
        "Found mojibake markers outside allowlist:\n  {}",
        violations.join("\n  ")
    );
}

fn scan_mojibake_dir(dir: &Path, root: &Path, violations: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name == "node_modules" || name == ".shuji" || name == ".git" {
                continue;
            }
            scan_mojibake_dir(&path, root, violations);
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "rs" && ext != "ts" && ext != "tsx" && ext != "md" {
            continue;
        }

        // Skip files on the allowlist (the test itself and the cleanup plan doc)
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if MOJIBAKE_ALLOWLIST_SUFFIXES
            .iter()
            .any(|s| filename.ends_with(s))
        {
            continue;
        }

        scan_mojibake_file(&path, root, violations);
    }
}

fn scan_mojibake_file(path: &Path, root: &Path, violations: &mut Vec<String>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    for (line_num, line) in content.lines().enumerate() {
        for marker in MOJIBAKE_MARKERS {
            if line.contains(marker) {
                violations.push(format!("{}:{} (marker: {})", rel, line_num + 1, marker));
                break; // one violation per line
            }
        }
    }
}
