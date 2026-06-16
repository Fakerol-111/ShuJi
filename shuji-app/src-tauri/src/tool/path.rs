use std::path::{Path, PathBuf};

/// Resolve a project-relative path against root with safety checks.
///
/// - Canonicalizes root for reliable comparison (handles Windows `\\?\` prefix)
/// - Rejects absolute paths and `..` traversal
/// - Canonicalizes existing paths to detect symlink escapes
/// - Returns error if resolved path is not within `root`
///
/// For files that don't exist yet (write operations), canonicalizes
/// the parent directory and then appends the filename.
pub async fn resolve_scoped_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    // Canonicalize root once for reliable comparison across all code paths.
    // This handles Windows `\\?\` prefix, symlinks, and path normalization.
    let canon_root = tokio::fs::canonicalize(root)
        .await
        .map_err(|e| format!("project root resolution failed: {}", e))?;

    let rel_path = Path::new(rel);

    // Block absolute paths
    if rel_path.is_absolute() {
        return Err(format!("absolute paths forbidden: {}", rel));
    }

    // Block .. traversal (use path components, not string match)
    if rel_path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(format!("parent directory traversal forbidden: {}", rel));
    }

    // Block Windows drive-letter / UNC prefix paths (C:, \\server, etc.)
    for comp in rel_path.components() {
        if matches!(comp, std::path::Component::Prefix(_)) {
            return Err(format!("drive letter or UNC path forbidden: {}", rel));
        }
    }

    let candidate = root.join(rel_path);

    // For existing paths, canonicalize to detect escapes
    if candidate.exists() {
        let canon = tokio::fs::canonicalize(&candidate)
            .await
            .map_err(|e| format!("path resolution failed {}: {}", rel, e))?;

        if !canon.starts_with(&canon_root) {
            return Err(format!(
                "path out of bounds: {} resolves to {}, not within project directory",
                rel,
                canon.display()
            ));
        }
        return Ok(canon);
    }

    // For non-existing paths, canonicalize parent directory
    if let Some(parent) = candidate.parent() {
        if parent.exists() {
            let canon_parent = tokio::fs::canonicalize(parent)
                .await
                .map_err(|e| format!("parent directory resolution failed {}: {}", rel, e))?;

            if !canon_parent.starts_with(&canon_root) {
                return Err(format!(
                    "path out of bounds: parent directory of {} is not within project directory",
                    rel,
                ));
            }

            let filename = candidate
                .file_name()
                .ok_or_else(|| format!("invalid filename: {}", rel))?;

            return Ok(canon_parent.join(filename));
        }
    }

    // Parent doesn't exist yet — can't canonicalize the full path.
    // Walk up to find the longest existing ancestor, canonicalize it,
    // verify it's within the project root, then reconstruct the path.
    for ancestor in candidate.ancestors() {
        if ancestor.exists() {
            let canon_ancestor = tokio::fs::canonicalize(ancestor)
                .await
                .map_err(|e| format!("parent resolution failed {}: {}", rel, e))?;
            if !canon_ancestor.starts_with(&canon_root) {
                return Err(format!("path out of bounds: {}", rel));
            }
            let suffix = candidate
                .strip_prefix(ancestor)
                .map_err(|_| format!("path resolution internal error: {}", rel))?;
            return Ok(canon_ancestor.join(suffix));
        }
    }

    // Nothing in the path exists. Since rel is already sanitized
    // (no .., no absolute, no prefix components), root.join(rel) is
    // guaranteed to be within root. Use canon_root as anchor so
    // Windows normalization (\\?\ prefix, casing) is applied.
    Ok(canon_root.join(rel))
}

/// Command blocklist: (keyword, reason) tuples for `check_safe_command`.
/// System-level commands that are dangerous in any project context.
pub const SYSTEM_BLOCKS: &[(&str, &str)] = &[
    ("format", "formatting disk forbidden"),
    ("mkfs", "formatting disk forbidden"),
    ("fdisk", "modifying partition table forbidden"),
    ("diskpart", "modifying disk partitions forbidden"),
    ("shutdown", "shutdown/reboot system forbidden"),
    ("reboot", "shutdown/reboot system forbidden"),
    ("restart-computer", "reboot system forbidden"),
    ("stop-computer", "shutdown system forbidden"),
    ("poweroff", "shutdown system forbidden"),
    ("halt", "shutdown system forbidden"),
    ("sudo", "sudo privilege escalation forbidden"),
    ("runas", "elevated execution forbidden"),
    ("takeown", "taking file ownership forbidden"),
    ("reg delete", "modifying registry forbidden"),
    ("reg add", "modifying registry forbidden"),
    ("sc delete", "deleting services forbidden"),
    ("net user", "managing user accounts forbidden"),
    ("net localgroup", "managing user groups forbidden"),
    ("cacls", "modifying file permissions forbidden"),
    ("wget", "remote download/execution forbidden"),
    ("powershell -enc", "encoded PowerShell execution forbidden"),
    ("certutil -urlcache", "remote download forbidden"),
    ("bitsadmin /transfer", "remote download forbidden"),
    ("mshta", "MSHTA script execution forbidden"),
    ("npm install -g", "global install forbidden"),
];

/// Path escape patterns: strings that indicate an attempt to access
/// files outside the project directory.
pub const PATH_ESCAPE: &[&str] = &[
    "..\\",
    "../",
    "/windows",
    "/windows/system32",
    "/program files",
    "/programdata",
    "/users",
    "%systemroot%",
    "%windir%",
    "%appdata%",
    "%programfiles%",
];
