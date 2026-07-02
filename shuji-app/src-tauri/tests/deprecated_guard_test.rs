//! Guardrail tests to prevent reintroduction of deprecated patterns.
//!
//! These tests scan source and prompt files for markers that should only
//! appear in explicitly allowlisted locations. If a new `deprecated`,
//! `legacy`, or `route_to(` occurrence appears outside the allowlist,
//! these tests fail.

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
