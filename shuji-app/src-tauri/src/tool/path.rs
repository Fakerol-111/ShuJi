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
        .map_err(|e| format!("项目根目录解析失败: {}", e))?;

    let rel_path = Path::new(rel);

    // Block absolute paths
    if rel_path.is_absolute() {
        return Err(format!("禁止使用绝对路径: {}", rel));
    }

    // Block .. traversal (use path components, not string match)
    if rel_path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(format!("禁止使用父目录跳转: {}", rel));
    }

    // Block Windows drive-letter / UNC prefix paths (C:, \\server, etc.)
    for comp in rel_path.components() {
        if matches!(comp, std::path::Component::Prefix(_)) {
            return Err(format!("禁止使用盘符或 UNC 路径: {}", rel));
        }
    }

    let candidate = root.join(rel_path);

    // For existing paths, canonicalize to detect escapes
    if candidate.exists() {
        let canon = tokio::fs::canonicalize(&candidate)
            .await
            .map_err(|e| format!("路径解析失败 {}: {}", rel, e))?;

        if !canon.starts_with(&canon_root) {
            return Err(format!(
                "路径越界: {} 解析到 {}，不在项目目录内",
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
                .map_err(|e| format!("父目录解析失败 {}: {}", rel, e))?;

            if !canon_parent.starts_with(&canon_root) {
                return Err(format!("路径越界: {} 的父目录不在项目目录内", rel,));
            }

            let filename = candidate
                .file_name()
                .ok_or_else(|| format!("无效文件名: {}", rel))?;

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
                .map_err(|e| format!("父目录解析失败 {}: {}", rel, e))?;
            if !canon_ancestor.starts_with(&canon_root) {
                return Err(format!("路径越界: {}", rel));
            }
            let suffix = candidate
                .strip_prefix(ancestor)
                .map_err(|_| format!("路径解析内部错误: {}", rel))?;
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
    ("format", "禁止格式化磁盘"),
    ("mkfs", "禁止格式化磁盘"),
    ("fdisk", "禁止修改分区表"),
    ("diskpart", "禁止修改磁盘分区"),
    ("shutdown", "禁止关闭/重启系统"),
    ("reboot", "禁止关闭/重启系统"),
    ("restart-computer", "禁止重启系统"),
    ("stop-computer", "禁止关闭系统"),
    ("poweroff", "禁止关闭系统"),
    ("halt", "禁止关闭系统"),
    ("sudo", "禁止使用sudo提权"),
    ("runas", "禁止提权运行"),
    ("takeown", "禁止夺取文件所有权"),
    ("reg delete", "禁止修改注册表"),
    ("reg add", "禁止修改注册表"),
    ("sc delete", "禁止删除服务"),
    ("net user", "禁止管理用户账户"),
    ("net localgroup", "禁止管理用户组"),
    ("cacls", "禁止修改文件权限"),
    ("wget", "禁止远程下载执行"),
    ("powershell -enc", "禁止编码执行PowerShell"),
    ("certutil -urlcache", "禁止远程下载"),
    ("bitsadmin /transfer", "禁止远程下载"),
    ("mshta", "禁止执行MSHTA脚本"),
    ("npm install -g", "禁止全局安装"),
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
