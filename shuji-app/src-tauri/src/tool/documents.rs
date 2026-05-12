use std::path::Path;
use std::sync::Mutex;

use crate::tool::{resolve_scoped_path, ToolOutput};

/// ── Counter ─────────────────────────────────────────────────────────

static COUNTER_LOCK: Mutex<()> = Mutex::new(());

/// Get the next ID from the project-local counter at `.shuji/_counter`.
fn next_id(working_dir: &Path) -> Result<u64, String> {
    let _lock = COUNTER_LOCK.lock().map_err(|e| format!("计数器锁失败: {}", e))?;

    let counter_path = working_dir.join(".shuji/_counter");
    let current: u64 = std::fs::read_to_string(&counter_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1);

    std::fs::write(&counter_path, (current + 1).to_string())
        .map_err(|e| format!("计数器写入失败: {}", e))?;

    Ok(current)
}

/// ── YAML frontmatter helpers ───────────────────────────────────────

struct DocMeta {
    id: String,
    doc_type: String,
    status: String,
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
    let mut status = String::new();
    let mut author = String::new();
    let mut timestamp = String::new();
    let mut refs = String::from("[-1]");

    for line in header.lines() {
        if let Some((key, val)) = line.split_once(": ") {
            let val = val.trim();
            match key {
                "id" => id = val.to_string(),
                "type" => doc_type = val.to_string(),
                "status" => status = val.to_string(),
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

    Ok((DocMeta { id, doc_type, status, author, timestamp, refs }, body_text))
}

/// Build a full document string from metadata and body.
fn build_doc(meta: &DocMeta, body: &str) -> String {
    format!(
        "---\nid: {}\ntype: {}\nstatus: {}\nauthor: {}\ntimestamp: {}\nrefs: {}\n---\n{}",
        meta.id, meta.doc_type, meta.status, meta.author, meta.timestamp, meta.refs, body
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
        "menxiajishizhong" => "门下给事中",
        "neige" => "内阁",
        "shangshuling" => "尚书令",
        "libushangshu" => "吏部尚书",
        "bingbushangshu" => "兵部尚书",
        "gongbushangshu" => "工部尚书",
        "xingbushangshu" => "刑部尚书",
        "liburshangshu" => "礼部尚书",
        "zhisi" => "制司",
        "hubu" => "户部",
        _ => "未知",
    }
}

/// Map document type prefix to subdirectory under `.shuji/`.
fn type_to_dir(doc_type: &str) -> &'static str {
    match doc_type {
        "dsgn" | "plan" | "pdsg" => "designs",
        "ddtl" => "designs/detail",
        "revw" => "reviews",
        "task" => "tasks",
        "ctrt" => "contracts",
        "rprt" => "reports",
        _ => "misc",
    }
}

/// Determine initial status for a document type.
fn initial_status(doc_type: &str) -> &'static str {
    match doc_type {
        "dsgn" | "plan" | "pdsg" => "draft",
        _ => "todo",
    }
}

/// ── create_document ────────────────────────────────────────────────

pub fn tool_create_document(working_dir: &Path, args: &serde_json::Value, dept: &str) -> String {
    let doc_type = args["type"].as_str().unwrap_or("").to_string();
    if doc_type.is_empty() {
        return ToolOutput::error("create_document", "", "empty_type", "文档类型不能为空");
    }
    let valid_types = ["dsgn", "plan", "pdsg", "ddtl", "task", "ctrt", "rprt", "revw"];
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

    let id_num = match next_id(working_dir) {
        Ok(n) => n,
        Err(e) => return ToolOutput::error("create_document", "", "counter_error", &e),
    };
    let doc_id = format!("{}_{}", doc_type, id_num);
    let dir = type_to_dir(&doc_type);
    let rel_path = format!(".shuji/{}/{}.md", dir, doc_id);

    let status = initial_status(&doc_type);
    let author = dept_to_author(dept);
    let ts = now_iso();

    let meta = DocMeta {
        id: doc_id.clone(),
        doc_type: doc_type.clone(),
        status: status.to_string(),
        author: author.to_string(),
        timestamp: ts,
        refs,
    };
    let content = build_doc(&meta, "");

    // Resolve the path and write
    let full = match resolve_scoped_path(working_dir, &rel_path) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("create_document", &doc_id, "path_error", &e),
    };
    if let Some(parent) = full.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&full, &content) {
        Ok(_) => ToolOutput::success("create_document", &doc_id, &format!("文档 {} 创建成功", doc_id)),
        Err(e) => ToolOutput::error("create_document", &doc_id, "write_error", &e.to_string()),
    }
}

/// ── update_document ────────────────────────────────────────────────

pub fn tool_update_document(working_dir: &Path, args: &serde_json::Value, _dept: &str) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error("update_document", "", "empty_id", "文档 ID 不能为空");
    }

    // Parse id to find the file path
    let type_prefix = id.split('_').next().unwrap_or("");
    let dir = type_to_dir(type_prefix);
    let rel_path = format!(".shuji/{}/{}.md", dir, id);

    let full = match resolve_scoped_path(working_dir, &rel_path) {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("update_document", id, "path_error", &e),
    };
    if !full.exists() {
        return ToolOutput::error("update_document", id, "not_found", &format!("文档 {} 不存在", id));
    }

    let content = match std::fs::read_to_string(&full) {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("update_document", id, "read_error", &e.to_string()),
    };

    let (mut meta, body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error("update_document", id, "parse_error", &e),
    };

    // Apply changes
    let new_body = if let Some(new_content) = args["content"].as_str() {
        new_content.to_string()
    } else if let Some(append) = args["append"].as_str() {
        if body.is_empty() { append.to_string() } else { format!("{}\n{}", body, append) }
    } else {
        body.to_string()
    };

    if let Some(status) = args["status"].as_str() {
        // Validate status transition
        let valid = match meta.doc_type.as_str() {
            "dsgn" | "plan" | "pdsg" => matches!(status, "draft" | "approved" | "closed"),
            "ddtl" | "task" | "ctrt" | "rprt" | "revw" => matches!(status, "todo" | "done"),
            _ => true,
        };
        if !valid {
            return ToolOutput::error("update_document", id, "invalid_status",
                &format!("类型 {} 不支持状态 {}", meta.doc_type, status));
        }
        meta.status = status.to_string();
    }

    meta.timestamp = now_iso();
    let new_content = build_doc(&meta, &new_body);

    match std::fs::write(&full, &new_content) {
        Ok(_) => ToolOutput::success("update_document", id, "更新成功"),
        Err(e) => ToolOutput::error("update_document", id, "write_error", &e.to_string()),
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
                        "enum": ["dsgn", "plan", "pdsg", "ddtl", "task", "ctrt", "rprt", "revw"],
                        "description": "文档类型。dsgn=整体设计, plan=阶段规划, pdsg=阶段设计, ddtl=详细设计, revw=审查报告, task=任务, ctrt=协议(接口契约), rprt=报告"
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

pub fn update_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "update_document".into(),
            description: "更新已有文档的状态和/或正文。三个参数都是可选的，只需传需要变更的字段。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "文档 ID，如 dsgn_003"
                    },
                    "status": {
                        "type": "string",
                        "description": "新状态。dsgn/plan/pdsg: draft / approved / closed；其他: todo / done"
                    },
                    "content": {
                        "type": "string",
                        "description": "覆盖正文内容"
                    },
                    "append": {
                        "type": "string",
                        "description": "追加到正文末尾"
                    }
                },
                "required": ["id"]
            }),
        },
    }
}

