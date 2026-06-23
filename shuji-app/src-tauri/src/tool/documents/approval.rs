use std::path::Path;

use crate::tool::ToolOutput;

use super::parse::{build_doc, now_iso, parse_doc, parse_refs, resolve_doc_path};

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

pub async fn tool_set_document_status(working_dir: &Path, args: &serde_json::Value) -> String {
    let id = args["id"].as_str().unwrap_or("");
    let new_status = args["status"].as_str().unwrap_or("");
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

    meta.status = new_status.to_string();
    meta.notes = args["emperor_note"].as_str().unwrap_or("").to_string();
    meta.timestamp = now_iso();

    let new_content = build_doc(&meta, body);
    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => {
            let _ = remove_pending_approval(working_dir, id).await;
            let detail = format!("status={}", new_status);
            crate::audit::append(working_dir, "set_document_status", "emperor", id, &detail).await;

            // 设计质量反馈：记录审核结论到审计跟踪
            crate::audit::log_review_conclusion(working_dir, id, "emperor", &new_status, 0, 0)
                .await;
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
                            "Document {} references {}, which is still awaiting emperor approval (status: in_review). Please handle the approval first.",
                            subject_doc_id, ref_id
                        ));
                    }
                    if ref_meta.status == "rejected" {
                        return Err(format!(
                            "Document {} references {}, which has been rejected (status: rejected). Please address the rejection feedback first.",
                            subject_doc_id, ref_id
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}
