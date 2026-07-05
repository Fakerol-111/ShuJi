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
        return ToolOutput::error("append_file", "", "empty_path", "File path is empty");
    }
    if content.is_empty() {
        return ToolOutput::error(
            "append_file",
            path,
            "empty_content",
            "Append content is empty",
        );
    }
    if content.len() > 6000 {
        return ToolOutput::error("append_file", path, "content_too_long",
            &format!("content length {} exceeds max 6000 chars. Split into multiple append_file calls, each ≤6000 chars.", content.len()));
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
            ToolOutput::success("append_file", path, "Appended successfully")
        }
        Err(e) => ToolOutput::error("append_file", path, "open_error", &e.to_string()),
    }
}

pub fn append_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "append_file".into(),
            description: "Append content to the end of a file. content ≤6000 chars; split large files into multiple writes.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path, relative to project root" },
                    "content": { "type": "string", "description": "Content to append (max 6000 chars per call)", "maxLength": 6000 }
                },
                "required": ["path", "content"]
            }),
        },
    }
}

// ── modify_file ──────────────────────────────────────────────

pub async fn tool_modify_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("modify_file", "", "empty_path", "File path is empty");
    }
    let full = match resolve_scoped_path(working_dir, path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("modify_file", path, "path_error", &e),
    };
    if !full.exists() {
        return ToolOutput::error("modify_file", path, "not_found", "File does not exist");
    }
    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("modify_file", path, "read_error", &e.to_string()),
    };
    let old_text = args["old_text"].as_str().unwrap_or("");
    let new_text = args["new_text"].as_str().unwrap_or("");
    if old_text.is_empty() {
        return ToolOutput::error(
            "modify_file",
            path,
            "empty_old_text",
            "old_text cannot be empty",
        );
    }
    if old_text.len() > 3000 || new_text.len() > 3000 {
        return ToolOutput::error(
            "modify_file",
            path,
            "text_too_long",
            "Text exceeds 3000 char limit. For large changes use apply_patch (supports 50000 chars).",
        );
    }
    if !content.contains(old_text) {
        return ToolOutput::error("modify_file", path, "not_found",
            "Could not find matching text in the file. Use read_file to confirm the content and ensure old_text exactly matches (including whitespace and indentation).");
    }
    let new_content = content.replacen(old_text, new_text, 1);
    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => ToolOutput::success(
            "modify_file",
            path,
            &format!("Replaced successfully (replaced {} bytes)", old_text.len()),
        ),
        Err(e) => ToolOutput::error("modify_file", path, "write_error", &e.to_string()),
    }
}

pub fn modify_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "modify_file".into(),
            description: "Replace text in a file (find+replace). ≤3000 chars. For large changes use apply_patch."
                .into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "old_text": { "type": "string", "maxLength": 3000 }, "new_text": { "type": "string", "maxLength": 3000 } }, "required": ["path", "old_text", "new_text"] }),
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
    // Check cache first; if hit, use cached content to skip file I/O.
    // The cached content is raw file text — it still needs to go through
    // the normal formatting below (line splitting, excerpt, ToolOutput).
    let content = if is_full_read {
        if let Some(cached) = cache_lookup(working_dir, &full) {
            cached
        } else {
            match tokio::fs::read_to_string(&full).await {
                Ok(c) => {
                    if let Ok(meta) = tokio::fs::metadata(&full).await {
                        if let Ok(mtime) = meta.modified() {
                            cache_insert(working_dir, full.clone(), mtime, c.clone());
                        }
                    }
                    c
                }
                Err(e) => {
                    return ToolOutput::error("read_file", path, "read_error", &e.to_string())
                }
            }
        }
    } else {
        // Chunked reads bypass cache (the full content is cached above)
        match tokio::fs::read_to_string(&full).await {
            Ok(c) => c,
            Err(e) => return ToolOutput::error("read_file", path, "read_error", &e.to_string()),
        }
    };
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let is_chunked = offset > 0 || limit < u64::MAX;
    if !is_chunked && total > 200 {
        // Instead of returning a bare error, return the first 200 lines
        // as a preview. This prevents agents from getting stuck in a
        // read → too_large → retry loop when they don't use offset/limit.
        let preview_end = 200.min(total);
        let excerpt: Vec<String> = lines[0..preview_end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>4}| {}", i + 1, line))
            .collect();
        let meta = format!(
            "{} ({} lines total, showing 1-{}. Use offset={} to read the next chunk.)",
            path, total, preview_end, preview_end,
        );
        return ToolOutput::read_file(
            "read_file",
            path,
            &format!("{}\n{}", meta, excerpt.join("\n")),
        );
    }
    let start = (offset as usize).min(total);
    let end = (start + (limit as usize).min(total - start)).min(total);
    let excerpt: Vec<String> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>4}| {}", start + i + 1, line))
        .collect();
    let meta = format!(
        "{} ({} lines total, showing {}-{})",
        path,
        total,
        start + 1,
        end
    );
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

// ── edit_file ────────────────────────────────────────────────

pub fn edit_file_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "edit_file".into(),
            description:
                "Apply a SEARCH/REPLACE edit to an existing file. One replacement block per call."
                    .into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "search": { "type": "string" }, "replace": { "type": "string" } }, "required": ["path", "search", "replace"] }),
        },
    }
}

pub async fn tool_edit_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let search_text = args["search"].as_str().unwrap_or("");
    let replace_text = args["replace"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("edit_file", "", "empty_path", "File path is empty");
    }
    if search_text.is_empty() {
        return ToolOutput::error("edit_file", path, "empty_search", "search content is empty");
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
            "File does not exist. For new files use create_file; for full overwrite use apply_patch (empty SEARCH).",
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
            &format!("SEARCH text not found in the file.\nSearched for:\n```\n{}\n```\nPlease use read_file to confirm the latest content and try again.", preview));
    }
    if count > 1 {
        return ToolOutput::error(
            "edit_file",
            path,
            "search_ambiguous",
            &format!(
                "SEARCH text appears {} times in the file, not unique. Include more context lines in search.",
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
                "Successfully replaced 1 occurrence ({} bytes -> {} bytes)",
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
            "[ripgrep] {} matches found:\n{}",
            results.len(),
            results.join("\n")
        ),
    ))
}

pub async fn tool_search_text(working_dir: &Path, args: &serde_json::Value) -> String {
    let pattern = args["pattern"].as_str().unwrap_or("");
    if pattern.is_empty() {
        return ToolOutput::error(
            "search_text",
            "",
            "empty_pattern",
            "Search pattern cannot be empty",
        );
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
            "Searched in {} files for '{}'{}",
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
                "{} — no matches found{}",
                searched,
                if error_count > 0 {
                    format!("({} files skipped)", error_count)
                } else {
                    String::new()
                }
            ),
        );
    }
    let summary = format!(
        "Found {} matches (scanned {} files):\n{}",
        results.len(),
        file_count,
        results.join("\n")
    );
    ToolOutput::success_raw("search_text", &summary)
}

pub(crate) fn simple_glob_match(name: &str, glob: &str) -> bool {
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
            description: "Recursively search for a text pattern in project files. Returns file:line:content format.".into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "pattern": { "type": "string" }, "glob": { "type": "string" }, "max_results": { "type": "integer" }, "case_sensitive": { "type": "boolean" } }, "required": ["pattern"] }),
        },
    }
}
