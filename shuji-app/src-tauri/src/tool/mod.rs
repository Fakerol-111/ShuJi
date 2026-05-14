use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use serde::Serialize;

pub mod documents;
mod tool_log;

/// Resolve a project-relative path against root with safety checks.
///
/// - Canonicalizes root for reliable comparison (handles Windows `\\?\` prefix)
/// - Rejects absolute paths and `..` traversal
/// - Canonicalizes existing paths to detect symlink escapes
/// - Returns error if resolved path is not within `root`
///
/// For files that don't exist yet (write operations), canonicalizes
/// the parent directory and then appends the filename.
pub fn resolve_scoped_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    // Canonicalize root once for reliable comparison across all code paths.
    // This handles Windows `\\?\` prefix, symlinks, and path normalization.
    let canon_root = std::fs::canonicalize(root)
        .map_err(|e| format!("项目根目录解析失败: {}", e))?;

    let rel_path = Path::new(rel);

    // Block absolute paths
    if rel_path.is_absolute() {
        return Err(format!("禁止使用绝对路径: {}", rel));
    }

    // Block .. traversal
    if rel.contains("..") {
        return Err(format!("禁止使用父目录跳转: {}", rel));
    }

    // Block drive letters (Windows)
    let lower = rel.to_lowercase();
    if lower.starts_with("c:") || lower.starts_with("d:") || lower.starts_with("e:") {
        return Err(format!("禁止使用盘符路径: {}", rel));
    }

    let candidate = root.join(rel_path);

    // For existing paths, canonicalize to detect escapes
    if candidate.exists() {
        let canon = std::fs::canonicalize(&candidate)
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
            let canon_parent = std::fs::canonicalize(parent)
                .map_err(|e| format!("父目录解析失败 {}: {}", rel, e))?;

            if !canon_parent.starts_with(&canon_root) {
                return Err(format!(
                    "路径越界: {} 的父目录不在项目目录内",
                    rel,
                ));
            }

            let filename = candidate
                .file_name()
                .ok_or_else(|| format!("无效文件名: {}", rel))?;

            return Ok(canon_parent.join(filename));
        }
    }

    // Parent doesn't exist yet — can't canonicalize. Compare against root
    // (not canon_root, which has Windows \\?\ prefix).
    let root_normalized = root
        .components()
        .collect::<PathBuf>();
    let normalized = candidate
        .components()
        .collect::<PathBuf>();

    if normalized.starts_with(&root_normalized) {
        Ok(normalized)
    } else {
        Err(format!("路径不在项目目录内: {}", rel))
    }
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
        serde_json::to_string(&o).unwrap_or_else(|_| format!("{{\"ok\":true,\"operation\":\"{}\",\"message\":\"{}\"}}", operation, message))
    }

    pub fn success_raw(operation: &str, message: &str) -> String {
        let mut o = Self::new(true, operation);
        o.message = Some(message.to_string());
        serde_json::to_string(&o).unwrap_or_else(|_| format!("{{\"ok\":true,\"operation\":\"{}\",\"message\":\"{}\"}}", operation, message))
    }

    pub fn read_file(operation: &str, path: &str, content: &str) -> String {
        let mut o = Self::new(true, operation);
        o.path = Some(path.to_string());
        o.message = Some(format!("共 {} 字节。内容如下：\n{}", content.len(), content));
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
pub fn tool_append_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("append_file", "", "empty_path", "文件路径为空");
    }
    if content.is_empty() {
        return ToolOutput::error("append_file", path, "empty_content", "追加内容为空");
    }
    let full = match resolve_scoped_path(working_dir, path) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("append_file", path, "path_error", &e),
    };
    // Create parent directories if needed
    if let Some(parent) = full.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::OpenOptions::new().create(true).append(true).open(&full) {
        Ok(mut file) => {
            use std::io::Write;
            if let Err(e) = writeln!(file, "{}", content) {
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
            description: "追加内容到已存在的文件末尾。CRITICAL: 每次调用的 content 参数必须在 500 字符以内，大文件必须分多次调用写入。先用 create_file 写第一部分，再用 append_file 逐块追加。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径，相对于项目根目录"
                    },
                    "content": {
                        "type": "string",
                        "description": "要追加的内容（每次最多 500 字符）",
                        "maxLength": 500
                    }
                },
                "required": ["path", "content"]
            }),
        },
    }
}

// ── delete_file helper ────────────────────────────────────────────

/// Delete a file. Returns an error if the path doesn't exist or is a directory.
pub fn tool_delete_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("delete_file", "", "empty_path", "文件路径为空");
    }
    let full = match resolve_scoped_path(working_dir, path) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("delete_file", path, "path_error", &e),
    };
    if !full.exists() {
        return ToolOutput::error("delete_file", path, "not_found", "文件不存在");
    }
    if full.is_dir() {
        return ToolOutput::error("delete_file", path, "is_directory", "不能删除目录，请使用文件路径");
    }
    match std::fs::remove_file(&full) {
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
            description: "删除项目目录下的文件，用于清理旧代码或旧设计文件".into(),
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
pub fn tool_rename_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let from = args["from"].as_str().unwrap_or("");
    let to = args["to"].as_str().unwrap_or("");
    if from.is_empty() || to.is_empty() {
        return ToolOutput::error("rename_file", "", "empty_path", "from 和 to 都不能为空");
    }
    let full_from = match resolve_scoped_path(working_dir, from) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("rename_file", from, "path_error", &e),
    };
    if !full_from.exists() {
        return ToolOutput::error("rename_file", from, "not_found", "源文件不存在");
    }
    let full_to = match resolve_scoped_path(working_dir, to) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("rename_file", to, "path_error", &e),
    };
    if let Some(parent) = full_to.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::rename(&full_from, &full_to) {
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

// ── modify_file helper ─────────────────────────────────────────

/// Modify an existing file by replacing matching text (find+replace).
/// Reads the file, finds `old_text`, replaces it with `new_text` (first
/// occurrence only).  The LLM should use read_file first to locate the
/// exact text to replace.
pub fn tool_modify_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("modify_file", "", "empty_path", "文件路径为空");
    }
    let full = match resolve_scoped_path(working_dir, path) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("modify_file", path, "path_error", &e),
    };
    if !full.exists() {
        return ToolOutput::error("modify_file", path, "not_found", "文件不存在");
    }
    let content = match std::fs::read_to_string(&full) {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("modify_file", path, "read_error", &e.to_string()),
    };

    let old_text = args["old_text"].as_str().unwrap_or("");
    let new_text = args["new_text"].as_str().unwrap_or("");
    if old_text.is_empty() {
        return ToolOutput::error("modify_file", path, "empty_old_text", "old_text 不能为空");
    }
    if !content.contains(old_text) {
        return ToolOutput::error("modify_file", path, "not_found",
            "未在文件中找到匹配的文本。请先用 read_file 确认文件内容，并确保 old_text 与原文件完全一致（包括空格和缩进）。");
    }

    let new_content = content.replacen(old_text, new_text, 1);
    match std::fs::write(&full, &new_content) {
        Ok(_) => ToolOutput::success("modify_file", path, &format!("替换成功（替换 {} 字节）", old_text.len())),
        Err(e) => ToolOutput::error("modify_file", path, "write_error", &e.to_string()),
    }
}

/// Generate a ToolDefinition for modify_file.
pub fn modify_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "modify_file".into(),
            description: "修改已存在的文件：找到 old_text 首次出现的位置，替换为 new_text。CRITICAL: old_text 和 new_text 参数必须在 300 字符以内。old_text 必须与文件中完全一致（含空格和缩进）。先用 read_file 确认文件内容，再调用此工具。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径，相对于项目根目录"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "文件中现有的文本，必须精确匹配（最多 300 字符）",
                        "maxLength": 300
                    },
                    "new_text": {
                        "type": "string",
                        "description": "替换后的新文本（最多 300 字符）",
                        "maxLength":300
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
pub fn tool_execute_command(working_dir: &Path, args: &serde_json::Value, dept: &str) -> String {
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
    match execute_with_timeout("bash", &["-l", "-c"], cmd, working_dir, timeout) {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code().unwrap_or(-1);
            if exit_code == 0 {
                format!("命令执行成功 (exit={}):\n{}", exit_code, stdout)
            } else {
                format!("命令执行失败 (exit={}):\nstdout:\n{}\nstderr:\n{}", exit_code, stdout, stderr)
            }
        }
        Err(timeout_msg) => timeout_msg,
    }
}

fn execute_with_timeout(
    shell: &str,
    args: &[&str],
    cmd: &str,
    working_dir: &Path,
    timeout: std::time::Duration,
) -> Result<std::process::Output, String> {
    let mut child = Command::new(shell);
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

    let start = std::time::Instant::now();
    let poll_interval = std::time::Duration::from_millis(500);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(ref mut out) = child.stdout {
                    let _ = out.read_to_end(&mut stdout);
                }
                if let Some(ref mut err) = child.stderr {
                    let _ = err.read_to_end(&mut stderr);
                }
                return Ok(std::process::Output { status, stdout, stderr });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "命令执行超时（超过 {} 秒），进程已终止。",
                        timeout.as_secs()
                    ));
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => return Err(format!("命令执行出错: {}", e)),
        }
    }
}

fn check_safe_command(cmd: &str) -> Result<(), &'static str> {
    let c = cmd.trim();
    for &(keyword, reason) in SYSTEM_BLOCKS {
        if c.to_lowercase().contains(keyword) {
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
pub fn tool_read_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let offset = args["offset"].as_u64().unwrap_or(0);
    let limit = args["limit"].as_u64().unwrap_or(u64::MAX);
    let full = match resolve_scoped_path(working_dir, path) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("read_file", path, "path_error", &e),
    };
    let content = match std::fs::read_to_string(&full) {
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
    let excerpt: Vec<String> = lines[start..end].iter().enumerate()
        .map(|(i, line)| format!("{:>4}| {}", start + i + 1, line))
        .collect();
    let meta = format!("{}（共 {} 行，显示 {}-{}）", path, total, start + 1, end);
    let result = format!("{}\n{}", meta, excerpt.join("\n"));

    ToolOutput::read_file("read_file", path, &result)
}

/// Create a new file with initial content. Rejects if the file already exists
/// (use modify_file or delete+create instead).
pub fn tool_create_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("create_file", "", "empty_path", "文件路径为空");
    }

    let full = match resolve_scoped_path(working_dir, path) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("create_file", path, "path_error", &e),
    };

    if full.exists() {
        return ToolOutput::error("create_file", path, "already_exists",
            "文件已存在，不允许覆盖。请使用 modify_file 修改内容，或先 delete_file 再 create_file。");
    }

    if let Some(parent) = full.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&full, content) {
        Ok(_) => ToolOutput::success("create_file", path, "写入成功"),
        Err(e) => ToolOutput::error("create_file", path, "write_error", &e.to_string()),
    }
}

/// List directory contents.
pub fn tool_list_dir(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let full = match resolve_scoped_path(working_dir, path) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("list_dir", path, "path_error", &e),
    };
    match std::fs::read_dir(&full) {
        Ok(entries) => {
            let items: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| {
                    let tag = e.file_type()
                        .map(|t| if t.is_dir() { "[DIR]" } else { "[FILE]" })
                        .unwrap_or("[?]");
                    format!("{} {}", tag, e.file_name().to_string_lossy())
                })
                .collect();
            let message = if items.is_empty() { "(空目录)".to_string() } else { items.join("\n") };
            ToolOutput::success_raw("list_dir", &message)
        }
        Err(e) => ToolOutput::error("list_dir", path, "list_error", &e.to_string()),
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
            description: format!("{}。CRITICAL: content 参数必须在 500 字符以内。大文件必须先用 create_file 写入最小内容，再用 append_file 分块追加。", description),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件路径，相对于项目根目录"
                    },
                    "content": {
                        "type": "string",
                        "description": "初始文件内容（最多 500 字符）",
                        "maxLength": 500
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
pub fn tool_summarize_logs(working_dir: &Path, args: &serde_json::Value) -> String {
    let log_path = working_dir.join(".shuji").join("logs").join("activity.log");
    if !log_path.exists() {
        return ToolOutput::success_raw("summarize_logs", "暂无日志记录");
    }

    let content = match std::fs::read_to_string(&log_path) {
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
    result.push(format!("共 {} 条日志记录（文件共 {} 行，自第 {} 行开始）：", entries.len(), total_lines, since));
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
            description: "读取 .shuji/logs/activity.log，按行号返回日志记录。不传 since 则读全部，传 since 则只读新行。用于生成项目进展总结报告".into(),
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
pub fn execute_named_tool(name: &str, working_dir: &Path, args: &serde_json::Value, dept: &str) -> String {
    tool_log::log_tool_call(dept, name, args, working_dir);
    match name {
        "read_file" => tool_read_file(working_dir, args),
        "create_file" => tool_create_file(working_dir, args),
        "list_dir" => tool_list_dir(working_dir, args),
        "append_file" => tool_append_file(working_dir, args),
        "delete_file" => tool_delete_file(working_dir, args),
        "rename_file" => tool_rename_file(working_dir, args),
        "modify_file" => tool_modify_file(working_dir, args),
        "create_document" => documents::tool_create_document(working_dir, args, dept),
        "modify_document" => documents::tool_modify_document(working_dir, args, dept),
        "append_document" => documents::tool_append_document(working_dir, args, dept),
        "find_document" => documents::tool_find_document(working_dir, args),
        "execute_command" => tool_execute_command(working_dir, args, dept),
        "summarize_logs" => tool_summarize_logs(working_dir, args),
        "route" => ToolOutput::success_raw("route",
            "请调用 route_to 工具，不要输出文本 route 标签。"),
        _ => ToolOutput::error("unknown_tool", name, "unknown_tool", "未知工具"),
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
    ("wget ", "禁止远程下载执行"),
    ("powershell -enc", "禁止编码执行PowerShell"),
    ("certutil -urlcache", "禁止远程下载"),
    ("bitsadmin /transfer", "禁止远程下载"),
    ("mshta", "禁止执行MSHTA脚本"),
    ("npm install -g", "禁止全局安装"),
];

const PATH_ESCAPE: &[&str] = &[
    "..\\", "../",
    "/windows", "/windows/system32", "/program files", "/programdata", "/users",
    "%systemroot%", "%windir%", "%appdata%", "%programfiles%",
];

