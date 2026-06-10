use std::path::Path;

use crate::tool::cache::{cache_insert, cache_lookup};
use crate::tool::path::resolve_scoped_path;
use crate::tool::ToolOutput;

// ── append_file ──────────────────────────────────────────────

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
    if content.len() > 2000 {
        return ToolOutput::error("append_file", path, "content_too_long",
            &format!("content 长度 {} 超过上限 2000 字符。请拆分成多次 append_file 调用，每次 ≤2000 字符。", content.len()));
    }
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("append_file", path, "path_error", &e),
    };
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

pub fn append_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "append_file".into(),
            description: "追加内容到文件末尾。content ≤2000 字符，大文件分批写入。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "文件路径，相对于项目根目录" },
                    "content": { "type": "string", "description": "要追加的内容（每次最多 2000 字符）", "maxLength": 2000 }
                },
                "required": ["path", "content"]
            }),
        },
    }
}

// ── delete_file ──────────────────────────────────────────────

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

pub fn delete_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "delete_file".into(),
            description: "删除项目目录下的文件".into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string", "description": "文件路径" } }, "required": ["path"] }),
        },
    }
}

// ── rename_file ──────────────────────────────────────────────

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

pub fn rename_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "rename_file".into(),
            description: "重命名或移动文件。提供 from（源路径）和 to（目标路径）".into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "from": { "type": "string", "description": "原路径" }, "to": { "type": "string", "description": "新路径" } }, "required": ["from", "to"] }),
        },
    }
}

// ── apply_patch ──────────────────────────────────────────────

pub async fn tool_apply_patch(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let patch_str = args["patch"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("apply_patch", "", "empty_path", "文件路径为空");
    }
    if patch_str.is_empty() {
        return ToolOutput::error("apply_patch", path, "empty_patch", "patch 内容为空");
    }
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
    if let Some(parent) = full.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let mut content = tokio::fs::read_to_string(&full).await.unwrap_or_default();
    let original_len = content.len();
    let blocks = match parse_search_replace_blocks(patch_str) {
        Ok(b) => b,
        Err(e) => return ToolOutput::error("apply_patch", path, "parse_error", &e),
    };
    for (i, (search_text, replace_text)) in blocks.iter().enumerate() {
        let block_num = i + 1;
        if search_text.is_empty() {
            content = replace_text.clone();
            continue;
        }
        let count = content.matches(search_text.as_str()).count();
        if count == 0 {
            let preview = if search_text.len() > 120 {
                let cutoff = search_text.floor_char_boundary(120);
                format!("{}...", &search_text[..cutoff])
            } else {
                search_text.clone()
            };
            return ToolOutput::error(
                "apply_patch",
                path,
                "search_not_found",
                &format!(
                    "第{}个 SEARCH 块未在文件中找到。\n查找的文本：\n```\n{}\n```",
                    block_num, preview
                ),
            );
        }
        if count > 1 {
            return ToolOutput::error(
                "apply_patch",
                path,
                "search_ambiguous",
                &format!(
                    "第{}个 SEARCH 块在文件中出现 {} 次，不唯一。",
                    block_num, count
                ),
            );
        }
        content = content.replacen(search_text.as_str(), replace_text.as_str(), 1);
    }
    match tokio::fs::write(&full, &content).await {
        Ok(_) => ToolOutput::success(
            "apply_patch",
            path,
            &format!(
                "成功应用 {} 个 SEARCH/REPLACE 块（{} 字节 → {} 字节）",
                blocks.len(),
                original_len,
                content.len()
            ),
        ),
        Err(e) => ToolOutput::error("apply_patch", path, "write_error", &e.to_string()),
    }
}

fn parse_search_replace_blocks(input: &str) -> Result<Vec<(String, String)>, String> {
    let mut blocks = Vec::new();
    let mut remaining = input;
    loop {
        let search_start = match remaining.find("<<<<<<< SEARCH") {
            Some(idx) => idx,
            None => break,
        };
        let after_marker = &remaining[search_start + 14..];
        let body = after_marker.strip_prefix('\n').unwrap_or(after_marker);
        let (search_text, rest) = if let Some(idx) = body.find("\n=======\n") {
            (&body[..idx], &body[idx + 10..])
        } else if body.starts_with("=======\n") {
            ("", &body[9..])
        } else {
            return Err("SEARCH/REPLACE 块缺少 '=======' 分隔符。".to_string());
        };
        let end_idx = match rest.find("\n>>>>>>> REPLACE") {
            Some(idx) => idx,
            None => return Err("SEARCH/REPLACE 块缺少 '>>>>>>> REPLACE' 结束标记。".to_string()),
        };
        let replace_text = &rest[..end_idx];
        blocks.push((search_text.to_string(), replace_text.to_string()));
        remaining = &rest[end_idx + 15..];
    }
    if blocks.is_empty() {
        return Err("未找到 SEARCH/REPLACE 块。格式：\n<<<<<<< SEARCH\n旧文本\n=======\n新文本\n>>>>>>> REPLACE".to_string());
    }
    Ok(blocks)
}

pub fn apply_patch_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "apply_patch".into(),
            description: "对文件应用 SEARCH/REPLACE 修改。支持多块。".into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "patch": { "type": "string" } }, "required": ["path", "patch"] }),
        },
    }
}

// ── modify_file ──────────────────────────────────────────────

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
    if old_text.len() > 800 || new_text.len() > 800 {
        return ToolOutput::error(
            "modify_file",
            path,
            "text_too_long",
            "文本超过 800 字符上限。大块修改请用 apply_patch（支持 50000 字符）。",
        );
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

pub fn modify_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "modify_file".into(),
            description: "替换文件中的文本 (find+replace)。≤800字符。大块修改请用 apply_patch。"
                .into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "old_text": { "type": "string", "maxLength": 800 }, "new_text": { "type": "string", "maxLength": 800 } }, "required": ["path", "old_text", "new_text"] }),
        },
    }
}

// ── read_file ────────────────────────────────────────────────

pub async fn tool_read_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let offset = args["offset"].as_u64().unwrap_or(0);
    let limit = args["limit"].as_u64().unwrap_or(u64::MAX);
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("read_file", path, "path_error", &e),
    };
    let is_full_read = offset == 0 && limit == u64::MAX;
    if is_full_read {
        if let Some(cached) = cache_lookup(&full) {
            return cached;
        }
    }
    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => {
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
    let is_chunked = offset > 0 || limit < u64::MAX;
    if !is_chunked && total > 200 {
        return ToolOutput::error(
            "read_file",
            path,
            "too_large",
            &format!(
                "文件过大（共 {} 行）。请用 offset 和 limit 参数分段读取。",
                total
            ),
        );
    }
    let start = (offset as usize).min(total);
    let end = (start + (limit as usize).min(total - start)).min(total);
    let excerpt: Vec<String> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>4}| {}", start + i + 1, line))
        .collect();
    let meta = format!("{}（共 {} 行，显示 {}-{}）", path, total, start + 1, end);
    ToolOutput::read_file(
        "read_file",
        path,
        &format!("{}\n{}", meta, excerpt.join("\n")),
    )
}

pub fn read_file_tool_def(description: &str) -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "read_file".into(),
            description: description.into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "offset": { "type": "integer" }, "limit": { "type": "integer" } }, "required": ["path"] }),
        },
    }
}

// ── create_file ──────────────────────────────────────────────

pub async fn tool_create_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("create_file", "", "empty_path", "文件路径为空");
    }
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

pub fn create_file_tool_def(description: &str) -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_file".into(),
            description: format!(
                "{}。content ≤8000 字符。大文件用 apply_patch 一次性写入。",
                description
            ),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "content": { "type": "string", "maxLength": 8000 } }, "required": ["path", "content"] }),
        },
    }
}

// ── list_dir ─────────────────────────────────────────────────

pub async fn tool_list_dir(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("list_dir", path, "path_error", &e),
    };
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

pub fn list_dir_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "list_dir".into(),
            description: "列出目录下的文件和子目录".into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }),
        },
    }
}

// ── list_dir_tree ────────────────────────────────────────────

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
    const MAX_ITEMS: usize = 200;

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
    ) {
        if *total >= MAX_ITEMS {
            return;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        let mut items: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                if let Some(g) = glob {
                    simple_glob_match(&e.file_name().to_string_lossy(), g)
                } else {
                    true
                }
            })
            .collect();
        items.sort_by(|a, b| {
            let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (b_dir, a.file_name()).cmp(&(a_dir, b.file_name()))
        });
        for (i, entry) in items.iter().enumerate() {
            if *total >= MAX_ITEMS {
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let is_last = i == items.len() - 1;
            let connector = if is_last { "└── " } else { "├── " };
            let ep = entry.path();
            let rel = ep.strip_prefix(working_dir).unwrap_or(&ep);
            lines.push(format!("{}{}{}", prefix, connector, rel.display()));
            *total += 1;
            if is_dir && depth < max_depth {
                let child_prefix = format!("{}{}   ", prefix, if is_last { " " } else { "│" });
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
    );
    let mut output = if lines.is_empty() {
        "(空)".to_string()
    } else {
        lines.join("\n")
    };
    if total >= MAX_ITEMS {
        output.push_str(&format!("\n\n… 显示前 {} 项", MAX_ITEMS));
    }
    output.push_str(&format!("\n\n共 {} 项", total));
    ToolOutput::success_raw("list_dir_tree", &output)
}

pub fn list_dir_tree_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "list_dir_tree".into(),
            description: "递归目录树浏览。depth 默认 2，最大 5。支持 glob 过滤。".into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "depth": { "type": "integer" }, "glob": { "type": "string" } }, "required": ["path"] }),
        },
    }
}

// ── edit_file ────────────────────────────────────────────────

pub fn edit_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "edit_file".into(),
            description: "对已有文件进行 SEARCH/REPLACE 局部修改。一次只做一个替换块。".into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "search": { "type": "string" }, "replace": { "type": "string" } }, "required": ["path", "search", "replace"] }),
        },
    }
}

pub async fn tool_edit_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let search_text = args["search"].as_str().unwrap_or("");
    let replace_text = args["replace"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("edit_file", "", "empty_path", "文件路径为空");
    }
    if search_text.is_empty() {
        return ToolOutput::error("edit_file", path, "empty_search", "search 内容为空");
    }
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("edit_file", path, "path_error", &e),
    };
    if !full.exists() {
        return ToolOutput::error(
            "edit_file",
            path,
            "not_found",
            "文件不存在。新文件请用 create_file，全量覆盖请用 apply_patch（空 SEARCH）。",
        );
    }
    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("edit_file", path, "read_error", &e.to_string()),
    };
    let count = content.matches(search_text).count();
    if count == 0 {
        let preview = if search_text.len() > 120 {
            let cutoff = search_text.floor_char_boundary(120);
            format!("{}...", &search_text[..cutoff])
        } else {
            search_text.to_string()
        };
        return ToolOutput::error("edit_file", path, "search_not_found",
            &format!("SEARCH 文本未在文件中找到。\n查找的文本：\n```\n{}\n```\n请用 read_file 确认文件最新内容后再试。", preview));
    }
    if count > 1 {
        return ToolOutput::error(
            "edit_file",
            path,
            "search_ambiguous",
            &format!(
                "SEARCH 文本在文件中出现 {} 次，不唯一。请在 search 中包含更多上下文行。",
                count
            ),
        );
    }
    let new_content = content.replacen(search_text, replace_text, 1);
    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => ToolOutput::success(
            "edit_file",
            path,
            &format!(
                "成功替换 1 处文本（{} 字节 → {} 字节）",
                content.len(),
                new_content.len()
            ),
        ),
        Err(e) => ToolOutput::error("edit_file", path, "write_error", &e.to_string()),
    }
}

// ── search_text ──────────────────────────────────────────────

async fn try_rg_search(
    working_dir: &Path,
    pattern: &str,
    max_results: usize,
    glob: Option<&str>,
    case_sensitive: bool,
) -> Result<String, ()> {
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
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let output = cmd.output().await.map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results: Vec<String> = Vec::new();
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
            }
        }
    }
    if results.is_empty() {
        return Err(());
    }
    Ok(crate::tool::ToolOutput::success_raw(
        "search_text",
        &format!(
            "[ripgrep] 匹配 {} 处：\n{}",
            results.len(),
            results.join("\n")
        ),
    ))
}

pub async fn tool_search_text(working_dir: &Path, args: &serde_json::Value) -> String {
    let pattern = args["pattern"].as_str().unwrap_or("");
    if pattern.is_empty() {
        return ToolOutput::error("search_text", "", "empty_pattern", "搜索模式不能为空");
    }
    let max_results = args["max_results"].as_u64().unwrap_or(50) as usize;
    let glob = args["glob"].as_str().filter(|s| !s.is_empty());
    let case_sensitive = args["case_sensitive"].as_bool().unwrap_or(true);

    if let Ok(result) = try_rg_search(working_dir, pattern, max_results, glob, case_sensitive).await
    {
        return result;
    }

    let mut results: Vec<String> = Vec::new();
    let mut file_count = 0usize;
    let mut error_count = 0usize;
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
                if let Some(g) = glob {
                    if !simple_glob_match(&name, g) {
                        continue;
                    }
                }
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
        let searched = format!(
            "在 {} 文件中搜索「{}」{}",
            file_count,
            pattern,
            if let Some(g) = glob {
                format!("（{}）", g)
            } else {
                String::new()
            }
        );
        return ToolOutput::success_raw(
            "search_text",
            &format!(
                "{}，未找到匹配{}",
                searched,
                if error_count > 0 {
                    format!("（{} 个文件跳过）", error_count)
                } else {
                    String::new()
                }
            ),
        );
    }
    let summary = format!(
        "找到 {} 个匹配（共扫描 {} 文件）：\n{}",
        results.len(),
        file_count,
        results.join("\n")
    );
    ToolOutput::success_raw("search_text", &summary)
}

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
        for i in ni..=name.len() {
            if glob_match_inner(name, glob, i, gi + 1) {
                return true;
            }
        }
        false
    } else if glob[gi] == '?' {
        ni < name.len() && glob_match_inner(name, glob, ni + 1, gi + 1)
    } else {
        ni < name.len() && name[ni] == glob[gi] && glob_match_inner(name, glob, ni + 1, gi + 1)
    }
}

pub fn search_text_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "search_text".into(),
            description: "在项目文件中递归搜索文本模式。返回 文件:行号:内容 格式。".into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "pattern": { "type": "string" }, "glob": { "type": "string" }, "max_results": { "type": "integer" }, "case_sensitive": { "type": "boolean" } }, "required": ["pattern"] }),
        },
    }
}
