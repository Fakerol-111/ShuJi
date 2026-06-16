use std::path::Path;

use crate::tool::file_ops::edit::simple_glob_match;
use crate::tool::path::resolve_scoped_path;
use crate::tool::ToolOutput;

// ── create_file ──────────────────────────────────────────────

pub async fn tool_create_file(working_dir: &Path, args: &serde_json::Value) -> String {
    let path = args["path"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if path.is_empty() {
        return ToolOutput::error("create_file", "", "empty_path", "File path is empty");
    }
    if content.len() > 8000 {
        return ToolOutput::error(
            "create_file",
            path,
            "content_too_long",
            &format!(
                "content length {} exceeds max 8000 chars. Use create_file + append_file to write in chunks.",
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
            "File already exists — overwrite is not allowed. Use modify_file to edit, or delete_file first then create_file.");
    }
    if let Some(parent) = full.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::write(&full, content).await {
        Ok(_) => ToolOutput::success("create_file", path, "Written successfully"),
        Err(e) => ToolOutput::error("create_file", path, "write_error", &e.to_string()),
    }
}

pub fn create_file_tool_def(description: &str) -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_file".into(),
            description: format!(
                "{}. content ≤8000 chars. For large files use apply_patch to write in one go.",
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
                "(empty directory)".to_string()
            } else {
                items.join("\n")
            };
            ToolOutput::success_raw("list_dir", &message)
        }
        Ok(Err(e)) => ToolOutput::error("list_dir", path, "list_error", &e.to_string()),
        Err(_) => ToolOutput::error("list_dir", path, "join_error", "Background task failed"),
    }
}

pub fn list_dir_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "list_dir".into(),
            description: "List files and directories in a directory".into(),
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
        "(empty)".to_string()
    } else {
        lines.join("\n")
    };
    if total >= MAX_ITEMS {
        output.push_str(&format!("\n\n... showing first {} items", MAX_ITEMS));
    }
    output.push_str(&format!("\n\n{} items total", total));
    ToolOutput::success_raw("list_dir_tree", &output)
}

pub fn list_dir_tree_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "list_dir_tree".into(),
            description:
                "Recursive directory tree viewer. depth defaults to 2, max 5. Supports glob filter."
                    .into(),
            parameters: serde_json::json!({ "type": "object", "properties": { "path": { "type": "string" }, "depth": { "type": "integer" }, "glob": { "type": "string" } }, "required": ["path"] }),
        },
    }
}
