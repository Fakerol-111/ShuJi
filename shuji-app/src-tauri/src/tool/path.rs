use std::path::{Component, Path, PathBuf};

/// Strip the Windows verbatim prefix from a canonicalized
/// path if present.  This ensures consistent path comparisons
/// regardless of whether canonicalize adds the prefix.
fn normalize_canonical(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path
}

/// Compare two paths component-by-component, case-insensitively
/// on Windows.  Rust Path::starts_with compares Normal components
/// case-insensitively on Windows, but Prefix components (drive
/// letters) are compared as-is.  This helper does a full
/// case-insensitive string comparison per component.
fn safe_starts_with(path: &Path, prefix: &Path) -> bool {
    let path_components: Vec<_> = path.components().collect();
    let prefix_components: Vec<_> = prefix.components().collect();

    if path_components.len() < prefix_components.len() {
        return false;
    }

    #[cfg(windows)]
    {
        path_components[..prefix_components.len()]
            .iter()
            .zip(&prefix_components)
            .all(|(a, b)| {
                a.as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
            })
    }
    #[cfg(not(windows))]
    {
        path_components[..prefix_components.len()] == prefix_components[..]
    }
}

/// Resolve a project-relative path against root with safety checks.
///
/// - Rejects absolute paths and .. traversal
/// - Canonicalizes existing paths to detect symlink escapes
/// - For non-existing paths, input validation alone is sufficient
/// - Uses safe component-level path comparisons
pub async fn resolve_scoped_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel_path = Path::new(rel);

    if rel_path.is_absolute() {
        return Err(format!("absolute paths forbidden: {}", rel));
    }

    if rel_path.components().any(|c| c == Component::ParentDir) {
        return Err(format!("parent directory traversal forbidden: {}", rel));
    }

    for comp in rel_path.components() {
        if matches!(comp, Component::Prefix(_)) {
            return Err(format!("drive letter or UNC path forbidden: {}", rel));
        }
    }

    let candidate = root.join(rel_path);

    if candidate.exists() {
        let canon_root = normalize_canonical(
            tokio::fs::canonicalize(root)
                .await
                .map_err(|e| format!("project root canonicalization failed: {}", e))?,
        );
        let canon = normalize_canonical(
            tokio::fs::canonicalize(&candidate)
                .await
                .map_err(|e| format!("path canonicalization failed {}: {}", rel, e))?,
        );

        if !safe_starts_with(&canon, &canon_root) {
            return Err(format!(
                "path out of bounds: {} resolves to {},",
                rel,
                canon.display()
            ) + " not within project directory");
        }
        return Ok(canon);
    }

    // For non-existing paths, validation above guarantees
    // that root.join(rel) is within root.
    Ok(candidate)
}

/// Command blocklist: (keyword, reason) tuples
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

/// Path escape patterns for string-based detection.
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
