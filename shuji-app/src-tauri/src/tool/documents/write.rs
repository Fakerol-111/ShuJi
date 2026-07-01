//! Document create, modify, and append operations.

use std::path::Path;

use crate::tool::{resolve_scoped_path, ToolOutput};

use super::approval::add_pending_approval;
use super::parse::{
    build_doc, dept_to_author, find_rprt_path, next_id, now_iso, parse_doc, parse_refs,
    resolve_ref_doc_id, rprt_rel_path, type_to_dir, DocMeta, MUST_APPROVE_TYPES,
};
use super::policy::allowed_doc_types;

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
            let run_id = crate::audit::active_run_id(working_dir).await;
            crate::audit::append_line_event(
                working_dir,
                &run_id,
                "create_document",
                &doc_id,
                serde_json::json!({"type": doc_type, "author": dept}),
            )
            .await;
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
            let run_id = crate::audit::active_run_id(working_dir).await;
            crate::audit::append_line_event(
                working_dir,
                &run_id,
                "modify_document",
                id,
                serde_json::json!({"author": dept}),
            )
            .await;
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
            b.push('\n');
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
            let run_id = crate::audit::active_run_id(working_dir).await;
            crate::audit::append_line_event(
                working_dir,
                &run_id,
                "append_document",
                id,
                serde_json::json!({"author": dept}),
            )
            .await;
            let msg = revert_msg
                .map(|m| format!("Appended successfully. {m}"))
                .unwrap_or_else(|| "Appended successfully".to_string());
            ToolOutput::success("append_document", id, &msg)
        }
        Err(e) => ToolOutput::error("append_document", id, "write_error", &e.to_string()),
    }
}
