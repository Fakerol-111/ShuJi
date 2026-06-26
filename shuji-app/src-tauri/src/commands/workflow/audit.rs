use std::path::Path;

use tauri::State;

use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;

/// Verify the SHA-256 hash chain integrity of the audit log.
#[tauri::command]
pub async fn verify_audit_trail(
    state: State<'_, AppState>,
) -> Result<crate::audit::VerificationReport, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    crate::audit::verify_audit_trail(Path::new(&working_dir)).await
}

/// Get the document lineage tree for a given doc ID.
#[tauri::command]
pub async fn get_document_lineage(
    state: State<'_, AppState>,
    doc_id: String,
) -> Result<Option<crate::audit::LineageNode>, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    Ok(crate::audit::build_lineage(Path::new(&working_dir), &doc_id).await)
}

/// Get the aggregated audit timeline.
#[tauri::command]
pub async fn get_audit_timeline(
    state: State<'_, AppState>,
) -> Result<crate::audit::TimelineData, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    Ok(crate::audit::build_timeline(Path::new(&working_dir)).await)
}

/// Generate a delivery report for the current project.
#[tauri::command]
pub async fn generate_delivery_report(state: State<'_, AppState>) -> Result<String, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    Ok(crate::audit::generate_report(Path::new(&working_dir)).await)
}

/// A diff file reference for the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DocDiffFile {
    pub filename: String,
    pub event: String,
    pub ts: String,
}

/// List available diff files for a document.
#[tauri::command]
pub async fn get_document_diffs(
    state: State<'_, AppState>,
    doc_id: String,
) -> Result<Vec<DocDiffFile>, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    let diff_dir = Path::new(&working_dir)
        .join(".shuji")
        .join("audit")
        .join("diffs");
    let mut diffs = Vec::new();

    if let Ok(mut rd) = tokio::fs::read_dir(&diff_dir).await {
        let mut entries = Vec::new();
        while let Ok(Some(entry)) = rd.next_entry().await {
            entries.push(entry);
        }
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&format!("{}_", doc_id)) {
                let stripped = name.strip_suffix(".patch").unwrap_or(&name);
                let parts: Vec<&str> = stripped.splitn(3, '_').collect();
                let event = if parts.len() > 1 {
                    parts[1].to_string()
                } else {
                    String::new()
                };
                let ts = if parts.len() > 2 {
                    parts[2].to_string()
                } else {
                    String::new()
                };
                diffs.push(DocDiffFile {
                    filename: name,
                    event,
                    ts,
                });
            }
        }
    }
    diffs.sort_by(|a, b| b.ts.cmp(&a.ts));
    Ok(diffs)
}

/// Read the content of a specific diff file.
#[tauri::command]
pub async fn read_document_diff(
    state: State<'_, AppState>,
    filename: String,
) -> Result<String, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    let path = Path::new(&working_dir)
        .join(".shuji")
        .join("audit")
        .join("diffs")
        .join(&filename);
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取 diff 失败: {}", e))
}

/// Set a document's approval status (approved only).
#[tauri::command]
pub async fn set_document_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
    emperor_note: Option<String>,
) -> Result<String, String> {
    if status != "approved" {
        return Err("status must be approved".to_string());
    }
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };

    let mut args = serde_json::json!({
        "id": id,
        "status": status,
    });
    if let Some(note) = emperor_note {
        args["emperor_note"] = serde_json::Value::String(note);
    }

    let result =
        crate::tool::documents::tool_set_document_status(Path::new(&working_dir), &args).await;

    let v: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| "解析结果失败".to_string())?;
    if v["ok"].as_bool().unwrap_or(false) {
        crate::audit::sync_ref_index(Path::new(&working_dir), &id).await;
        Ok(v["message"].as_str().unwrap_or("ok").to_string())
    } else {
        Err(v["message"].as_str().unwrap_or("未知错误").to_string())
    }
}

/// Trace a document through its history.
#[tauri::command]
pub async fn trace_document(
    state: State<'_, AppState>,
    doc_id: String,
) -> Result<crate::audit::TraceResult, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    Ok(crate::audit::trace_document(Path::new(&working_dir), &doc_id).await)
}

/// Query documents with combined filters.
#[tauri::command]
pub async fn query_documents(
    state: State<'_, AppState>,
    filter: crate::audit::DocQuery,
) -> Result<Vec<crate::audit::DocSummary>, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    Ok(crate::audit::query_documents(Path::new(&working_dir), &filter).await)
}
