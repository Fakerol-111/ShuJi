use std::path::Path;

use sha2::{Digest, Sha256};

use crate::tool::ToolOutput;

use super::parse::{
    append_note_entry, build_doc, now_iso, parse_doc, parse_refs, resolve_doc_path,
    resolve_ref_doc_id, MUST_APPROVE_TYPES,
};

fn hash_body(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Scan designs/ and reviews/ for plan/revw documents with status=in_review.
pub async fn scan_in_review_docs(working_dir: &Path) -> Vec<String> {
    let mut pending = Vec::new();
    for dir_name in ["designs", "reviews"] {
        let dir = working_dir.join(".shuji").join(dir_name);
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                if let Ok((meta, _)) = parse_doc(&content) {
                    if MUST_APPROVE_TYPES.contains(&meta.doc_type.as_str())
                        && meta.status == "in_review"
                    {
                        pending.push(meta.id);
                    }
                }
            }
        }
    }
    pending.sort();
    pending
}

/// Rewrite pending_approvals.json cache from scan results (derived view).
pub async fn sync_pending_approvals_cache(working_dir: &Path) -> Result<Vec<String>, String> {
    let list = scan_in_review_docs(working_dir).await;
    let path = working_dir.join(".shuji/pending_approvals.json");
    let shuji_dir = path.parent().ok_or("Path error")?;
    let _ = tokio::fs::create_dir_all(shuji_dir).await;
    let json = serde_json::to_string(&list)
        .map_err(|e| format!("Serializing pending_approvals failed: {e}"))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| e.to_string())?;
    Ok(list)
}

/// Add a document ID to the pending approvals list.
pub async fn add_pending_approval(working_dir: &Path, doc_id: &str) -> Result<(), String> {
    let path = working_dir.join(".shuji/pending_approvals.json");
    let shuji_dir = path.parent().ok_or("Path error")?;
    let _ = tokio::fs::create_dir_all(shuji_dir).await;
    let mut list: Vec<String> = tokio::fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if !list.contains(&doc_id.to_string()) {
        list.push(doc_id.to_string());
    }
    let json = serde_json::to_string(&list)
        .map_err(|e| format!("Serializing pending_approvals failed: {e}"))?;
    tokio::fs::write(&path, json)
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
    let json = serde_json::to_string(&list)
        .map_err(|e| format!("Serializing pending_approvals failed: {e}"))?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| e.to_string())
}

/// Get the first pending approval doc ID, if any (scans by status).
pub async fn get_first_pending_approval(working_dir: &Path) -> Option<String> {
    scan_in_review_docs(working_dir).await.into_iter().next()
}

/// ── set_document_status ──────────────────────────────────────────
pub async fn tool_set_document_status(working_dir: &Path, args: &serde_json::Value) -> String {
    let id = args["id"].as_str().unwrap_or("");
    let new_status = args["status"].as_str().unwrap_or("");
    let auto = args["auto"].as_bool().unwrap_or(false);
    let retries = args["retries"].as_u64().unwrap_or(3);
    if id.is_empty() || new_status.is_empty() {
        return ToolOutput::error(
            "set_document_status",
            "",
            "empty_params",
            "id and status cannot be empty",
        );
    }
    if !matches!(new_status, "approved" | "rejected") {
        return ToolOutput::error(
            "set_document_status",
            id,
            "invalid_status",
            "status must be approved or rejected",
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
            &format!("Document {} does not exist", id),
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

    if !MUST_APPROVE_TYPES.contains(&meta.doc_type.as_str()) {
        return ToolOutput::error(
            "set_document_status",
            id,
            "wrong_type",
            "Only plan and revw document types can have approval status set",
        );
    }

    let emperor_note = args["emperor_note"].as_str().unwrap_or("");
    let ts = now_iso();

    if auto {
        let note_line = format!(
            "[auto_approved @{}] 超时自动放行（内阁连续 {} 轮未请示）",
            ts, retries
        );
        meta.notes = append_note_entry(&meta.notes, &note_line);
    } else if !emperor_note.is_empty() {
        let note_line = format!("朱批[{new_status} @{ts}]: {emperor_note}");
        meta.notes = append_note_entry(&meta.notes, &note_line);
    }

    meta.status = new_status.to_string();
    meta.timestamp = ts;

    if new_status == "approved" {
        meta.approved_hash = hash_body(body);
    } else {
        meta.approved_hash.clear();
    }

    let new_content = build_doc(&meta, body);
    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => {
            let _ = remove_pending_approval(working_dir, id).await;
            let role = if auto { "system" } else { "emperor" };
            let detail = if auto {
                format!(
                    "status=approved; auto_approved=true; retries={}; hash={}",
                    retries, meta.approved_hash
                )
            } else if new_status == "approved" {
                format!(
                    "status=approved; hash={}; note={}",
                    meta.approved_hash, emperor_note
                )
            } else {
                format!("status={new_status}; note={emperor_note}")
            };
            crate::audit::append(working_dir, "set_document_status", role, id, &detail).await;
            ToolOutput::success(
                "set_document_status",
                id,
                &format!("Document {} status set to {}", id, new_status),
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
            description: "Set document approval status (approved/rejected). Only applies to plan and revw document types. After status change, downstream departments may proceed.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID, e.g. plan_5 or revw_3"
                    },
                    "status": {
                        "type": "string",
                        "enum": ["approved", "rejected"],
                        "description": "approved or rejected"
                    },
                    "emperor_note": {
                        "type": "string",
                        "description": "Emperor's note (optional)"
                    }
                },
                "required": ["id", "status"]
            }),
        },
    }
}

async fn check_must_approve_status(
    working_dir: &Path,
    doc_id: &str,
    subject_doc_id: &str,
) -> Result<(), String> {
    let full = match resolve_doc_path(working_dir, doc_id).await {
        Ok(p) => p,
        Err(_) => {
            return Err(format!(
                "断链：文档 {} 引用的 {} 不存在",
                subject_doc_id, doc_id
            ))
        }
    };
    if !full.exists() {
        return Err(format!(
            "断链：文档 {} 引用的 {} 不存在",
            subject_doc_id, doc_id
        ));
    }
    let content = tokio::fs::read_to_string(&full)
        .await
        .map_err(|e| e.to_string())?;
    let (ref_meta, _) = parse_doc(&content)?;

    if ref_meta.status == "in_review" {
        return Err(format!(
            "Document {} references {}, which is still awaiting emperor approval (status: in_review). Please handle the approval first.",
            subject_doc_id, doc_id
        ));
    }
    if ref_meta.status == "rejected" {
        return Err(format!(
            "Document {} references {}, which has been rejected (status: rejected). Please address the rejection feedback first.",
            subject_doc_id, doc_id
        ));
    }
    Ok(())
}

/// Check whether any must-approve document referenced by the given doc
/// is still `in_review`. Returns Err if any referenced plan/revw is pending.
pub async fn check_doc_refs_approved_for_route(
    working_dir: &Path,
    subject_doc_id: &str,
) -> Result<(), String> {
    let full = match resolve_doc_path(working_dir, subject_doc_id).await {
        Ok(p) => p,
        Err(_) => return Ok(()),
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

    // Subject itself must be approved if plan/revw
    if MUST_APPROVE_TYPES.contains(&meta.doc_type.as_str()) {
        if meta.status == "in_review" {
            return Err(format!(
                "Document {} is still awaiting emperor approval (status: in_review). Please handle the approval first.",
                subject_doc_id
            ));
        }
        if meta.status == "rejected" {
            return Err(format!(
                "Document {} has been rejected (status: rejected). Please address the rejection feedback first.",
                subject_doc_id
            ));
        }
    }

    let ref_nums = parse_refs(&meta.refs);
    if ref_nums.is_empty() {
        return Ok(());
    }

    for num in ref_nums {
        let ref_id = match resolve_ref_doc_id(working_dir, num).await {
            Some(id) => id,
            None => {
                return Err(format!(
                    "断链：文档 {} 引用的 {} 不存在",
                    subject_doc_id, num
                ))
            }
        };

        if MUST_APPROVE_TYPES.contains(&ref_id.split('_').next().unwrap_or("")) {
            check_must_approve_status(working_dir, &ref_id, subject_doc_id).await?;
        }

        // One-level transitive: check plan/revw refs of the resolved document
        if let Ok(ref_path) = resolve_doc_path(working_dir, &ref_id).await {
            if let Ok(ref_content) = tokio::fs::read_to_string(&ref_path).await {
                if let Ok((ref_meta, _)) = parse_doc(&ref_content) {
                    for indirect_num in parse_refs(&ref_meta.refs) {
                        if let Some(indirect_id) =
                            resolve_ref_doc_id(working_dir, indirect_num).await
                        {
                            if MUST_APPROVE_TYPES
                                .contains(&indirect_id.split('_').next().unwrap_or(""))
                            {
                                check_must_approve_status(
                                    working_dir,
                                    &indirect_id,
                                    subject_doc_id,
                                )
                                .await?;
                            }
                        } else {
                            return Err(format!(
                                "断链：文档 {} 间接引用的 {} 不存在",
                                subject_doc_id, indirect_num
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
