use crate::actor::FastMessage;
use crate::api::client::AnthropicClient;
use crate::models::role::Role;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

pub mod documents;
pub mod registry;
mod tool_log;

// ── Tool context for special tools ─────────────────────────────────

/// Optional context passed to special tools that need access to system
/// resources beyond the filesystem (API client, cancel map, etc.).
/// Currently used by 内阁's special tools (cancel_agent, update_soul,
/// expand_requirements, create_skill).
pub struct ToolContext {
    pub working_dir: PathBuf,
    pub cancel_map: Option<Arc<Mutex<HashMap<Role, Arc<AtomicBool>>>>>,
    pub client: Option<Arc<AnthropicClient>>,
    pub model: Option<String>,
    /// Fast mailbox senders for interrupting departments immediately.
    pub fast_txs: Option<Arc<HashMap<Role, mpsc::UnboundedSender<FastMessage>>>>,
}

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

// ── Structured tool result ───────────────────────────────────────────

/// Structured tool result returned to the LLM as JSON.
/// Helps the model reliably determine operation outcomes.
#[derive(Debug, Serialize)]
pub struct ToolOutput {
    pub ok: bool,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

impl ToolOutput {
    fn new(ok: bool, operation: &str) -> Self {
        Self {
            ok,
            operation: operation.to_string(),
            path: None,
            message: None,
            error_code: None,
        }
    }

    pub fn success(operation: &str, path: &str, message: &str) -> String {
        let mut o = Self::new(true, operation);
        o.path = Some(path.to_string());
        o.message = Some(message.to_string());
        serde_json::to_string(&o).unwrap_or_else(|_| {
            format!(
                "{{\"ok\":true,\"operation\":\"{}\",\"message\":\"{}\"}}",
                operation, message
            )
        })
    }

    pub fn success_raw(operation: &str, message: &str) -> String {
        let mut o = Self::new(true, operation);
        o.message = Some(message.to_string());
        serde_json::to_string(&o).unwrap_or_else(|_| {
            format!(
                "{{\"ok\":true,\"operation\":\"{}\",\"message\":\"{}\"}}",
                operation, message
            )
        })
    }

    pub fn read_file(operation: &str, path: &str, content: &str) -> String {
        let mut o = Self::new(true, operation);
        o.path = Some(path.to_string());
        o.message = Some(format!(
            "共 {} 字节。内容如下：\n{}",
            content.len(),
            content
        ));
        serde_json::to_string(&o).unwrap_or_else(|_| content.to_string())
    }

    pub fn error(operation: &str, path: &str, code: &str, message: &str) -> String {
        let mut o = Self::new(false, operation);
        o.path = Some(path.to_string());
        o.error_code = Some(code.to_string());
        o.message = Some(message.to_string());
        serde_json::to_string(&o).unwrap_or_else(|_| {
            format!("{{\"ok\":false,\"operation\":\"{}\",\"path\":\"{}\",\"error_code\":\"{}\",\"message\":\"{}\"}}",
                operation, path, code, message)
        })
    }
}

// ── append_file helper ─────────────────────────────────────────────

/// Append content to an existing file. Creates the file if it doesn't exist.
pub async fn tool_append_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("append_file", "", "empty_path", "文件路径为空");
    }
    if content.is_empty() {
        return ToolOutput::error("append_file", path, "empty_content", "追加内容为空");
    }
    // Hard limit: 2000 characters per call
    if content.len() > 2000 {
        return ToolOutput::error("append_file", path, "content_too_long",
            &format!("content 长度 {} 超过上限 2000 字符。请拆分成多次 append_file 调用，每次 ≤2000 字符。", content.len()));
    }
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("append_file", path, "path_error", &e),
    };
    // Create parent directories if needed
    if let Some(parent) = full.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&full)
        .await
    {
        Ok(mut file) => {
            use tokio::io::AsyncWriteExt;
            if let Err(e) = file.write_all(format!("{}\n", content).as_bytes()).await {
                return ToolOutput::error("append_file", path, "write_error", &e.to_string());
            }
            ToolOutput::success("append_file", path, "追加成功")
        }
        Err(e) => ToolOutput::error("append_file", path, "open_error", &e.to_string()),
    }
}

/// Helper: generate a ToolDefinition for append_file.
pub fn append_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "append_file".into(),
            description: "追加内容到文件末尾。content ≤2000 字符，大文件分批写入。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径，相对于项目根目录"
                    },
                    "content": {
                        "type": "string",
                        "description": "要追加的内容（每次最多 2000 字符）",
                        "maxLength": 2000
                    }
                },
                "required": ["path", "content"]
            }),
        },
    }
}

// ── delete_file helper ────────────────────────────────────────────

/// Delete a file. Returns an error if the path doesn't exist or is a directory.
pub async fn tool_delete_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("delete_file", "", "empty_path", "文件路径为空");
    }
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("delete_file", path, "path_error", &e),
    };
    if !full.exists() {
        return ToolOutput::error("delete_file", path, "not_found", "文件不存在");
    }
    if full.is_dir() {
        return ToolOutput::error(
            "delete_file",
            path,
            "is_directory",
            "不能删除目录，请使用文件路径",
        );
    }
    match tokio::fs::remove_file(&full).await {
        Ok(_) => ToolOutput::success("delete_file", path, "删除成功"),
        Err(e) => ToolOutput::error("delete_file", path, "delete_error", &e.to_string()),
    }
}

/// Generate a ToolDefinition for delete_file.
pub fn delete_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "delete_file".into(),
            description: "删除项目目录下的文件".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径，相对于项目根目录"
                    }
                },
                "required": ["path"]
            }),
        },
    }
}

// ── rename_file helper ────────────────────────────────────────────

/// Rename or move a file. Takes source path and destination path.
pub async fn tool_rename_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let from = args["from"].as_str().unwrap_or("");
    let to = args["to"].as_str().unwrap_or("");
    if from.is_empty() || to.is_empty() {
        return ToolOutput::error("rename_file", "", "empty_path", "from 和 to 都不能为空");
    }
    let full_from = match resolve_scoped_path(working_dir, from).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("rename_file", from, "path_error", &e),
    };
    if !full_from.exists() {
        return ToolOutput::error("rename_file", from, "not_found", "源文件不存在");
    }
    let full_to = match resolve_scoped_path(working_dir, to).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("rename_file", to, "path_error", &e),
    };
    if let Some(parent) = full_to.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::rename(&full_from, &full_to).await {
        Ok(_) => ToolOutput::success("rename_file", to, &format!("从 {} 重命名成功", from)),
        Err(e) => ToolOutput::error("rename_file", to, "rename_error", &e.to_string()),
    }
}

/// Generate a ToolDefinition for rename_file.
pub fn rename_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "rename_file".into(),
            description: "重命名或移动文件。提供 from（源路径）和 to（目标路径）".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "from": {
                        "type": "string",
                        "description": "原文件路径，相对于项目根目录"
                    },
                    "to": {
                        "type": "string",
                        "description": "新文件路径，相对于项目根目录"
                    }
                },
                "required": ["from", "to"]
            }),
        },
    }
}

// ── apply_patch helper ────────────────────────────────────────

/// Apply a unified diff patch to an existing file using the `diffy` crate.
/// The patch format is standard unified diff (`diff -u old new`).
/// Supports both creating new files (patch to /dev/null) and modifying
/// existing files.  Returns the full patched file content on success.
pub async fn tool_apply_patch(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let patch_str = args["patch"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("apply_patch", "", "empty_path", "文件路径为空");
    }
    if patch_str.is_empty() {
        return ToolOutput::error("apply_patch", path, "empty_patch", "patch 内容为空");
    }
    // Hard limit: 50KB per patch
    if patch_str.len() > 50_000 {
        return ToolOutput::error(
            "apply_patch",
            path,
            "patch_too_large",
            &format!(
                "patch 长度 {} 超过上限 50000 字符。请拆分成多次 apply_patch 调用。",
                patch_str.len()
            ),
        );
    }

    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("apply_patch", path, "path_error", &e),
    };

    // Ensure parent dir exists
    if let Some(parent) = full.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    // Read current content (empty string if file doesn't exist — /dev/null case)
    let old_content = tokio::fs::read_to_string(&full).await.unwrap_or_default();

    // Parse and apply the unified diff
    let patch = match diffy::Patch::from_str(patch_str) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("apply_patch", path, "patch_parse_error",
            &format!("patch 解析失败: {}\n\npatch 必须是 unified diff 格式（diff -u 输出）。请使用 --- a/file 和 +++ b/file 头。", e)),
    };

    let new_content = match diffy::apply(&old_content, &patch) {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("apply_patch", path, "patch_apply_error",
            &format!("patch 应用失败: {}。请确认 patch 的上下文行与原文件内容一致。可先用 read_file 确认最新内容再生成 patch。", e)),
    };

    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => ToolOutput::success(
            "apply_patch",
            path,
            &format!(
                "patch 应用成功（{} 字节 → {} 字节）",
                old_content.len(),
                new_content.len()
            ),
        ),
        Err(e) => ToolOutput::error("apply_patch", path, "write_error", &e.to_string()),
    }
}

/// Generate a ToolDefinition for apply_patch.
pub fn apply_patch_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "apply_patch".into(),
            description:
                "应用 unified diff patch 到文件。适用于任何幅度的修改：大段替换（>5行）、重构、批量修改。patch 格式为 `diff -u` 输出。注意：patch 的 ---/+++ 行指定旧/新文件名，系统会自动映射到工作目录。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径，相对于项目根目录。patch 中的 ---/+++ 文件名会被忽略，实际修改此路径指定的文件。"
                    },
                    "patch": {
                        "type": "string",
                        "description": "unified diff 格式的 patch 内容",
                    }
                },
                "required": ["path", "patch"]
            }),
        },
    }
}

// ── modify_file helper ─────────────────────────────────────────

/// Modify an existing file by replacing matching text (find+replace).
/// Reads the file, finds `old_text`, replaces it with `new_text` (first
/// occurrence only).  The LLM should use read_file first to locate the
/// exact text to replace.
pub async fn tool_modify_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("modify_file", "", "empty_path", "文件路径为空");
    }
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("modify_file", path, "path_error", &e),
    };
    if !full.exists() {
        return ToolOutput::error("modify_file", path, "not_found", "文件不存在");
    }
    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("modify_file", path, "read_error", &e.to_string()),
    };

    let old_text = args["old_text"].as_str().unwrap_or("");
    let new_text = args["new_text"].as_str().unwrap_or("");
    if old_text.is_empty() {
        return ToolOutput::error("modify_file", path, "empty_old_text", "old_text 不能为空");
    }
    // Hard limit: 800 characters per parameter
    if old_text.len() > 800 {
        return ToolOutput::error("modify_file", path, "old_text_too_long",
            &format!("old_text 长度 {} 超过上限 800 字符。对于大块修改，请使用 read_file → delete_file → create_file 模式。", old_text.len()));
    }
    if new_text.len() > 800 {
        return ToolOutput::error("modify_file", path, "new_text_too_long",
            &format!("new_text 长度 {} 超过上限 800 字符。对于大块修改，请使用 read_file → delete_file → create_file 模式。", new_text.len()));
    }
    if !content.contains(old_text) {
        return ToolOutput::error("modify_file", path, "not_found",
            "未在文件中找到匹配的文本。请先用 read_file 确认文件内容，并确保 old_text 与原文件完全一致（包括空格和缩进）。");
    }

    let new_content = content.replacen(old_text, new_text, 1);
    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => ToolOutput::success(
            "modify_file",
            path,
            &format!("替换成功（替换 {} 字节）", old_text.len()),
        ),
        Err(e) => ToolOutput::error("modify_file", path, "write_error", &e.to_string()),
    }
}

/// Generate a ToolDefinition for modify_file.
pub fn modify_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "modify_file".into(),
            description:
                "替换文件中的文本 (find+replace)。≤800字符。大块修改用 read→delete→create。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径，相对于项目根目录"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "待替换文本（≤800字符）",
                        "maxLength": 800
                    },
                    "new_text": {
                        "type": "string",
                        "description": "新文本（≤800字符）",
                        "maxLength": 800
                    }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        },
    }
}

// ── execute_command helper ─────────────────────────────────────────

/// Execute a command with safety checks and timeout.
/// Used by 兵部 and 刑部 for running test commands.
pub async fn tool_execute_command(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let cmd = args["command"].as_str().unwrap_or("");
    if cmd.is_empty() {
        return ToolOutput::error("execute_command", "", "empty_command", "命令为空");
    }
    log_console!("[{}] executing: {}", dept, cmd);

    if let Err(blocked) = check_safe_command(cmd) {
        log_console!("[{}] BLOCKED command: {} — reason: {}", dept, cmd, blocked);
        return format!("[安全拦截] 命令被禁止执行: {}\n原因: {}", cmd, blocked);
    }

    let timeout = std::time::Duration::from_secs(120);
    let (shell, shell_args) = if cfg!(windows) {
        ("powershell", vec!["-Command"])
    } else {
        ("bash", vec!["-l", "-c"])
    };
    match execute_with_timeout(shell, &shell_args, cmd, working_dir, timeout).await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code().unwrap_or(-1);
            if exit_code == 0 {
                format!("命令执行成功 (exit={}):\n{}", exit_code, stdout)
            } else {
                format!(
                    "命令执行失败 (exit={}):\nstdout:\n{}\nstderr:\n{}",
                    exit_code, stdout, stderr
                )
            }
        }
        Err(timeout_msg) => timeout_msg,
    }
}

async fn execute_with_timeout(
    shell: &str,
    args: &[&str],
    cmd: &str,
    working_dir: &Path,
    timeout: std::time::Duration,
) -> Result<std::process::Output, String> {
    let mut child = tokio::process::Command::new(shell);
    for a in args {
        child.arg(a);
    }
    child.arg(cmd);
    let mut child = child
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动命令: {}", e))?;

    let start = tokio::time::Instant::now();
    let poll_interval = tokio::time::Duration::from_millis(500);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(ref mut out) = child.stdout {
                    let _ = out.read_to_end(&mut stdout).await;
                }
                if let Some(ref mut err) = child.stderr {
                    let _ = err.read_to_end(&mut stderr).await;
                }
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    return Err(format!(
                        "命令执行超时（超过 {} 秒），进程已终止。",
                        timeout.as_secs()
                    ));
                }
                tokio::time::sleep(poll_interval).await;
            }
            Err(e) => return Err(format!("命令执行出错: {}", e)),
        }
    }
}

fn check_safe_command(cmd: &str) -> Result<(), &'static str> {
    let c = cmd.trim();
    let tokens: Vec<&str> = c.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(());
    }

    for &(keyword, reason) in SYSTEM_BLOCKS {
        let kw_tokens: Vec<&str> = keyword.split_whitespace().collect();

        let matched = if kw_tokens.len() == 1 {
            // Single-token: match first command token (exact, or prefix for mkfs).
            if keyword == "mkfs" {
                tokens[0].starts_with("mkfs")
            } else {
                tokens[0] == keyword
            }
        } else {
            // Multi-token: match prefix of command tokens.
            tokens.len() >= kw_tokens.len() && tokens[..kw_tokens.len()] == kw_tokens[..]
        };

        if matched {
            return Err(reason);
        }
    }

    for &pattern in PATH_ESCAPE {
        if c.to_lowercase().contains(pattern) {
            return Err("禁止操作项目目录之外的文件");
        }
    }
    Ok(())
}

// ── Unified tool implementations (used by all agents) ─────────────

/// Read a file with optional line range (`offset`, `limit`).
/// Files over 200 lines require offset/limit to prevent token overflow.
pub async fn tool_read_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let offset = args["offset"].as_u64().unwrap_or(0);
    let limit = args["limit"].as_u64().unwrap_or(u64::MAX);
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("read_file", path, "path_error", &e),
    };
    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("read_file", path, "read_error", &e.to_string()),
    };

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    // Reject full read for large files unless offset/limit are specified
    let is_chunked = offset > 0 || limit < u64::MAX;
    if !is_chunked && total > 200 {
        return ToolOutput::error("read_file", path, "too_large",
            &format!("文件过大（共 {} 行）。请用 offset 和 limit 参数分段读取，如 offset=0&limit=50。或用 edit_file 做局部修改。", total));
    }

    let start = (offset as usize).min(total);
    let end = (start + (limit as usize).min(total - start)).min(total);
    let excerpt: Vec<String> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>4}| {}", start + i + 1, line))
        .collect();
    let meta = format!("{}（共 {} 行，显示 {}-{}）", path, total, start + 1, end);
    let result = format!("{}\n{}", meta, excerpt.join("\n"));

    ToolOutput::read_file("read_file", path, &result)
}

/// Create a new file with initial content. Rejects if the file already exists
/// (use modify_file or delete+create instead).
pub async fn tool_create_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("create_file", "", "empty_path", "文件路径为空");
    }
    // Hard limit: 8000 characters per call
    if content.len() > 8000 {
        return ToolOutput::error(
            "create_file",
            path,
            "content_too_long",
            &format!(
                "content 长度 {} 超过上限 8000 字符。请使用 create_file + append_file 分块写入。",
                content.len()
            ),
        );
    }

    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("create_file", path, "path_error", &e),
    };

    if full.exists() {
        return ToolOutput::error("create_file", path, "already_exists",
            "文件已存在，不允许覆盖。请使用 modify_file 修改内容，或先 delete_file 再 create_file。");
    }

    if let Some(parent) = full.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::write(&full, content).await {
        Ok(_) => ToolOutput::success("create_file", path, "写入成功"),
        Err(e) => ToolOutput::error("create_file", path, "write_error", &e.to_string()),
    }
}

/// List directory contents.
pub async fn tool_list_dir(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("list_dir", path, "path_error", &e),
    };

    // Use spawn_blocking for directory listing since it involves sync iteration
    // of DirEntry results, which doesn't map cleanly to tokio::fs::ReadDir.
    let full_for_blocking = full.clone();
    match tokio::task::spawn_blocking(move || std::fs::read_dir(&full_for_blocking)).await {
        Ok(Ok(entries)) => {
            let items: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| {
                    let tag = e
                        .file_type()
                        .map(|t| if t.is_dir() { "[DIR]" } else { "[FILE]" })
                        .unwrap_or("[?]");
                    format!("{} {}", tag, e.file_name().to_string_lossy())
                })
                .collect();
            let message = if items.is_empty() {
                "(空目录)".to_string()
            } else {
                items.join("\n")
            };
            ToolOutput::success_raw("list_dir", &message)
        }
        Ok(Err(e)) => ToolOutput::error("list_dir", path, "list_error", &e.to_string()),
        Err(_) => ToolOutput::error("list_dir", path, "join_error", "后台任务异常"),
    }
}

// ── Unified tool definitions (parameterized descriptions) ────────

pub fn read_file_tool_def(description: &str) -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "read_file".into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径，相对于项目根目录"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "起始行号（从 0 开始），不填则从开头读"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "最多读取的行数。大文件必须用此参数分段读取"
                    }
                },
                "required": ["path"]
            }),
        },
    }
}

pub fn create_file_tool_def(description: &str) -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_file".into(),
            description: format!(
                "{}。content ≤8000 字符。大文件用 apply_patch 一次性写入。",
                description
            ),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径，相对于项目根目录"
                    },
                    "content": {
                        "type": "string",
                        "description": "文件内容（≤8000字符）",
                        "maxLength": 8000
                    }
                },
                "required": ["path", "content"]
            }),
        },
    }
}

pub fn list_dir_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "list_dir".into(),
            description: "列出目录下的文件和子目录".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "目录路径，相对于项目根目录。空字符串列出根目录"
                    }
                },
                "required": ["path"]
            }),
        },
    }
}

/// Read `.shuji/logs/activity.log`, parse JSON lines, return as formatted text.
/// Lines are naturally chronological since it's a single append-only file.
pub async fn tool_summarize_logs(working_dir: &Path, args: &serde_json::Value) -> String {
    let log_path = working_dir.join(".shuji").join("logs").join("activity.log");
    if !log_path.exists() {
        return ToolOutput::success_raw("summarize_logs", "暂无日志记录");
    }

    let content = match tokio::fs::read_to_string(&log_path).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("summarize_logs", "", "read_error", &e.to_string()),
    };

    let since = args["since"].as_u64().unwrap_or(0) as usize;
    let mut entries: Vec<serde_json::Value> = Vec::new();
    let total_lines;

    {
        let lines: Vec<&str> = content.lines().collect();
        total_lines = lines.len();
        for line in lines.iter().skip(since) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                entries.push(val);
            }
        }
    }

    if entries.is_empty() {
        return ToolOutput::success_raw("summarize_logs", "暂无日志记录");
    }

    let mut result = Vec::new();
    result.push(format!(
        "共 {} 条日志记录（文件共 {} 行，自第 {} 行开始）：",
        entries.len(),
        total_lines,
        since
    ));
    result.push(String::new());

    for entry in &entries {
        let ts = entry["ts"].as_str().unwrap_or("?");
        let author = entry["author"].as_str().unwrap_or("?");
        let summary = entry["summary"].as_str().unwrap_or("");

        let short_ts = if ts.len() > 19 { &ts[..19] } else { ts };
        result.push(format!("[{}] {}: {}", short_ts, author, summary));
    }

    ToolOutput::success_raw("summarize_logs", &result.join("\n"))
}

/// Tool definition for summarize_logs.
pub fn summarize_logs_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "summarize_logs".into(),
            description: "读取 activity.log 日志，可按行号增量读取".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "since": {
                        "type": "integer",
                        "description": "起始行号（从 0 开始），不传则从开头读"
                    }
                }
            }),
        },
    }
}

pub fn execute_command_tool_def(description: &str) -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "execute_command".into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "要执行的命令"
                    }
                },
                "required": ["command"]
            }),
        },
    }
}

/// Central tool dispatch: all agents call this instead of writing their own match block.
pub async fn execute_named_tool(
    name: &str,
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    tool_log::log_tool_call(dept, name, args, working_dir).await;
    match name {
        "read_file" => tool_read_file(working_dir, args).await,
        "create_file" => tool_create_file(working_dir, args).await,
        "list_dir" => tool_list_dir(working_dir, args).await,
        "append_file" => tool_append_file(working_dir, args).await,
        "delete_file" => tool_delete_file(working_dir, args).await,
        "rename_file" => tool_rename_file(working_dir, args).await,
        "modify_file" => tool_modify_file(working_dir, args).await,
        "apply_patch" => tool_apply_patch(working_dir, args).await,
        "create_document" => documents::tool_create_document(working_dir, args, dept).await,
        "modify_document" => documents::tool_modify_document(working_dir, args, dept).await,
        "set_document_status" => documents::tool_set_document_status(working_dir, args).await,
        "append_document" => {
            // Gate: check refs before appending to a document
            let id = args["id"].as_str().unwrap_or("");
            if !id.is_empty() {
                if let Err(msg) =
                    documents::check_doc_refs_approved_for_route(working_dir, id).await
                {
                    ToolOutput::error("append_document", id, "doc_not_approved", &msg)
                } else {
                    documents::tool_append_document(working_dir, args, dept).await
                }
            } else {
                documents::tool_append_document(working_dir, args, dept).await
            }
        }
        "find_document" => documents::tool_find_document(working_dir, args).await,
        "execute_command" => tool_execute_command(working_dir, args, dept).await,
        "summarize_logs" => tool_summarize_logs(working_dir, args).await,
        "route_to" => {
            // Gate: check refs before routing to execution departments
            let exec_depts = ["尚书令", "吏部", "兵部", "工部", "刑部", "礼部"];
            let to_name = args["to"].as_str().unwrap_or("");
            let subject = args["subject"].as_str().unwrap_or("");
            if exec_depts.contains(&to_name) && !subject.is_empty() {
                if let Err(msg) =
                    documents::check_doc_refs_approved_for_route(working_dir, subject).await
                {
                    ToolOutput::error("route_to", subject, "doc_not_approved", &msg)
                } else {
                    handle_route_to(args, dept)
                }
            } else {
                handle_route_to(args, dept)
            }
        }
        "route" => {
            ToolOutput::success_raw("route", "请调用 route_to 工具，不要输出文本 route 标签。")
        }
        _ => ToolOutput::error("unknown_tool", name, "unknown_tool", "未知工具"),
    }
}

/// Validate and execute route_to — returns a ToolOutput with `operation: "route_to"`
/// so the AgentController can detect it in the result and break the tool loop.
fn handle_route_to(args: &serde_json::Value, dept: &str) -> String {
    let to_name = args["to"].as_str().unwrap_or("");
    if to_name.is_empty() {
        return ToolOutput::error("route_to", "", "missing_target", "缺少目标部门（to 参数）");
    }
    let subject = args["subject"].as_str().unwrap_or("");
    if subject.is_empty() {
        return ToolOutput::error(
            "route_to",
            "",
            "missing_subject",
            "缺少文档 ID（subject 参数）",
        );
    }
    let _type = args["type"].as_str().unwrap_or("task");
    if !matches!(_type, "task" | "replace" | "interrupt") {
        return ToolOutput::error(
            "route_to",
            "",
            "invalid_type",
            &format!("无效的路由类型: {}，必须是 task/replace/interrupt", _type),
        );
    }
    let _ = dept;
    ToolOutput::success(
        "route_to",
        "",
        &format!("路由到 {}（{}）：{}", to_name, _type, subject),
    )
}

// ── Special tools (内阁 only) ──────────────────────────────────────

/// Dispatch handler for 内阁's special tools. Returns `Some(result)` if
/// the tool name matches, `None` to fall through to the normal tool dispatch.
pub async fn tool_handle_neige_special(
    name: &str,
    args: &serde_json::Value,
    ctx: &ToolContext,
) -> Option<String> {
    match name {
        "cancel_agent" => Some(tool_cancel_agent(args, ctx).await),
        "update_soul" => Some(tool_update_soul(args, ctx).await),
        "expand_requirements" => Some(tool_expand_requirements(args, ctx).await),
        "create_skill" => Some(tool_create_skill(args, ctx).await),
        _ => None,
    }
}

async fn tool_cancel_agent(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let target = args["to"].as_str().unwrap_or("");
    if let Some(role) = Role::from_name(target) {
        // Set cancel flag
        if let Some(ref map) = ctx.cancel_map {
            if let Ok(guard) = map.lock() {
                if let Some(flag) = guard.get(&role) {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        }
        // Send fast interrupt to immediately stop tool execution
        if let Some(ref fast_txs) = ctx.fast_txs {
            if let Some(tx) = fast_txs.get(&role) {
                let _ = tx.send(FastMessage::Interrupt);
            }
        }
        log_console!("[tool] cancel_agent → {} interrupted", target);
        return serde_json::json!({"ok": true, "message": format!("已中断 {} 的当前操作", target)})
            .to_string();
    }
    serde_json::json!({"ok": false, "message": format!("无法中断: {}", target)}).to_string()
}

async fn tool_update_soul(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let content = args["content"].as_str().unwrap_or("");
    if content.is_empty() {
        return r#"{"ok": false, "message": "content 不能为空"}"#.to_string();
    }
    if content.len() > 500 {
        return r#"{"ok": false, "message": "内容过长（最多500字符）"}"#.to_string();
    }
    let section = args["section"].as_str();
    let soul_dir = ctx.working_dir.join(".shuji").join("soul");
    let soul_path = soul_dir.join("neige.md");
    let _ = tokio::fs::create_dir_all(&soul_dir).await;

    let entry = format!("- {}\n", content);
    let result = if let Some(sec) = section {
        match tokio::fs::read_to_string(&soul_path).await {
            Ok(existing) => {
                let heading = format!("## {}", sec);
                if let Some(pos) = existing.find(&heading) {
                    let after_heading = &existing[pos + heading.len()..];
                    let next_heading = after_heading.find("\n## ");
                    let insert_pos =
                        pos + heading.len() + next_heading.unwrap_or(after_heading.len());
                    let mut new_content = existing[..insert_pos].to_string();
                    if !new_content.ends_with('\n') {
                        new_content.push('\n');
                    }
                    if !new_content.ends_with("\n\n") {
                        new_content.push('\n');
                    }
                    new_content.push_str(&entry);
                    new_content.push_str(&existing[insert_pos..]);
                    match tokio::fs::write(&soul_path, &new_content).await {
                        Ok(_) => Ok(format!("已记录到「{}」章节", sec)),
                        Err(e) => Err(e),
                    }
                } else {
                    use tokio::io::AsyncWriteExt;
                    match tokio::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .write(true)
                        .open(&soul_path)
                        .await
                    {
                        Ok(mut f) => {
                            let line = format!("\n## {}\n\n{}", sec, entry);
                            f.write_all(line.as_bytes()).await.ok();
                            Ok("已创建章节并记录".to_string())
                        }
                        Err(e) => Err(e),
                    }
                }
            }
            Err(_) => {
                let default = include_str!("../agent/neige/soul.md");
                let with_entry = format!("{}\n{}", default, entry);
                match tokio::fs::write(&soul_path, &with_entry).await {
                    Ok(_) => Ok("已记录".to_string()),
                    Err(e) => Err(e),
                }
            }
        }
    } else {
        use tokio::io::AsyncWriteExt;
        match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(&soul_path)
            .await
        {
            Ok(mut f) => {
                f.write_all(entry.as_bytes()).await.ok();
                Ok("已记录".to_string())
            }
            Err(e) => Err(e),
        }
    };

    match result {
        Ok(msg) => {
            log_console!(
                "[tool] update_soul → {} (section={})",
                content,
                section.unwrap_or("末尾")
            );

            // Check soul file size and auto-compact if > 8KB
            if let Ok(metadata) = tokio::fs::metadata(&soul_path).await {
                if metadata.len() > 8 * 1024 {
                    log_console!("[tool] soul 超过 8KB（{}），触发自动压缩", metadata.len());
                    match compact_soul_file(ctx).await {
                        Ok(compact_msg) => {
                            let full_msg = format!("{}. {}", msg, compact_msg);
                            return serde_json::json!({"ok": true, "message": full_msg})
                                .to_string();
                        }
                        Err(e) => {
                            log_console!("[tool] soul 压缩失败: {}", e);
                        }
                    }
                }
            }

            serde_json::json!({"ok": true, "message": msg}).to_string()
        }
        Err(e) => {
            serde_json::json!({"ok": false, "message": format!("写入失败: {}", e)}).to_string()
        }
    }
}

/// Compact soul file using LLM when it exceeds 8KB.
/// Summarizes into a concise version with core 10 items max.
async fn compact_soul_file(ctx: &ToolContext) -> Result<String, String> {
    let soul_path = ctx.working_dir.join(".shuji").join("soul").join("neige.md");
    let content = tokio::fs::read_to_string(&soul_path)
        .await
        .map_err(|e| format!("读取 soul 失败: {}", e))?;

    let client = ctx
        .client
        .clone()
        .ok_or("LLM 客户端未配置，无法压缩 soul")?;
    let model = ctx.model.clone().ok_or("LLM 模型未配置")?;

    let prompt = format!(
        r#"你是一个 soul 压缩工具。soul 是内阁首辅的经验/教训/偏好记录。

当前 soul {} 字节，已超过 8KB 上限。请提炼为核心版本，保留最有价值的条目。

要求：
- 保留 ## 经验 / ## 教训 / ## 偏好 三个章节
- 每章节不超过 5 条
- 保持原有格式（每条用 `- ` 开头）
- 去除重复或相似条目
- 总字符数不超过 4000

原始 soul：
{}"#,
        content.len(),
        content
    );

    let msg = crate::models::message::Message::user(&prompt);
    let compacted = client
        .send_message(
            "请压缩 soul 内容，输出精简版 Markdown（包含 ## 经验 / ## 教训 / ## 偏好）",
            &[msg],
            &model,
        )
        .await
        .map_err(|e| format!("LLM 压缩请求失败: {}", e))?
        .trim()
        .to_string();

    if compacted.is_empty() || compacted.len() >= content.len() {
        return Err("压缩结果无效或未减小".to_string());
    }

    tokio::fs::write(&soul_path, &compacted)
        .await
        .map_err(|e| format!("写入压缩后 soul 失败: {}", e))?;

    log_console!(
        "[tool] soul 压缩完成: {} → {} 字节",
        content.len(),
        compacted.len()
    );

    Ok(format!(
        "soul 已自动压缩（{} → {} 字节）",
        content.len(),
        compacted.len()
    ))
}

async fn tool_expand_requirements(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let task_id = args["task_id"].as_str().unwrap_or("");
    if task_id.is_empty() {
        return r#"{"ok": false, "message": "task_id 不能为空"}"#.to_string();
    }
    let client = match ctx.client.as_ref() {
        Some(c) => c,
        None => return serde_json::json!({"ok": false, "message": "API客户端不可用"}).to_string(),
    };
    let model = match ctx.model.as_ref() {
        Some(m) => m,
        None => return serde_json::json!({"ok": false, "message": "模型不可用"}).to_string(),
    };
    match crate::agent::expand_requirements::run(task_id, &ctx.working_dir, client, model).await {
        Ok(doc_id) => {
            log_console!("[tool] expand_requirements → {}", doc_id);
            serde_json::json!({"ok": true, "document_id": doc_id}).to_string()
        }
        Err(e) => {
            log_console!("[tool] expand_requirements 失败: {}", e);
            serde_json::json!({"ok": false, "message": e}).to_string()
        }
    }
}

async fn tool_create_skill(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let skill_name = args["name"].as_str().unwrap_or("");
    let description = args["description"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if skill_name.is_empty() || content.is_empty() {
        return r#"{"ok": false, "message": "name 和 content 不能为空"}"#.to_string();
    }
    if skill_name
        .chars()
        .any(|c| !c.is_alphanumeric() && c != '_' && c != '-')
    {
        return r#"{"ok": false, "message": "name 只能包含英文字母、数字、下划线和连字符"}"#
            .to_string();
    }
    let skills_dir = ctx.working_dir.join(".shuji").join("skills");
    let _ = tokio::fs::create_dir_all(&skills_dir).await;
    let skill_path = skills_dir.join(format!("{}.md", skill_name));
    let file_content = format!("# {}\n\n{}\n\n---\n\n{}", skill_name, description, content);
    match tokio::fs::write(&skill_path, &file_content).await {
        Ok(_) => {
            log_console!("[tool] create_skill → {} ({})", skill_name, description);
            serde_json::json!({
                "ok": true,
                "message": format!("技能 {} 已创建", skill_name),
                "skill_name": skill_name
            })
            .to_string()
        }
        Err(e) => {
            serde_json::json!({"ok": false, "message": format!("写入失败: {}", e)}).to_string()
        }
    }
}

const SYSTEM_BLOCKS: &[(&str, &str)] = &[
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

const PATH_ESCAPE: &[&str] = &[
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
