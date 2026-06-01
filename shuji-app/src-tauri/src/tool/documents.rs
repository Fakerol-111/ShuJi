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
        let _lock = COUNTER_LOCK
            .lock()
            .map_err(|e| format!("计数器锁失败: {}", e))?;
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
    status: String,
    notes: String,
}

/// Parse the YAML frontmatter and body from a document string.
/// Returns (DocMeta, body_text) or an error.
fn parse_doc(content: &str) -> Result<(DocMeta, &str), String> {
    let body = content
        .strip_prefix("---\n")
        .ok_or_else(|| "缺少 YAML frontmatter 起始标记".to_string())?;
    let end = body
        .find("\n---")
        .ok_or_else(|| "缺少 YAML frontmatter 结束标记".to_string())?;
    let header = &body[..end];
    let body_text = body[end + 4..].trim_start();

    let mut id = String::new();
    let mut doc_type = String::new();
    let mut author = String::new();
    let mut timestamp = String::new();
    let mut refs = String::from("[-1]");
    let mut status = String::new();
    let mut notes = String::new();

    for line in header.lines() {
        if let Some((key, val)) = line.split_once(": ") {
            let val = val.trim();
            match key {
                "id" => id = val.to_string(),
                "type" => doc_type = val.to_string(),
                "author" => author = val.to_string(),
                "timestamp" => timestamp = val.to_string(),
                "refs" => refs = val.to_string(),
                "status" => status = val.to_string(),
                "notes" => notes = val.to_string(),
                _ => {}
            }
        }
    }

    if id.is_empty() {
        return Err("文档缺少 id 字段".to_string());
    }

    Ok((
        DocMeta {
            id,
            doc_type,
            author,
            timestamp,
            refs,
            status,
            notes,
        },
        body_text,
    ))
}

/// Build a full document string from metadata and body.
fn build_doc(meta: &DocMeta, body: &str) -> String {
    let mut frontmatter = format!(
        "---\nid: {}\ntype: {}\nauthor: {}\ntimestamp: {}\nrefs: {}",
        meta.id, meta.doc_type, meta.author, meta.timestamp, meta.refs,
    );
    if !meta.status.is_empty() {
        frontmatter += &format!("\nstatus: {}", meta.status);
    }
    if !meta.notes.is_empty() {
        frontmatter += &format!("\nnotes: {}", meta.notes);
    }
    format!("{}\n---\n{}", frontmatter, body)
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
    })
    .await
    .ok()
    .flatten()
}

/// ── create_document ────────────────────────────────────────────────
pub async fn tool_create_document(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let doc_type = args["type"].as_str().unwrap_or("").to_string();
    if doc_type.is_empty() {
        return ToolOutput::error("create_document", "", "empty_type", "文档类型不能为空");
    }
    let valid_types = [
        "dsgn", "plan", "pdsg", "ddtl", "task", "ctrt", "rprt", "revw", "precepts", "anls", "reqs",
    ];
    if !valid_types.contains(&doc_type.as_str()) {
        return ToolOutput::error(
            "create_document",
            "",
            "invalid_type",
            &format!(
                "无效文档类型: {}，支持的类型: {}",
                doc_type,
                valid_types.join(", ")
            ),
        );
    }

    let refs = args["refs"]
        .as_array()
        .map(|arr| {
            let nums: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_i64())
                .map(|n| n.to_string())
                .collect();
            if nums.is_empty() {
                "[-1]".to_string()
            } else {
                format!("[{}]", nums.join(", "))
            }
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
    let status = match doc_type.as_str() {
        "plan" | "revw" => "in_review".to_string(),
        _ => String::new(),
    };

    let meta = DocMeta {
        id: doc_id.clone(),
        doc_type: doc_type.clone(),
        author: author.to_string(),
        timestamp: ts,
        refs,
        status: status.clone(),
        notes: String::new(),
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
        Ok(_) => {
            // Track pending approvals for plan/revw docs
            if status == "in_review" {
                let _ = add_pending_approval(working_dir, &doc_id).await;
            }
            ToolOutput::success(
                "create_document",
                &doc_id,
                &format!("文档 {} 创建成功", doc_id),
            )
        }
        Err(e) => ToolOutput::error("create_document", &doc_id, "write_error", &e.to_string()),
    }
}

/// ── update_document ────────────────────────────────────────────────
pub async fn tool_modify_document(
    working_dir: &Path,
    args: &serde_json::Value,
    _dept: &str,
) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error("modify_document", "", "empty_id", "文档 ID 不能为空");
    }

    let type_prefix = id.split('_').next().unwrap_or("");
    let full = if type_prefix == "rprt" {
        match find_rprt_path(working_dir, id).await {
            Some(p) => p,
            None => {
                return ToolOutput::error(
                    "modify_document",
                    id,
                    "not_found",
                    &format!("文档 {} 不存在", id),
                )
            }
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
        return ToolOutput::error(
            "modify_document",
            id,
            "not_found",
            &format!("文档 {} 不存在", id),
        );
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
            return ToolOutput::error(
                "modify_document",
                id,
                "not_found",
                "未在文档正文中找到匹配的文本。请先 read_file 确认内容。",
            );
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
pub async fn tool_append_document(
    working_dir: &Path,
    args: &serde_json::Value,
    _dept: &str,
) -> String {
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
            None => {
                return ToolOutput::error(
                    "append_document",
                    id,
                    "not_found",
                    &format!("文档 {} 不存在", id),
                )
            }
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
        return ToolOutput::error(
            "append_document",
            id,
            "not_found",
            &format!("文档 {} 不存在", id),
        );
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

/// ── Pending approvals ───────────────────────────────────────────
/// Add a document ID to the pending approvals list.
pub async fn add_pending_approval(working_dir: &Path, doc_id: &str) -> Result<(), String> {
    let path = working_dir.join(".shuji/pending_approvals.json");
    let shuji_dir = path.parent().ok_or("路径错误")?;
    let _ = tokio::fs::create_dir_all(shuji_dir).await;
    let mut list: Vec<String> = tokio::fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if !list.contains(&doc_id.to_string()) {
        list.push(doc_id.to_string());
    }
    tokio::fs::write(&path, serde_json::to_string(&list).unwrap())
        .await
        .map_err(|e| e.to_string())
}

/// Remove a document ID from the pending approvals list.
pub async fn remove_pending_approval(working_dir: &Path, doc_id: &str) -> Result<(), String> {
    let path = working_dir.join(".shuji/pending_approvals.json");
    let mut list: Vec<String> = tokio::fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    list.retain(|id| id != doc_id);
    tokio::fs::write(&path, serde_json::to_string(&list).unwrap())
        .await
        .map_err(|e| e.to_string())
}

/// Get the first pending approval doc ID, if any.
pub async fn get_first_pending_approval(working_dir: &Path) -> Option<String> {
    let path = working_dir.join(".shuji/pending_approvals.json");
    let content = tokio::fs::read_to_string(&path).await.ok()?;
    let list: Vec<String> = serde_json::from_str(&content).ok()?;
    list.into_iter().next()
}

/// ── set_document_status ──────────────────────────────────────────
/// Must-approve document types that require emperor approval.
const MUST_APPROVE_TYPES: &[&str] = &["plan", "revw"];

/// Resolve the full path for a document by its ID.
async fn resolve_doc_path(working_dir: &Path, id: &str) -> Result<PathBuf, String> {
    let type_prefix = id.split('_').next().unwrap_or("");
    if type_prefix == "rprt" {
        find_rprt_path(working_dir, id)
            .await
            .ok_or_else(|| format!("文档 {} 不存在", id))
    } else if MUST_APPROVE_TYPES.contains(&type_prefix) {
        let dir = type_to_dir(type_prefix);
        let rel_path = format!(".shuji/{}/{}.md", dir, id);
        resolve_scoped_path(working_dir, &rel_path)
            .await
            .map_err(|e| format!("路径错误: {}", e))
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        resolve_scoped_path(working_dir, &rel_path)
            .await
            .map_err(|e| format!("路径错误: {}", e))
    }
}

pub async fn tool_set_document_status(working_dir: &Path, args: &serde_json::Value) -> String {
    let id = args["id"].as_str().unwrap_or("");
    let new_status = args["status"].as_str().unwrap_or("");
    if id.is_empty() || new_status.is_empty() {
        return ToolOutput::error(
            "set_document_status",
            "",
            "empty_params",
            "id 和 status 不能为空",
        );
    }
    if !matches!(new_status, "approved" | "rejected") {
        return ToolOutput::error(
            "set_document_status",
            id,
            "invalid_status",
            "status 必须是 approved 或 rejected",
        );
    }

    let full = match resolve_doc_path(working_dir, id).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("set_document_status", id, "not_found", &e),
    };
    if !full.exists() {
        return ToolOutput::error(
            "set_document_status",
            id,
            "not_found",
            &format!("文档 {} 不存在", id),
        );
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(e) => {
            return ToolOutput::error("set_document_status", id, "read_error", &e.to_string())
        }
    };

    let (mut meta, body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error("set_document_status", id, "parse_error", &e),
    };

    // Only allow status change on must-approve types
    if !MUST_APPROVE_TYPES.contains(&meta.doc_type.as_str()) {
        return ToolOutput::error(
            "set_document_status",
            id,
            "wrong_type",
            "只有 plan 和 revw 类型文档可以设置审批状态",
        );
    }

    meta.status = new_status.to_string();
    meta.notes = args["emperor_note"].as_str().unwrap_or("").to_string();
    meta.timestamp = now_iso();

    let new_content = build_doc(&meta, body);
    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => {
            let _ = remove_pending_approval(working_dir, id).await;
            ToolOutput::success(
                "set_document_status",
                id,
                &format!("文档 {} 状态已设为 {}", id, new_status),
            )
        }
        Err(e) => ToolOutput::error("set_document_status", id, "write_error", &e.to_string()),
    }
}

pub fn set_document_status_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "set_document_status".into(),
            description: "设置文档审批状态（approved/rejected）。仅适用于 plan、revw 类型文档。调用后文档状态变更，下游部门可继续执行。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "文档 ID，如 plan_5 或 revw_3"
                    },
                    "status": {
                        "type": "string",
                        "enum": ["approved", "rejected"],
                        "description": "批准(approved)或驳回(rejected)"
                    },
                    "emperor_note": {
                        "type": "string",
                        "description": "皇帝御批备注（可选）"
                    }
                },
                "required": ["id", "status"]
            }),
        },
    }
}

/// ── Approval gate helpers ────────────────────────────────────────
/// Parse refs string like "[3, 4]" or "[-1]" into a Vec<u64>.
fn parse_refs(refs: &str) -> Vec<u64> {
    let inner = refs.trim().trim_start_matches('[').trim_end_matches(']');
    if inner.is_empty() || inner == "-1" {
        return vec![];
    }
    inner
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

/// Check whether any must-approve document referenced by the given doc
/// is still `in_review`. Returns Err if any referenced plan/revw is pending.
pub async fn check_doc_refs_approved_for_route(
    working_dir: &Path,
    subject_doc_id: &str,
) -> Result<(), String> {
    let full = match resolve_doc_path(working_dir, subject_doc_id).await {
        Ok(p) => p,
        Err(_) => return Ok(()), // doc doesn't exist, gate passes
    };
    if !full.exists() {
        return Ok(());
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let (meta, _body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };

    let ref_nums = parse_refs(&meta.refs);
    if ref_nums.is_empty() {
        return Ok(());
    }

    for num in ref_nums {
        for prefix in MUST_APPROVE_TYPES {
            let ref_id = format!("{}_{}", prefix, num);
            let ref_path = match resolve_doc_path(working_dir, &ref_id).await {
                Ok(p) => p,
                Err(_) => continue,
            };
            if !ref_path.exists() {
                continue;
            }
            if let Ok(ref_content) = tokio::fs::read_to_string(&ref_path).await {
                if let Ok((ref_meta, _)) = parse_doc(&ref_content) {
                    if ref_meta.status == "in_review" {
                        return Err(format!(
                            "文档 {} 引用了 {}，该文档尚在待皇帝御批中（status: in_review）。请先处理审批。",
                            subject_doc_id, ref_id
                        ));
                    }
                    if ref_meta.status == "rejected" {
                        return Err(format!(
                            "文档 {} 引用了 {}，该文档已被驳回（status: rejected）。请先处理驳回意见。",
                            subject_doc_id, ref_id
                        ));
                    }
                }
            }
        }
    }

    Ok(())
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
            None => ToolOutput::error(
                "find_document",
                id,
                "not_found",
                &format!("文档 {} 不存在", id),
            ),
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
            Ok(_) => ToolOutput::error(
                "find_document",
                id,
                "not_found",
                &format!("文档 {} 不存在", id),
            ),
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
