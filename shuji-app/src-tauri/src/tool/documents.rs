use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::tool::{resolve_scoped_path, ToolOutput};

/// ── Counter ─────────────────────────────────────────────────────────

static COUNTER_LOCK: Mutex<()> = Mutex::new(());

/// Get the next ID from the project-local counter at `.shuji/_counter`.
async fn next_id(working_dir: &Path) -> Result<u64, String> {
    let counter_path = working_dir.join(".shuji/_counter");

    // Use spawn_blocking so the std::sync::MutexGuard is never held across .await.
    tokio::task::spawn_blocking(move || {
        let _lock = COUNTER_LOCK.lock().map_err(|e| format!("计数器锁失败: {}", e))?;
        let current: u64 = std::fs::read_to_string(&counter_path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(1);
        std::fs::write(&counter_path, (current + 1).to_string())
            .map_err(|e| format!("计数器写入失败: {}", e))?;
        Ok(current)
    })
    .await
    .map_err(|_| "后台任务异常: next_id".to_string())?
}

/// ── YAML frontmatter helpers ───────────────────────────────────────

struct DocMeta {
    id: String,
    doc_type: String,
    author: String,
    timestamp: String,
    refs: String,
}

/// Parse the YAML frontmatter and body from a document string.
/// Returns (DocMeta, body_text) or an error.
fn parse_doc(content: &str) -> Result<(DocMeta, &str), String> {
    let body = content.strip_prefix("---\n").ok_or_else(|| "缺少 YAML frontmatter 起始标记".to_string())?;
    let end = body.find("\n---").ok_or_else(|| "缺少 YAML frontmatter 结束标记".to_string())?;
    let header = &body[..end];
    let body_text = body[end + 4..].trim_start();

    let mut id = String::new();
    let mut doc_type = String::new();
    let mut author = String::new();
    let mut timestamp = String::new();
    let mut refs = String::from("[-1]");

    for line in header.lines() {
        if let Some((key, val)) = line.split_once(": ") {
            let val = val.trim();
            match key {
                "id" => id = val.to_string(),
                "type" => doc_type = val.to_string(),
                "author" => author = val.to_string(),
                "timestamp" => timestamp = val.to_string(),
                "refs" => refs = val.to_string(),
                _ => {}
            }
        }
    }

    if id.is_empty() {
        return Err("文档缺少 id 字段".to_string());
    }

    Ok((DocMeta { id, doc_type, author, timestamp, refs }, body_text))
}

/// Build a full document string from metadata and body.
fn build_doc(meta: &DocMeta, body: &str) -> String {
    format!(
        "---\nid: {}\ntype: {}\nauthor: {}\ntimestamp: {}\nrefs: {}\n---\n{}",
        meta.id, meta.doc_type, meta.author, meta.timestamp, meta.refs, body
    )
}

/// Timestamp string for the current moment.
fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Map English dept string to Chinese author name.
fn dept_to_author(dept: &str) -> &'static str {
    match dept {
        "zhongshuling" => "中书令",
        "menxiashizhong" => "门下侍中",
        "neige" => "内阁",
        "shangshuling" => "尚书令",
        "libushangshu" => "吏部",
        "bingbushangshu" => "兵部",
        "gongbushangshu" => "工部",
        "xingbushangshu" => "刑部",
        "liburshangshu" => "礼部",
        "zhisi" => "制司",
        "hubu" => "户部",
        _ => "未知",
    }
}

/// Map document type prefix to subdirectory under `.shuji/`.
/// Returns empty string for root-level files (e.g. precepts).
/// For reports, returns the reports base dir — dept subfolder is appended separately.
fn type_to_dir(doc_type: &str) -> &'static str {
    match doc_type {
        "dsgn" | "plan" | "pdsg" => "designs",
        "ddtl" => "designs/detail",
        "revw" => "reviews",
        "task" => "tasks",
        "ctrt" => "contracts",
        "rprt" => "reports",
        "anls" => "analysis",
        "reqs" => "requirements",
        "precepts" => "",
        _ => "misc",
    }
}

/// Build the full relative path for a rprt document, scoped to the author department.
fn rprt_rel_path(dept: &str, doc_id: &str) -> String {
    format!(".shuji/reports/{}/{}.md", dept, doc_id)
}

/// Search for a report document across all dept subdirectories.
async fn find_rprt_path(working_dir: &Path, id: &str) -> Option<PathBuf> {
    let reports_dir = working_dir.join(".shuji/reports");
    let id = id.to_string();
    tokio::task::spawn_blocking(move || {
        let entries = std::fs::read_dir(&reports_dir).ok()?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let candidate = path.join(format!("{}.md", id));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
        None
    }).await.ok().flatten()
}

/// ── create_document ────────────────────────────────────────────────

pub async fn tool_create_document(working_dir: &Path, args: &serde_json::Value, dept: &str) -> String {
    let doc_type = args["type"].as_str().unwrap_or("").to_string();
    if doc_type.is_empty() {
        return ToolOutput::error("create_document", "", "empty_type", "文档类型不能为空");
    }
    let valid_types = ["dsgn", "plan", "pdsg", "ddtl", "task", "ctrt", "rprt", "revw", "precepts", "anls", "reqs"];
    if !valid_types.contains(&doc_type.as_str()) {
        return ToolOutput::error("create_document", "", "invalid_type",
            &format!("无效文档类型: {}，支持的类型: {}", doc_type, valid_types.join(", ")));
    }

    let refs = args["refs"].as_array()
        .map(|arr| {
            let nums: Vec<String> = arr.iter()
                .filter_map(|v| v.as_i64())
                .map(|n| n.to_string())
                .collect();
            if nums.is_empty() { "[-1]".to_string() } else { format!("[{}]", nums.join(", ")) }
        })
        .unwrap_or_else(|| "[-1]".to_string());

    let id_num = match next_id(working_dir).await {
        Ok(n) => n,
        Err(e) => return ToolOutput::error("create_document", "", "counter_error", &e),
    };
    let doc_id = format!("{}_{}", doc_type, id_num);
    let rel_path = if doc_type == "rprt" {
        rprt_rel_path(dept, &doc_id)
    } else {
        let dir = type_to_dir(&doc_type);
        if dir.is_empty() {
            format!(".shuji/{}.md", doc_id)
        } else {
            format!(".shuji/{}/{}.md", dir, doc_id)
        }
    };

    let author = dept_to_author(dept);
    let ts = now_iso();

    let meta = DocMeta {
        id: doc_id.clone(),
        doc_type: doc_type.clone(),
        author: author.to_string(),
        timestamp: ts,
        refs,
    };
    let content = build_doc(&meta, "");

    // Resolve the path and write
    let full = match resolve_scoped_path(working_dir, &rel_path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("create_document", &doc_id, "path_error", &e),
    };
    if let Some(parent) = full.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::write(&full, &content).await {
        Ok(_) => ToolOutput::success("create_document", &doc_id, &format!("文档 {} 创建成功", doc_id)),
        Err(e) => ToolOutput::error("create_document", &doc_id, "write_error", &e.to_string()),
    }
}

/// ── update_document ────────────────────────────────────────────────

pub async fn tool_modify_document(working_dir: &Path, args: &serde_json::Value, _dept: &str) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error("modify_document", "", "empty_id", "文档 ID 不能为空");
    }

    let type_prefix = id.split('_').next().unwrap_or("");
    let full = if type_prefix == "rprt" {
        match find_rprt_path(working_dir, id).await {
            Some(p) => p,
            None => return ToolOutput::error("modify_document", id, "not_found", &format!("文档 {} 不存在", id)),
        }
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        match resolve_scoped_path(working_dir, &rel_path).await {
            Ok(p) => p,
            Err(e) => return ToolOutput::error("modify_document", id, "path_error", &e),
        }
    };
    if !full.exists() {
        return ToolOutput::error("modify_document", id, "not_found", &format!("文档 {} 不存在", id));
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("modify_document", id, "read_error", &e.to_string()),
    };

    let (mut meta, body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error("modify_document", id, "parse_error", &e),
    };

    // Apply text replacement
    let new_body = if let Some(old_text) = args["old_text"].as_str() {
        let new_text = args["new_text"].as_str().unwrap_or("");
        if old_text.is_empty() {
            return ToolOutput::error("modify_document", id, "empty_old_text", "old_text 不能为空");
        }
        if !body.contains(old_text) {
            return ToolOutput::error("modify_document", id, "not_found",
                "未在文档正文中找到匹配的文本。请先 read_file 确认内容。");
        }
        body.replacen(old_text, new_text, 1)
    } else {
        body.to_string()
    };

    meta.timestamp = now_iso();
    let new_content = build_doc(&meta, &new_body);

    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => ToolOutput::success("modify_document", id, "修改成功"),
        Err(e) => ToolOutput::error("modify_document", id, "write_error", &e.to_string()),
    }
}

/// Append content to an existing document's body.
pub async fn tool_append_document(working_dir: &Path, args: &serde_json::Value, _dept: &str) -> String {
    let id = args["id"].as_str().unwrap_or("");
    let append_content = args["content"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error("append_document", "", "empty_id", "文档 ID 不能为空");
    }
    if append_content.is_empty() {
        return ToolOutput::error("append_document", id, "empty_content", "追加内容不能为空");
    }

    let type_prefix = id.split('_').next().unwrap_or("");
    let full = if type_prefix == "rprt" {
        match find_rprt_path(working_dir, id).await {
            Some(p) => p,
            None => return ToolOutput::error("append_document", id, "not_found", &format!("文档 {} 不存在", id)),
        }
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        match resolve_scoped_path(working_dir, &rel_path).await {
            Ok(p) => p,
            Err(e) => return ToolOutput::error("append_document", id, "path_error", &e),
        }
    };
    if !full.exists() {
        return ToolOutput::error("append_document", id, "not_found", &format!("文档 {} 不存在", id));
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("append_document", id, "read_error", &e.to_string()),
    };

    let (mut meta, body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error("append_document", id, "parse_error", &e),
    };

    let new_body = if body.is_empty() {
        append_content.to_string()
    } else {
        format!("{}\n{}", body, append_content)
    };

    meta.timestamp = now_iso();
    let new_content = build_doc(&meta, &new_body);

    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => ToolOutput::success("append_document", id, "追加成功"),
        Err(e) => ToolOutput::error("append_document", id, "write_error", &e.to_string()),
    }
}

/// ── Tool definitions ──────────────────────────────────────────────

pub fn create_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_document".into(),
            description: "创建新文档，系统自动分配 ID、生成 YAML 头部，返回文档 ID。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["dsgn", "plan", "pdsg", "ddtl", "task", "ctrt", "rprt", "revw", "anls", "reqs"],
                        "description": "dsgn/plan/pdsg/ddtl/revw/task/ctrt/rprt/anls/reqs"
                    },
                    "refs": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "引用的文档ID列表（整数，不带类型前缀）。无引用传 []"
                    }
                },
                "required": ["type", "refs"]
            }),
        },
    }
}

pub fn modify_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "modify_document".into(),
            description: "替换文档正文中的文本 (find+replace)。≤300字符。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "文档 ID，如 dsgn_003"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "待替换文本（≤300字符）",
                        "maxLength": 300
                    },
                    "new_text": {
                        "type": "string",
                        "description": "新文本（≤300字符）",
                        "maxLength": 300
                    }
                },
                "required": ["id", "old_text", "new_text"]
            }),
        },
    }
}

pub fn append_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "append_document".into(),
            description: "追加内容到已有文档的正文末尾。每次调用的 content 参数不超过 2000 字符，充分利用单次调用容量。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "文档 ID，如 dsgn_003"
                    },
                    "content": {
                        "type": "string",
                        "description": "要追加的内容（每次最多 2000 字符）",
                        "maxLength": 2000
                    }
                },
                "required": ["id", "content"]
            }),
        },
    }
}

/// ── find_document ─────────────────────────────────────────────────

pub async fn tool_find_document(working_dir: &Path, args: &serde_json::Value) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error("find_document", "", "empty_id", "文档 ID 不能为空");
    }

    let type_prefix = id.split('_').next().unwrap_or("");

    if type_prefix == "rprt" {
        match find_rprt_path(working_dir, id).await {
            Some(p) => {
                let rel = p.strip_prefix(working_dir).unwrap_or(&p);
                ToolOutput::success("find_document", id, &format!("{}", rel.display()))
            }
            None => ToolOutput::error("find_document", id, "not_found", &format!("文档 {} 不存在", id)),
        }
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        match resolve_scoped_path(working_dir, &rel_path).await {
            Ok(full) if full.exists() => {
                let rel = full.strip_prefix(working_dir).unwrap_or(&full);
                ToolOutput::success("find_document", id, &format!("{}", rel.display()))
            }
            Ok(_) => ToolOutput::error("find_document", id, "not_found", &format!("文档 {} 不存在", id)),
            Err(e) => ToolOutput::error("find_document", id, "path_error", &e),
        }
    }
}

pub fn find_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "find_document".into(),
            description: "根据文档ID查找文档路径".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "文档 ID，如 rprt_32, dsgn_003, task_5"
                    }
                },
                "required": ["id"]
            }),
        },
    }
}
