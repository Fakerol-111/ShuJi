use std::path::Path;

use crate::tool::path::resolve_scoped_path;
use crate::tool::ToolOutput;

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
