use std::path::Path;

use crate::tool::{resolve_scoped_path, ToolOutput};

use super::approval::add_pending_approval;
use super::parse::{
    build_doc, dept_to_author, find_rprt_path, list_doc_ids_in_subdir, next_id, normalize_doc_id,
    now_iso, parse_doc, parse_refs, resolve_doc_path, resolve_ref_doc_id, rprt_rel_path,
    type_to_dir, DocMeta, MUST_APPROVE_TYPES,
};

/// 返回某部门允许创建的文档类型白名单。
/// 返回 None 表示「不限制」（对未知部门保守放行，避免误伤）。
fn allowed_doc_types(dept: &str) -> Option<&'static [&'static str]> {
    use crate::models::role::Role;
    match dept.to_lowercase().as_str() {
        "requirements_agent" => return Some(&["reqs", "task"]),
        "survey_agent" => return Some(&["anls"]),
        _ => {}
    }
    let role = Role::from_name(dept)?;
    Some(match role {
        Role::Neige => &["task"],
        Role::Zhongshuling => &["dsgn", "plan", "pdsg", "anls", "precepts"],
        Role::MenxiaShizhong => &["revw"],
        Role::Shangshuling => &["task", "rprt"],
        Role::LiBuShangshu => &["ddtl", "pdsg"],
        Role::BingbuShangshu => &["ctrt", "rprt"],
        Role::GongbuShangshu => &["rprt"],
        Role::XingbuShangshu => &["rprt"],
        Role::LiBuRShangshu => &["rprt"],
    })
}

/// ── create_document ────────────────────────────────────────────────
pub async fn tool_create_document(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let doc_type = args["type"].as_str().unwrap_or("").to_string();
    if doc_type.is_empty() {
        return ToolOutput::error(
            "create_document",
            "",
            "empty_type",
            "Document type cannot be empty",
        );
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
                "Invalid document type: {}, supported types: {}",
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

    let ref_nums = parse_refs(&refs);
    for num in &ref_nums {
        if resolve_ref_doc_id(working_dir, *num).await.is_none() {
            return ToolOutput::error(
                "create_document",
                "",
                "dangling_ref",
                &format!("引用 {num} 不存在，请确认 refs 指向有效文档"),
            );
        }
    }

    if let Some(allowed) = allowed_doc_types(dept) {
        if !allowed.contains(&doc_type.as_str()) {
            use crate::models::role::Role;
            let hint = if Role::from_name(dept) == Some(Role::Neige) && doc_type == "plan" {
                "内阁应通过 submit_pipeline_plan 提交可执行流程，而非创建 plan 文档；plan 由中书令创建。"
            } else {
                "该文档类型不属于本部门职责，请改由对应部门创建，或选择正确的类型。"
            };
            return ToolOutput::error(
                "create_document",
                "",
                "forbidden_type",
                &format!(
                    "部门「{}」无权创建 {} 类型文档。{}",
                    dept_to_author(dept),
                    doc_type,
                    hint
                ),
            );
        }
    }

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
        "revw" => "in_review".to_string(),
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
        approved_hash: String::new(),
    };
    let content = build_doc(&meta, "");

    let full = match resolve_scoped_path(working_dir, &rel_path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("create_document", &doc_id, "path_error", &e),
    };
    if let Some(parent) = full.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::write(&full, &content).await {
        Ok(_) => {
            if status == "in_review" {
                let _ = add_pending_approval(working_dir, &doc_id).await;
            }
            let detail = format!("type={}, refs={}", doc_type, meta.refs);
            crate::audit::append(working_dir, "create_document", dept, &doc_id, &detail).await;
            ToolOutput::success(
                "create_document",
                &doc_id,
                &format!(
                    "Document {} created successfully\n【Important】Use this ID for subsequent operations (append/modify/set-status/route): {}",
                    doc_id, doc_id
                ),
            )
        }
        Err(e) => ToolOutput::error("create_document", &doc_id, "write_error", &e.to_string()),
    }
}

/// If an approved revw is modified, revert to in_review for re-approval.
async fn revert_approved_if_needed(meta: &mut DocMeta, working_dir: &Path) -> Option<&'static str> {
    if MUST_APPROVE_TYPES.contains(&meta.doc_type.as_str()) && meta.status == "approved" {
        meta.status = "in_review".to_string();
        meta.approved_hash.clear();
        let _ = add_pending_approval(working_dir, &meta.id).await;
        Some("已批准文档内容变更，已自动转入重审")
    } else {
        None
    }
}

/// ── update_document ────────────────────────────────────────────────
pub async fn tool_modify_document(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error(
            "modify_document",
            "",
            "empty_id",
            "Document ID cannot be empty",
        );
    }

    if let Some(old_text) = args["old_text"].as_str() {
        if old_text.len() > 3000 {
            return ToolOutput::error(
                "modify_document",
                id,
                "content_too_long",
                &format!(
                    "old_text too long ({} chars), max 3000. Narrow your match range.",
                    old_text.len()
                ),
            );
        }
    }
    if let Some(new_text) = args["new_text"].as_str() {
        if new_text.len() > 3000 {
            return ToolOutput::error(
                "modify_document",
                id,
                "content_too_long",
                &format!(
                    "new_text too long ({} chars), max 3000. Modify in batches or use append_document.",
                    new_text.len()
                ),
            );
        }
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
                    &format!("Document {} does not exist", id),
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
            &format!("Document {} does not exist", id),
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

    let new_body = if let Some(old_text) = args["old_text"].as_str() {
        let new_text = args["new_text"].as_str().unwrap_or("");
        if old_text.is_empty() {
            return ToolOutput::error(
                "modify_document",
                id,
                "empty_old_text",
                "old_text cannot be empty",
            );
        }
        if !body.contains(old_text) {
            return ToolOutput::error(
                "modify_document",
                id,
                "not_found",
                "Could not find matching text in the document body. Use read_document to confirm the content.",
            );
        }
        body.replacen(old_text, new_text, 1)
    } else {
        body.to_string()
    };

    meta.timestamp = now_iso();
    let revert_msg = revert_approved_if_needed(&mut meta, working_dir).await;
    let new_content = build_doc(&meta, &new_body);

    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => {
            let detail = format!(
                "old_text_len={}, new_text_len={}",
                args["old_text"].as_str().unwrap_or("").len(),
                args["new_text"].as_str().unwrap_or("").len()
            );
            crate::audit::append(working_dir, "modify_document", dept, id, &detail).await;
            crate::audit::save_diff(working_dir, id, "modify_document", body, &new_body).await;
            let msg = revert_msg
                .map(|m| format!("Modified successfully. {m}"))
                .unwrap_or_else(|| "Modified successfully".to_string());
            ToolOutput::success("modify_document", id, &msg)
        }
        Err(e) => ToolOutput::error("modify_document", id, "write_error", &e.to_string()),
    }
}

/// Append content to an existing document's body.
pub async fn tool_append_document(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error(
            "append_document",
            "",
            "empty_id",
            "Document ID cannot be empty",
        );
    }

    let append_parts: Vec<String> = if let Some(parts) = args["contents"].as_array() {
        if parts.is_empty() {
            return ToolOutput::error(
                "append_document",
                id,
                "empty_contents",
                "contents array cannot be empty",
            );
        }
        if parts.len() > 5 {
            return ToolOutput::error(
                "append_document",
                id,
                "too_many_contents",
                "contents max 5 items, call in batches",
            );
        }
        for (i, part) in parts.iter().enumerate() {
            let text = part.as_str().unwrap_or("");
            if text.is_empty() {
                return ToolOutput::error(
                    "append_document",
                    id,
                    "empty_content_item",
                    &format!("contents[{}] cannot be empty", i),
                );
            }
            if text.len() > 6000 {
                return ToolOutput::error(
                    "append_document",
                    id,
                    "content_too_long",
                    &format!(
                        "contents[{}] too long ({} chars), max 6000 chars.",
                        i,
                        text.len()
                    ),
                );
            }
        }
        parts
            .iter()
            .filter_map(|p| p.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        let single = args["content"].as_str().unwrap_or("").to_string();
        if single.is_empty() {
            return ToolOutput::error(
                "append_document",
                id,
                "empty_content",
                "Please pass content (single) or contents (batch) parameter",
            );
        }
        if single.len() > 6000 {
            return ToolOutput::error(
                "append_document",
                id,
                "content_too_long",
                &format!(
                    "Content too long ({} chars), max 6000 chars. Append in batches or use contents array.",
                    single.len()
                ),
            );
        }
        vec![single]
    };

    let type_prefix = id.split('_').next().unwrap_or("");
    let full = if type_prefix == "rprt" {
        match find_rprt_path(working_dir, id).await {
            Some(p) => p,
            None => {
                return ToolOutput::error(
                    "append_document",
                    id,
                    "not_found",
                    &format!("Document {} does not exist", id),
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
            &format!("Document {} does not exist", id),
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
        append_parts.join("\n")
    } else {
        let mut b = body.to_string();
        for part in &append_parts {
            b.push_str("\n");
            b.push_str(part);
        }
        b
    };

    meta.timestamp = now_iso();
    let revert_msg = revert_approved_if_needed(&mut meta, working_dir).await;
    let new_content = build_doc(&meta, &new_body);

    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => {
            let total: usize = append_parts.iter().map(|s| s.len()).sum();
            let detail = format!("append_parts={}, total_chars={}", append_parts.len(), total);
            crate::audit::append(working_dir, "append_document", dept, id, &detail).await;
            crate::audit::save_diff(working_dir, id, "append_document", body, &new_body).await;
            let msg = revert_msg
                .map(|m| format!("Appended successfully. {m}"))
                .unwrap_or_else(|| "Appended successfully".to_string());
            ToolOutput::success("append_document", id, &msg)
        }
        Err(e) => ToolOutput::error("append_document", id, "write_error", &e.to_string()),
    }
}

/// ── Tool definitions ──────────────────────────────────────────────
pub fn create_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_document".into(),
            description: "Create a new document. The system auto-assigns an ID and generates a YAML header. Returns the document ID.".into(),
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
                        "description": "Referenced document IDs (integers, without type prefix). Pass [] for no references"
                    }
                },
                "required": ["type", "refs"]
            }),
        },
    }
}

pub fn create_task_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_document".into(),
            description: "Create a task document (type fixed to 'task'). 内阁出流程请用 submit_pipeline_plan；本工具仅用于创建 task 文档作为 expand_requirements 的前置。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "enum": ["task"], "description": "固定为 task" },
                    "refs": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "Referenced document IDs (integers, no prefix). Pass [] for none"
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
            description: "Replace text in a document body (find+replace). ≤3000 chars.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID, e.g. dsgn_003"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Text to replace (≤3000 chars)",
                        "maxLength": 3000
                    },
                    "new_text": {
                        "type": "string",
                        "description": "Replacement text (≤3000 chars)",
                        "maxLength": 3000
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
            description: "Append content to an existing document's body. content ≤6000 chars. For multi-part content, call multiple times. Do NOT use the contents array — array JSON is prone to truncation with long content.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID, e.g. dsgn_003"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to append (≤6000 chars). For multi-part content, call multiple times — do NOT use the contents array.",
                        "maxLength": 6000
                    },
                    "contents": {
                        "type": "array",
                        "description": "[Not recommended] Array JSON is prone to truncation with long content. Use single content parameter with multiple calls instead.",
                        "items": {
                            "type": "string",
                            "maxLength": 6000
                        },
                        "maxItems": 5
                    }
                },
                "anyOf": [
                    {"required": ["id", "content"]},
                    {"required": ["id", "contents"]}
                ]
            }),
        },
    }
}

/// Build a helpful hint when read_document fails (list nearby valid IDs).
async fn doc_not_found_hint(working_dir: &Path, raw_id: &str, normalized_id: &str) -> String {
    let mut parts = vec![
        "read_document expects a document ID like dsgn_3 or plan_1 — not a filename path or context JSON."
            .to_string(),
    ];
    if raw_id != normalized_id {
        parts.push(format!(
            "Normalized \"{raw_id}\" → \"{normalized_id}\" but still not found."
        ));
    }
    if raw_id.ends_with(".md") || raw_id.contains('/') || raw_id.contains('\\') {
        parts.push("Do not pass file paths or .md extensions; use the ID only.".to_string());
    }
    let mut samples = list_doc_ids_in_subdir(working_dir, "designs", 8).await;
    if samples.is_empty() {
        samples = list_doc_ids_in_subdir(working_dir, "requirements", 5).await;
    }
    if !samples.is_empty() {
        parts.push(format!(
            "Available IDs in .shuji/designs: {}",
            samples.join(", ")
        ));
    } else {
        parts.push(
            "No documents in .shuji/designs yet — list_dir .shuji/designs first.".to_string(),
        );
    }
    parts.join(" ")
}

/// ── read_document ─────────────────────────────────────────────────
pub async fn tool_read_document(working_dir: &Path, args: &serde_json::Value) -> String {
    let raw_id = args["id"].as_str().unwrap_or("");
    if raw_id.is_empty() {
        return ToolOutput::error(
            "read_document",
            "",
            "empty_id",
            "Document ID cannot be empty. Use list_dir on .shuji/designs to find IDs like dsgn_3 (no .md suffix).",
        );
    }

    let id = normalize_doc_id(working_dir, raw_id).await;
    let full = match resolve_doc_path(working_dir, &id).await {
        Ok(p) => p,
        Err(e) => {
            let hint = doc_not_found_hint(working_dir, raw_id, &id).await;
            return ToolOutput::error(
                "read_document",
                raw_id,
                "not_found",
                &format!("{e}. {hint}"),
            );
        }
    };
    if !full.exists() {
        let hint = doc_not_found_hint(working_dir, raw_id, &id).await;
        return ToolOutput::error(
            "read_document",
            raw_id,
            "not_found",
            &format!("Document {id} does not exist. {hint}"),
        );
    }

    if let Some(cached) = crate::tool::cache_lookup(working_dir, &full) {
        return cached;
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => {
            if let Ok(meta) = tokio::fs::metadata(&full).await {
                if let Ok(mtime) = meta.modified() {
                    crate::tool::cache_insert(working_dir, full.clone(), mtime, c.clone());
                }
            }
            c
        }
        Err(e) => return ToolOutput::error("read_document", &id, "read_error", &e.to_string()),
    };

    let (meta, body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error("read_document", &id, "parse_error", &e),
    };

    let target_section = args["section"].as_str().filter(|s| !s.is_empty());
    let extracted = if let Some(section_name) = target_section {
        match extract_section(body, section_name) {
            Ok(content) => content,
            Err(msg) => {
                return ToolOutput::error("read_document", &id, "section_not_found", &msg);
            }
        }
    } else {
        body.to_string()
    };

    let max_chars = args["max_chars"].as_u64().unwrap_or(4000) as usize;
    let display_body = if max_chars > 0 && extracted.len() > max_chars {
        let cutoff = extracted.floor_char_boundary(max_chars);
        format!(
            "{}...\n\n[Truncated: showing first {} of {} chars]",
            &extracted[..cutoff],
            cutoff,
            extracted.len()
        )
    } else {
        extracted
    };

    let rel_path = full
        .strip_prefix(working_dir)
        .unwrap_or(&full)
        .to_string_lossy();
    let meta_line = format!(
        "{} | type: {} | author: {} | time: {} | status: {} | refs: {}",
        meta.id,
        meta.doc_type,
        meta.author,
        meta.timestamp,
        if meta.status.is_empty() {
            "-"
        } else {
            &meta.status
        },
        meta.refs
    );
    let result = if let Some(ref section) = target_section {
        format!(
            "{}\n--- Section [{}] ---\n{}",
            meta_line, section, display_body
        )
    } else {
        format!("{}\n--- Body ---\n{}", meta_line, display_body)
    };

    ToolOutput::read_file("read_document", &rel_path, &result)
}

/// Extract a section (## or ### heading) from markdown body text.
fn extract_section(body: &str, section_name: &str) -> Result<String, String> {
    let heading = format!("## {}", section_name);
    let heading3 = format!("### {}", section_name);
    let lines: Vec<&str> = body.lines().collect();
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == heading
            || trimmed.starts_with(&heading)
            || trimmed == heading3
            || trimmed.starts_with(&heading3)
        {
            start = Some(i);
        } else if start.is_some()
            && end.is_none()
            && (line.starts_with("## ") || line.starts_with("### "))
        {
            end = Some(i);
            break;
        }
    }

    if let Some(s) = start {
        let e = end.unwrap_or(lines.len());
        Ok(lines[s..e].join("\n"))
    } else {
        let available: Vec<&str> = lines
            .iter()
            .filter(|l| l.starts_with("## ") || l.starts_with("### "))
            .map(|l| l.trim())
            .collect();
        Err(format!(
            "Section \"{}\" not found in the document.\nAvailable sections ({} total):\n{}",
            section_name,
            available.len(),
            available.join("\n")
        ))
    }
}

pub fn read_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "read_document".into(),
            description: "Preferred document reading method. Reads by document ID, returns YAML metadata + body. Default truncation at 4000 chars (pass max_chars=0 to disable). Optional ## section extraction. Replaces the two-step find_document -> read_file approach.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID without .md suffix, e.g. dsgn_3, plan_1, revw_2. Use list_dir on .shuji/designs to discover IDs from filenames."
                    },
                    "section": {
                        "type": "string",
                        "description": "Optional: extract a specific ## section by title (e.g. 'Signature', 'Data Operations'). Omit to return full body."
                    },
                    "max_chars": {
                        "type": "integer",
                        "description": "Optional: max characters to return; truncates beyond this (prevents oversized documents from blowing up context)"
                    }
                },
                "required": ["id"]
            }),
        },
    }
}

/// ── find_document ─────────────────────────────────────────────────
pub async fn tool_find_document(working_dir: &Path, args: &serde_json::Value) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error(
            "find_document",
            "",
            "empty_id",
            "Document ID cannot be empty",
        );
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
                &format!("Document {} does not exist", id),
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
                &format!("Document {} does not exist", id),
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
            description: "Deprecated: Do not use unless read_document fails. read_document combines find + read + section extraction in one call.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID, e.g. rprt_32, dsgn_003, task_5"
                    }
                },
                "required": ["id"]
            }),
        },
    }
}
