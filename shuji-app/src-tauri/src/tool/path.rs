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
    ("del", "file deletion command forbidden"),
    ("erase", "file deletion command forbidden"),
    ("rd", "directory deletion command forbidden"),
    ("rmdir", "directory deletion command forbidden"),
    ("remove-item", "PowerShell recursive deletion forbidden"),
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
    ("curl", "remote download/execution forbidden"),
    ("iwr", "remote download forbidden"),
    ("invoke-webrequest", "remote download forbidden"),
    ("powershell -enc", "encoded PowerShell execution forbidden"),
    (
        "powershell -encodedcommand",
        "encoded PowerShell execution forbidden",
    ),
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
    "$env:",
    "$home",
    "c:\\windows",
    "c:/windows",
    "\\\\",
];

/// Unix/Linux system path patterns for string-based detection.
pub const UNIX_PATH_ESCAPE: &[&str] = &[
    "/etc/",
    "/root/",
    "/proc/",
    "/sys/",
    "/dev/",
    "/boot/",
    "/sbin/",
    "/usr/sbin/",
];

/// Destructive command substrings checked on all platforms.
pub const DESTRUCTIVE_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -fr /",
    "rm -rf /*",
    "rm -fr /*",
    "remove-item",
    " -recurse",
    ":(){ :|:& };:",
];

/// Unix-specific command blocklist extensions.
pub const UNIX_SYSTEM_BLOCKS: &[(&str, &str)] = &[
    ("dd", "disk write operation forbidden"),
    ("chroot", "chroot privilege escalation forbidden"),
    ("chmod", "permission modification forbidden"),
];

const SHELL_CONTROL_PATTERNS: &[&str] = &["&&", "||", "|", ";", ">", "<", "`", "$("];

const ALLOWED_COMMAND_PREFIXES: &[&str] = &[
    "cargo test",
    "cargo check",
    "cargo clippy",
    "cargo fmt --check",
    "cargo build",
    "cargo --version",
    "rustc --version",
    "rustc --explain",
    "npm test",
    "npm run lint",
    "npm run format:check",
    "npm run build",
    "npm --version",
    "node --version",
    "npx vitest run",
    "npx jest",
    "python -m pytest",
    "python --version",
    "py -m pytest",
    "py --version",
    "ruff check",
    "ruff --version",
];

fn normalized_command(cmd: &str) -> String {
    cmd.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn is_allowed_command(lower: &str) -> bool {
    ALLOWED_COMMAND_PREFIXES.iter().any(|prefix| {
        lower == *prefix
            || lower
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(' '))
    })
}

/// Check if a shell command is safe to execute.
pub fn check_command_safety(cmd: &str) -> Result<(), &'static str> {
    let c = cmd.trim();
    let lower = normalized_command(c);
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(());
    }

    for pattern in SHELL_CONTROL_PATTERNS {
        if lower.contains(pattern) {
            return Err("shell control operators are forbidden");
        }
    }

    let blocklists: &[&[(&str, &str)]] = if cfg!(windows) {
        &[SYSTEM_BLOCKS]
    } else {
        &[SYSTEM_BLOCKS, UNIX_SYSTEM_BLOCKS]
    };

    for blocks in blocklists {
        for &(keyword, reason) in *blocks {
            let kw_tokens: Vec<&str> = keyword.split_whitespace().collect();

            let matched = if kw_tokens.len() == 1 {
                if keyword == "mkfs" {
                    tokens[0].starts_with("mkfs")
                } else {
                    tokens[0] == keyword
                }
            } else {
                tokens.len() >= kw_tokens.len() && tokens[..kw_tokens.len()] == kw_tokens[..]
            };

            if matched {
                return Err(reason);
            }
        }
    }

    let path_patterns: Vec<&str> = if cfg!(windows) {
        PATH_ESCAPE.to_vec()
    } else {
        PATH_ESCAPE
            .iter()
            .chain(UNIX_PATH_ESCAPE.iter())
            .copied()
            .collect()
    };

    for pattern in path_patterns {
        if lower.contains(pattern) {
            return Err("Operation on files outside the project directory is prohibited");
        }
    }

    for pattern in DESTRUCTIVE_PATTERNS {
        if lower.contains(pattern) {
            return Err("destructive system operation forbidden");
        }
    }

    if !is_allowed_command(&lower) {
        return Err(
            "command is not in the restricted allowlist; use run_tests/run_lint or an approved build/format command",
        );
    }

    Ok(())
}

#[cfg(test)]
mod command_safety_tests {
    use super::check_command_safety;

    #[test]
    fn blocks_sudo() {
        assert!(check_command_safety("sudo rm -rf /").is_err());
    }

    #[test]
    #[cfg(not(windows))]
    fn blocks_rm_rf_root() {
        assert!(check_command_safety("rm -rf /").is_err());
    }

    #[test]
    #[cfg(not(windows))]
    fn blocks_dd() {
        assert!(check_command_safety("dd if=/dev/zero of=/dev/sda").is_err());
    }

    #[test]
    #[cfg(not(windows))]
    fn blocks_etc_access() {
        assert!(check_command_safety("cat /etc/passwd").is_err());
    }

    #[test]
    fn allows_project_relative_commands() {
        assert!(check_command_safety("cargo test --lib").is_ok());
        assert!(check_command_safety("npm test").is_ok());
    }

    #[test]
    fn blocks_shell_control_operators() {
        assert!(check_command_safety("cargo test && del important.txt").is_err());
        assert!(check_command_safety("npm test; Remove-Item file").is_err());
        assert!(check_command_safety("cargo test | powershell -enc aaa").is_err());
    }

    #[test]
    fn blocks_case_insensitive_dangerous_commands() {
        assert!(check_command_safety("SUDO rm -rf /").is_err());
        assert!(check_command_safety("PoWeRsHeLl -EncodedCommand AAA").is_err());
        assert!(check_command_safety("Invoke-WebRequest https://example.com/a.ps1").is_err());
    }

    #[test]
    fn blocks_unknown_commands_by_default() {
        assert!(check_command_safety("python setup.py install").is_err());
        assert!(check_command_safety("git status").is_err());
    }
}
