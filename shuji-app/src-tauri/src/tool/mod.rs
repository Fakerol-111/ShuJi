use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::actor::FastMessage;
use crate::api::client::AnthropicClient;
use crate::models::role::Role;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

// ── P2-2: In-memory read cache ─────────────────────────────
/// Session-level read cache: maps resolved path → (mtime, cached result string).
/// After any write/delete/rename/patch, the affected path(s) are invalidated.
static READ_CACHE: LazyLock<Mutex<HashMap<PathBuf, (SystemTime, String)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Look up a cached read result. Returns `Some(result)` if the file's mtime
/// hasn't changed since the cache entry was created.
pub fn cache_lookup(path: &Path) -> Option<String> {
    let cache = READ_CACHE.lock().ok()?;
    if let Some((cached_mtime, cached_result)) = cache.get(path) {
        if let Ok(current_mtime) = std::fs::metadata(path).and_then(|m| m.modified()) {
            if current_mtime == *cached_mtime {
                return Some(format!(
                    "{}[缓存命中: 内容未变 (cached: true)]",
                    cached_result
                ));
            }
        }
    }
    None
}

/// Insert a read result into the cache.
pub fn cache_insert(path: PathBuf, mtime: SystemTime, result: String) {
    if let Ok(mut cache) = READ_CACHE.lock() {
        cache.insert(path, (mtime, result));
    }
}

/// Invalidate cache entries whose key starts with or equals `path`.
/// Call after any write/delete/rename/patch operation.
fn cache_invalidate(path: &Path) {
    if let Ok(mut cache) = READ_CACHE.lock() {
        cache.retain(|k, _| !k.starts_with(path));
    }
}

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
    pub cancel_map: Option<crate::CancelMap>,
    pub client: Option<Arc<AnthropicClient>>,
    pub model: Option<String>,
    /// Fast mailbox senders for interrupting departments immediately.
    pub fast_txs: Option<crate::FastTxMap>,
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
            &format!("old_text 长度 {} 超过上限 800 字符。对于大块修改，请使用 apply_patch（支持 50000 字符）。", old_text.len()));
    }
    if new_text.len() > 800 {
        return ToolOutput::error("modify_file", path, "new_text_too_long",
            &format!("new_text 长度 {} 超过上限 800 字符。对于大块修改，请使用 apply_patch（支持 50000 字符）。", new_text.len()));
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
                "替换文件中的文本 (find+replace)。≤800字符。大块修改请用 apply_patch（支持 50000 字符）。工部/刑部已禁用此工具——使用 apply_patch。".into(),
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

    // P2-2: Read cache — return cached result for full-file reads
    let is_full_read = offset == 0 && limit == u64::MAX;
    if is_full_read {
        if let Some(cached) = cache_lookup(&full) {
            return cached;
        }
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => {
            // Cache raw content with mtime for future reads
            if let Ok(meta) = tokio::fs::metadata(&full).await {
                if let Ok(mtime) = meta.modified() {
                    cache_insert(full.clone(), mtime, c.clone());
                }
            }
            c
        }
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

/// Recursive directory tree listing (P0-5). Returns indented tree with depth limit.
/// Parameters: path, depth (default 2, max 5), glob filter.
pub async fn tool_list_dir_tree(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let max_depth = args["depth"].as_u64().unwrap_or(2).min(5) as usize;
    let glob = args["glob"].as_str().filter(|s| !s.is_empty());

    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("list_dir_tree", path, "path_error", &e),
    };

    let skip_dirs: &[&str] = &[
        ".git",
        ".shuji",
        "node_modules",
        "target",
        ".venv",
        "__pycache__",
        "dist",
        "build",
    ];

    let mut lines = Vec::new();
    let mut total = 0usize;
    let max_items = 200usize;

    // Use BFS-like recursion with depth tracking
    fn collect(
        dir: &Path,
        working_dir: &Path,
        prefix: &str,
        depth: usize,
        max_depth: usize,
        glob: Option<&str>,
        skip_dirs: &[&str],
        lines: &mut Vec<String>,
        total: &mut usize,
        max_items: usize,
    ) {
        if *total >= max_items {
            return;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        let mut items: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some(g) = glob {
                    simple_glob_match(&name, g)
                } else {
                    true
                }
            })
            .collect();
        // Sort: dirs first, then alpha
        items.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if a_is_dir != b_is_dir {
                b_is_dir.cmp(&a_is_dir)
            } else {
                a.file_name().cmp(&b.file_name())
            }
        });

        for (i, entry) in items.iter().enumerate() {
            if *total >= max_items {
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let is_last = i == items.len() - 1;
            let connector = if is_last { "└── " } else { "├── " };
            let ep = entry.path();
            let rel = ep.strip_prefix(working_dir).unwrap_or(&ep);
            let line = if is_dir {
                format!("{}{}{}/", prefix, connector, rel.display())
            } else {
                format!("{}{}{}", prefix, connector, rel.display())
            };
            // Only add connector for direct children; deeper levels just show path
            lines.push(if depth == 0 { line } else { line });
            *total += 1;

            if is_dir && depth < max_depth {
                let child_prefix = if is_last {
                    format!("{}    ", prefix)
                } else {
                    format!("{}│   ", prefix)
                };
                if !skip_dirs.contains(&name.as_str()) {
                    collect(
                        &entry.path(),
                        working_dir,
                        &child_prefix,
                        depth + 1,
                        max_depth,
                        glob,
                        skip_dirs,
                        lines,
                        total,
                        max_items,
                    );
                }
            }
        }
    }

    collect(
        &full,
        working_dir,
        "",
        0,
        max_depth,
        glob,
        skip_dirs,
        &mut lines,
        &mut total,
        max_items,
    );

    if lines.is_empty() {
        return ToolOutput::success_raw("list_dir_tree", "(空)");
    }

    let mut output = lines.join("\n");
    if total >= max_items {
        output.push_str(&format!(
            "\n\n… 显示前 {} 项，如需更多请缩小范围",
            max_items
        ));
    }
    output.push_str(&format!("\n\n共 {} 项", total));
    ToolOutput::success_raw("list_dir_tree", &output)
}

/// Tool definition for list_dir_tree.
pub fn list_dir_tree_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "list_dir_tree".into(),
            description: "递归目录树浏览。列出目录下所有子目录和文件（带缩进）。替代多次 list_dir。depth 默认 2，最大 5。支持 glob 过滤。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "目录路径，相对于项目根目录"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "递归深度（默认 2，最大 5）"
                    },
                    "glob": {
                        "type": "string",
                        "description": "可选：文件名过滤，如 *.rs, *.md"
                    }
                },
                "required": ["path"]
            }),
        },
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

// ── search_text helper ──────────────────────────────────────────────

/// Try ripgrep (`rg --json`) for fast text search. Falls back to the
/// Rust implementation if `rg` is not installed or returns an error.
async fn try_rg_search(
    working_dir: &Path,
    pattern: &str,
    max_results: usize,
    glob: Option<&str>,
    case_sensitive: bool,
) -> Result<String, ()> {
    use std::process::Stdio;

    let mut cmd = tokio::process::Command::new("rg");
    cmd.arg("--json")
        .arg("-e")
        .arg(pattern)
        .arg("--max-count")
        .arg(max_results.to_string())
        .current_dir(working_dir);

    if !case_sensitive {
        cmd.arg("-i");
    }
    if let Some(g) = glob {
        cmd.arg("-g").arg(g);
    }

    // Skip noise directories automatically (rg respects .gitignore by default)
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());

    let output = cmd.output().await.map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results: Vec<String> = Vec::new();
    let mut file_count = 0usize;

    for line in stdout.lines() {
        if results.len() >= max_results {
            break;
        }
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            if entry["type"].as_str() == Some("match") {
                let path = entry["data"]["path"]["text"].as_str().unwrap_or("?");
                let line_no = entry["data"]["line_number"].as_u64().unwrap_or(0);
                let content = entry["data"]["lines"]["text"].as_str().unwrap_or("");
                results.push(format!("{}:{}:{}", path, line_no, content.trim()));
                file_count += 1;
            }
        }
    }

    if results.is_empty() {
        return Err(());
    }

    let summary = format!(
        "[ripgrep] 在 {} 文件中搜索「{}」，匹配 {} 处：\n{}",
        file_count,
        pattern,
        results.len(),
        results.join("\n")
    );
    Ok(ToolOutput::success_raw("search_text", &summary))
}

/// Search for a text pattern in project files. (P2-3: tries `rg` first, then fallback.)
/// Returns path:line_number: content_line for each match.
pub async fn tool_search_text(working_dir: &Path, args: &serde_json::Value) -> String {
    let pattern = args["pattern"].as_str().unwrap_or("");
    if pattern.is_empty() {
        return ToolOutput::error("search_text", "", "empty_pattern", "搜索模式不能为空");
    }
    let max_results = args["max_results"].as_u64().unwrap_or(50) as usize;
    let glob = args["glob"].as_str().filter(|s| !s.is_empty());
    let case_sensitive = args["case_sensitive"].as_bool().unwrap_or(true);

    // P2-3: Try ripgrep first for speed, fall back to Rust implementation.
    if let Ok(result) = try_rg_search(working_dir, pattern, max_results, glob, case_sensitive).await
    {
        return result;
    }

    let mut results: Vec<String> = Vec::new();
    let mut file_count: usize = 0;
    let mut error_count: usize = 0;

    // Directories to skip (common noise directories)
    let skip_dirs: &[&str] = &[
        ".git",
        ".shuji",
        "node_modules",
        "target",
        ".venv",
        "__pycache__",
    ];

    let mut stack = vec![working_dir.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
            let ft = match entry.file_type().await {
                Ok(t) => t,
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if ft.is_dir() {
                if !skip_dirs.contains(&name.as_str()) {
                    stack.push(entry.path());
                }
            } else if ft.is_file() {
                // Apply glob filter on filename
                if let Some(g) = glob {
                    if !simple_glob_match(&name, g) {
                        continue;
                    }
                }

                // Read file and search line by line
                let content = match tokio::fs::read_to_string(entry.path()).await {
                    Ok(c) => c,
                    Err(_) => {
                        error_count += 1;
                        continue;
                    }
                };
                file_count += 1;

                let rel_path = entry
                    .path()
                    .strip_prefix(working_dir)
                    .unwrap_or(&entry.path())
                    .to_string_lossy()
                    .to_string();

                for (line_no, line) in content.lines().enumerate() {
                    let matched = if case_sensitive {
                        line.contains(pattern)
                    } else {
                        line.to_lowercase().contains(&pattern.to_lowercase())
                    };
                    if matched {
                        results.push(format!("{}:{}:{}", rel_path, line_no + 1, line));
                        if results.len() >= max_results {
                            break;
                        }
                    }
                }
                if results.len() >= max_results {
                    break;
                }
            }
        }
        if results.len() >= max_results {
            break;
        }
    }

    if results.is_empty() {
        let searched = if let Some(g) = glob {
            format!("在 {} 文件中搜索「{}」（{}）", file_count, pattern, g)
        } else {
            format!("在 {} 文件中搜索「{}」", file_count, pattern)
        };
        let msg = if error_count > 0 {
            format!(
                "{}，未找到匹配（{} 个文件因编码/权限跳过）",
                searched, error_count
            )
        } else {
            format!("{}，未找到匹配", searched)
        };
        return ToolOutput::success_raw("search_text", &msg);
    }

    let summary = format!(
        "找到 {} 个匹配（共扫描 {} 文件{}），显示前 {} 行：\n{}",
        results.len(),
        file_count,
        if error_count > 0 {
            format!("，{} 跳过", error_count)
        } else {
            String::new()
        },
        results.len().min(max_results),
        results.join("\n")
    );

    ToolOutput::success_raw("search_text", &summary)
}

/// Simple glob matching: supports `*` (any sequence) and `?` (single char).
/// No path separators — matches only against the filename.
fn simple_glob_match(name: &str, glob: &str) -> bool {
    let name_chars: Vec<char> = name.chars().collect();
    let glob_chars: Vec<char> = glob.chars().collect();
    glob_match_inner(&name_chars, &glob_chars, 0, 0)
}

fn glob_match_inner(name: &[char], glob: &[char], ni: usize, gi: usize) -> bool {
    if gi == glob.len() {
        return ni == name.len();
    }
    if glob[gi] == '*' {
        // Try matching 0, 1, 2, ... characters
        for i in ni..=name.len() {
            if glob_match_inner(name, glob, i, gi + 1) {
                return true;
            }
        }
        false
    } else if glob[gi] == '?' {
        if ni < name.len() {
            glob_match_inner(name, glob, ni + 1, gi + 1)
        } else {
            false
        }
    } else {
        if ni < name.len() && name[ni] == glob[gi] {
            glob_match_inner(name, glob, ni + 1, gi + 1)
        } else {
            false
        }
    }
}

pub fn search_text_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "search_text".into(),
            description: "在项目文件中递归搜索文本模式。返回 文件:行号:内容 格式。替代 list_dir + 多次 read_file。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "要搜索的文本（区分大小写，不支持正则）"
                    },
                    "glob": {
                        "type": "string",
                        "description": "可选：文件名过滤模式，如 *.rs, *.md, *test*。支持 * 和 ? 通配符"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "可选：最大返回匹配数（默认 50）"
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "description": "可选：是否区分大小写（默认 true）"
                    }
                },
                "required": ["pattern"]
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

/// ── run_tests (工部专用) ────────────────────────────────────────────
/// Runs tests with auto-detected project type and structured output.
/// Reduces LLM command-typing errors that trigger watchdog.
pub async fn tool_run_tests(working_dir: &Path, args: &serde_json::Value) -> String {
    let scope = args["scope"].as_str().unwrap_or("all");
    let path = args["path"].as_str().filter(|s| !s.is_empty());

    // Detect project type
    let project_type = detect_project_type(working_dir);
    let mut cmd = match project_type.as_str() {
        "rust" => match scope {
            "unit" => "cargo test --lib".to_string(),
            "integration" => "cargo test --tests".to_string(),
            _ => "cargo test".to_string(),
        },
        "node" => {
            // Use project's test script or common runners
            let has_vitest = working_dir.join("node_modules/.bin/vitest").exists();
            let has_jest = working_dir.join("node_modules/.bin/jest").exists();
            if has_vitest {
                format!("npx vitest run{}", scope_suffix(scope))
            } else if has_jest {
                format!("npx jest --verbose{}", scope_suffix(scope))
            } else {
                "npm test".to_string()
            }
        }
        "python" => match scope {
            "unit" => "python -m pytest tests/ -v".to_string(),
            "integration" => "python -m pytest tests/integration/ -v".to_string(),
            _ => "python -m pytest -v".to_string(),
        },
        _ => {
            return ToolOutput::success_raw(
                "run_tests",
                "未能检测到已知项目类型（Cargo.toml / package.json / pyproject.toml），无法确定测试命令。请使用 execute_command 自定义。",
            );
        }
    };

    // Append specific test file path if provided
    if let Some(p) = path {
        // Scope check: reject unit path with integration scope and vice versa
        if scope == "integration" && !p.contains("integration") {
            return ToolOutput::error(
                "run_tests",
                "",
                "scope_mismatch",
                &format!(
                    "scope=integration 但路径 {} 不匹配集成测试目录（tests/integration/）",
                    p
                ),
            );
        }
        cmd.push_str(&format!(" -- {}", p));
    }

    log_console!("[run_tests] executing: {}", cmd);
    let timeout = std::time::Duration::from_secs(300);

    let (shell, shell_args) = if cfg!(windows) {
        ("powershell", vec!["-Command"])
    } else {
        ("bash", vec!["-l", "-c"])
    };

    let output = match execute_with_timeout(shell, &shell_args, &cmd, working_dir, timeout).await {
        Ok(o) => o,
        Err(e) => return ToolOutput::error("run_tests", "", "exec_error", &e),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    // Parse test results from output
    let (parsed_pass, parsed_fail, parsed_total) = parse_test_output(&stdout, &stderr);

    let pass_count = parsed_pass.unwrap_or(0);
    let fail_count = parsed_fail.unwrap_or(0);
    let total_count = parsed_total.unwrap_or(0);

    // Extract failed test names from output
    let failed_tests: Vec<&str> = stdout
        .lines()
        .filter(|l| l.contains("FAILED") || l.contains("... FAILED"))
        .map(|l| {
            l.trim()
                .trim_start_matches("test ")
                .trim_end_matches(" ... FAILED")
                .trim_end_matches(" FAILED")
        })
        .collect();

    // Build structured result
    let mut report = String::new();
    report.push_str(&format!(
        "## 测试执行报告\n\n项目类型: {} | 范围: {} | 命令: `{}`\n",
        project_type, scope, cmd
    ));
    report.push_str(&format!(
        "退出码: {} | 通过: {} | 失败: {}",
        exit_code, pass_count, fail_count,
    ));
    if total_count > 0 {
        report.push_str(&format!(" | 总计: {}", total_count));
    }
    report.push('\n');

    if !failed_tests.is_empty() {
        report.push_str("\n### 失败用例\n");
        for t in &failed_tests {
            report.push_str(&format!("- {}\n", t));
        }
    }

    if exit_code != 0 {
        // Truncate stderr to avoid context overflow
        let stderr_trimmed = if stderr.len() > 2000 {
            format!(
                "{}...\n[截断：显示前 2000 字符，共 {} 字符]",
                &stderr[..2000],
                stderr.len()
            )
        } else {
            stderr.to_string()
        };
        if !stderr_trimmed.is_empty() {
            report.push_str(&format!("\n### stderr 摘要\n{}", stderr_trimmed));
        }
    }

    if exit_code == 0 && failed_tests.is_empty() {
        report.push_str("\n✅ 全部通过");
    }

    ToolOutput::success_raw("run_tests", &report)
}

/// Detect project type by checking for key files.
fn detect_project_type(working_dir: &Path) -> String {
    if working_dir.join("Cargo.toml").exists() {
        "rust".to_string()
    } else if working_dir.join("package.json").exists() {
        "node".to_string()
    } else if working_dir.join("pyproject.toml").exists()
        || working_dir.join("setup.py").exists()
        || working_dir.join("requirements.txt").exists()
    {
        "python".to_string()
    } else {
        "unknown".to_string()
    }
}

fn scope_suffix(scope: &str) -> &str {
    match scope {
        "unit" => " tests/",
        "integration" => " tests/integration/",
        _ => "",
    }
}

/// Parse test output to extract pass/fail/total counts.
/// Handles both Rust (cargo test) and Python (pytest) output formats.
fn parse_test_output(stdout: &str, stderr: &str) -> (Option<usize>, Option<usize>, Option<usize>) {
    let combined = format!("{}\n{}", stdout, stderr);

    // Rust: "test result: FAILED. 7 passed; 1 failed; 0 ignored; ..."
    if let Some(line) = combined.lines().find(|l| l.contains("test result:")) {
        let passed = line
            .split(';')
            .find_map(|s| {
                let s = s.trim();
                s.strip_suffix(" passed")
                    .or_else(|| s.strip_suffix(" passed"))
            })
            .and_then(|s| s.trim().parse().ok());
        let failed = line
            .split(';')
            .find_map(|s| {
                let s = s.trim();
                s.strip_suffix(" failed")
                    .or_else(|| s.strip_suffix(" failed"))
            })
            .and_then(|s| s.trim().parse().ok());
        let total = combined
            .lines()
            .filter(|l| l.starts_with("test ") && l.contains("..."))
            .count();
        return (passed, failed, Some(total));
    }

    // Python pytest: "= X passed, Y failed in Z.ZZs ="
    if let Some(line) = combined
        .lines()
        .find(|l| l.contains("passed") || l.contains("failed"))
    {
        let passed = line
            .split([' ', ','])
            .filter_map(|s| s.trim().strip_suffix("passed"))
            .filter_map(|s| s.trim().parse().ok())
            .next();
        let failed = line
            .split([' ', ','])
            .filter_map(|s| s.trim().strip_suffix("failed"))
            .filter_map(|s| s.trim().parse().ok())
            .next();
        return (passed, failed, None);
    }

    (None, None, None)
}

pub fn run_tests_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "run_tests".into(),
            description: "首选跑测试工具。自动检测项目类型（Rust/Node/Python），根据 scope 选择子命令。返回结构化报告：通过数、失败用例、stderr 摘要。禁止手写 cargo test/pytest——请使用本工具。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["unit", "integration", "all"],
                        "description": "unit=单元测试, integration=集成测试, all=全部（默认 all）"
                    },
                    "path": {
                        "type": "string",
                        "description": "可选：指定单个测试文件路径，如 tests/test_user.rs。scope 需匹配"
                    }
                },
                "required": ["scope"]
            }),
        },
    }
}

pub fn execute_command_tool_def(description: &str) -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "execute_command".into(),
            description: format!("{}。注意：运行测试请用 run_tests 工具（自动检测项目类型）。execute_command 仅用于 lint/format/构建等非测试命令。", description),
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

/// Truncate verbose tool results to avoid blowing up context.
/// Uses per-tool limits. Truncated results include a hint for continuation.
fn truncate_tool_result_by_name(name: &str, content: &str) -> String {
    let limit = match name {
        "read_file" | "read_document" => 8000,
        "search_text" => 8000,
        "list_dir" => 8000,
        "execute_command" => 4000,
        "run_tests" => 4000,
        "summarize_logs" => 4000,
        _ => 16000,
    };
    if content.len() > limit {
        let head: String = content.chars().take(limit).collect();
        format!(
            "{}...\n[截断：显示前 {} 字符，共 {} 字符。如需继续，请缩小范围后重试 (truncated: true)]",
            head,
            limit,
            content.len()
        )
    } else {
        content.to_string()
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
    let raw_result = match name {
        "read_file" => tool_read_file(working_dir, args).await,
        "create_file" => tool_create_file(working_dir, args).await,
        "list_dir" => tool_list_dir(working_dir, args).await,
        "list_dir_tree" => tool_list_dir_tree(working_dir, args).await,
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
        "find_document" => {
            // P0-2: find_document is deprecated — redirect to read_document
            let id = args["id"].as_str().unwrap_or("");
            ToolOutput::success_raw("find_document",
                &format!("find_document 已弃用。请改用 read_document(id=\"{}\")——一次调用即可查找+读取。", id))
        }
        "read_document" => documents::tool_read_document(working_dir, args).await,
        "search_text" => tool_search_text(working_dir, args).await,
        "run_tests" => tool_run_tests(working_dir, args).await,
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
        "init_checklist" => tool_init_checklist(args, working_dir).await,
        "update_checklist_item" => tool_update_checklist_item(args, working_dir).await,
        "add_violation" => tool_add_violation(args, working_dir).await,
        "request_reauth" => tool_request_reauth(args, working_dir).await,
        _ => ToolOutput::error("unknown_tool", name, "unknown_tool", "未知工具"),
    };
    // P2-2: Invalidate read cache after write operations
    match name {
        "create_file" | "modify_file" | "append_file" | "delete_file" | "rename_file"
        | "apply_patch" => {
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                if let Ok(full) = resolve_scoped_path(working_dir, path).await {
                    cache_invalidate(&full);
                    // Also invalidate parent directory (list_dir results change)
                    if let Some(parent) = full.parent() {
                        cache_invalidate(parent);
                    }
                }
            }
        }
        "create_document" | "modify_document" | "append_document" | "set_document_status" => {
            // Invalidate .shuji/ dir since document listings and reads may change
            let shuji_dir = working_dir.join(".shuji");
            cache_invalidate(&shuji_dir);
        }
        _ => {}
    }
    truncate_tool_result_by_name(name, &raw_result)
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
        "survey_codebase" => Some(tool_survey_codebase(args, ctx).await),
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
        crate::audit::append(
            &ctx.working_dir,
            "cancel_agent",
            "内阁",
            target,
            "cancel_agent 操作",
        )
        .await;
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

async fn tool_survey_codebase(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let task_description = args["task_description"].as_str().unwrap_or("");
    if task_description.is_empty() {
        return r#"{"ok": false, "message": "task_description 不能为空"}"#.to_string();
    }
    let client = match ctx.client.as_ref() {
        Some(c) => c,
        None => return serde_json::json!({"ok": false, "message": "API客户端不可用"}).to_string(),
    };
    let model = match ctx.model.as_ref() {
        Some(m) => m,
        None => return serde_json::json!({"ok": false, "message": "模型不可用"}).to_string(),
    };
    match crate::agent::survey_codebase::run(task_description, &ctx.working_dir, client, model)
        .await
    {
        Ok(doc_id) => {
            log_console!("[tool] survey_codebase → {}", doc_id);
            serde_json::json!({"ok": true, "document_id": doc_id}).to_string()
        }
        Err(e) => {
            log_console!("[tool] survey_codebase 失败: {}", e);
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

// ── Audit tools (礼部 + 尚书令) ─────────────────────────────

pub async fn tool_init_checklist(args: &serde_json::Value, working_dir: &Path) -> String {
    let category = args["category"].as_str().unwrap_or("general");
    let msg = crate::audit::init_checklist(working_dir, category).await;
    serde_json::json!({"ok": true, "message": msg}).to_string()
}

pub async fn tool_update_checklist_item(args: &serde_json::Value, working_dir: &Path) -> String {
    let id = args["id"].as_str().unwrap_or("");
    let status = args["status"].as_str().unwrap_or("");
    let note = args["note"].as_str().unwrap_or("");
    if id.is_empty() || status.is_empty() {
        return serde_json::json!({"ok": false, "message": "id 和 status 不能为空"}).to_string();
    }
    match crate::audit::update_checklist_item(working_dir, id, status, note).await {
        Ok(msg) => serde_json::json!({"ok": true, "message": msg}).to_string(),
        Err(e) => serde_json::json!({"ok": false, "message": e}).to_string(),
    }
}

pub async fn tool_add_violation(args: &serde_json::Value, working_dir: &Path) -> String {
    let severity = args["severity"].as_str().unwrap_or("warning");
    let rule_id = args["rule_id"].as_str().unwrap_or("");
    let location = args["location"].as_str().unwrap_or("");
    let description = args["description"].as_str().unwrap_or("");
    if rule_id.is_empty() || description.is_empty() {
        return serde_json::json!({"ok": false, "message": "rule_id 和 description 不能为空"})
            .to_string();
    }
    crate::audit::add_violation(working_dir, severity, rule_id, location, description).await;
    serde_json::json!({"ok": true, "message": format!("违规记录已添加: {} — {}", rule_id, description)}).to_string()
}

pub async fn tool_request_reauth(args: &serde_json::Value, working_dir: &Path) -> String {
    let subject = args["subject"].as_str().unwrap_or("");
    let reason = args["reason"].as_str().unwrap_or("");
    if subject.is_empty() || reason.is_empty() {
        return serde_json::json!({"ok": false, "message": "subject 和 reason 不能为空"})
            .to_string();
    }
    let _ = crate::audit::request_reauth(working_dir, subject, reason).await;
    // Return route_to operation so the AgentController automatically routes to the target
    let msg = format!(
        "已提交复验请求，自动路由到 {} 进行重新审计。{}",
        "礼部", reason
    );
    ToolOutput::success("route_to", subject, &msg)
}
