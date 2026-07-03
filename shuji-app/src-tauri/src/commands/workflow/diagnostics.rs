//! Diagnostic bundle export command.
//!
//! Packages a project's runtime state, metrics, logs, and config into a
//! single JSON file for debugging purposes. API keys are redacted.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::commands::project::AppState;
use crate::metrics::load_latest;

/// Diagnostic bundle containing project state for debugging.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticBundle {
    pub exported_at: String,
    pub runtime_summary: Option<serde_json::Value>,
    pub run_metrics: Option<serde_json::Value>,
    pub recent_audit_log: Vec<String>,
    pub recent_activity_log: Vec<String>,
    pub validation_report: Option<serde_json::Value>,
    pub pending_approvals: Vec<String>,
    pub checkpoint_summary: serde_json::Value,
    pub config_summary: serde_json::Value,
    pub workflow_graph: Option<serde_json::Value>,
}

/// Redact known API key patterns from a JSON value (mutates in-place).
fn redact_api_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (_key, val) in map.iter_mut() {
                if let serde_json::Value::String(s) = val {
                    let lower = s.to_lowercase();
                    if lower.contains("sk-")
                        || lower.contains("api_key")
                        || lower.starts_with("sk-")
                    {
                        let keep = if s.len() > 8 { &s[..8] } else { "" };
                        *val = serde_json::Value::String(format!("{}...REDACTED", keep));
                    }
                }
                redact_api_keys(val);
            }
        }
        serde_json::Value::Array(arr) => {
            for val in arr.iter_mut() {
                redact_api_keys(val);
            }
        }
        _ => {}
    }
}

/// Load a file, returning up to `max_lines` lines as a Vec<String>.
/// Missing files return an empty vec (tolerated).
async fn tail_file(path: &Path, max_lines: usize) -> Vec<String> {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].iter().map(|l| l.to_string()).collect()
}

/// Load a JSON file as a serde_json::Value, returning None on failure.
async fn load_json(path: &Path) -> Option<serde_json::Value> {
    let content = tokio::fs::read_to_string(path).await.ok()?;
    serde_json::from_str(&content).ok()
}

/// Map of pending approvals from pipeline runtime state.
async fn find_pending_approvals(project_dir: &Path) -> Vec<String> {
    let runtime_path = project_dir
        .join(".shuji")
        .join("pipeline")
        .join("runtime.json");
    if let Some(val) = load_json(&runtime_path).await {
        if let Some(status_map) = val.get("step_status").and_then(|v| v.as_object()) {
            if let Some(steps) = val
                .get("plan")
                .and_then(|p| p.get("steps"))
                .and_then(|s| s.as_array())
            {
                let mut pending = Vec::new();
                for step in steps {
                    let step_id = step["step_id"].as_str().unwrap_or("");
                    let action = step["action"].as_str().unwrap_or("");
                    if action == "approval_gate" {
                        if let Some(raw_status) = status_map.get(step_id) {
                            let status = raw_status.as_str().unwrap_or("");
                            if status == "in_progress" || status == "pending" {
                                pending.push(step_id.to_string());
                            }
                        }
                    }
                }
                return pending;
            }
        }
    }
    vec![]
}

/// Export diagnostics for the current project.
///
/// Produces a JSON bundle with redacted API keys and missing-file tolerance.
#[tauri::command]
pub async fn export_diagnostics(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let project_dir = {
        let guard = state.current_project.lock().await;
        let p = guard
            .as_ref()
            .ok_or_else(|| "No project selected".to_string())?;
        PathBuf::from(&p.working_dir)
    };

    let shuji = project_dir.join(".shuji");

    let pipeline_path = shuji.join("pipeline").join("runtime.json");
    let audit_path = shuji.join("audit.jsonl");
    let activity_path = shuji.join("logs").join("activity.log");
    let validate_path = shuji.join("validate").join("report.json");
    let checkpoint_path = shuji.join("checkpoints").join("index.json");
    let graph_path = shuji.join("workflow_graph.json");

    // Run metrics from dedicated API
    let run_metrics = load_latest(&project_dir)
        .await
        .map(|m| serde_json::to_value(m).unwrap_or_default());

    // Load everything else concurrently
    let (
        runtime_summary,
        audit_log,
        activity_log,
        validation_report,
        pending_approvals,
        checkpoint_summary,
        workflow_graph,
    ) = tokio::join!(
        load_json(&pipeline_path),
        tail_file(&audit_path, 50),
        tail_file(&activity_path, 50),
        load_json(&validate_path),
        find_pending_approvals(&project_dir),
        load_json(&checkpoint_path),
        load_json(&graph_path),
    );

    let checkpoint_summary =
        checkpoint_summary.unwrap_or(serde_json::json!({"error": "no checkpoint index"}));

    // Config summary (without secrets)
    let config_summary = serde_json::json!({
        "has_config_toml": project_dir.join("config.toml").exists(),
        "has_local_config": project_dir.join("config.local.toml").exists(),
    });

    let bundle = DiagnosticBundle {
        exported_at: chrono::Utc::now().to_rfc3339(),
        runtime_summary,
        run_metrics,
        recent_audit_log: audit_log,
        recent_activity_log: activity_log,
        validation_report,
        pending_approvals,
        checkpoint_summary,
        config_summary,
        workflow_graph,
    };

    // Serialize once, redact, return
    let mut json = serde_json::to_value(&bundle).map_err(|e| e.to_string())?;
    redact_api_keys(&mut json);

    serde_json::to_string_pretty(&json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_api_keys_in_object() {
        let mut val = serde_json::json!({
            "api_key": "sk-abc123secret",
            "url": "https://example.com",
            "nested": {
                "key": "sk-xyz789"
            }
        });
        redact_api_keys(&mut val);
        let api_key = val["api_key"].as_str().unwrap();
        assert!(api_key.contains("REDACTED"));
        assert!(!api_key.contains("secret"));
        assert_eq!(val["url"].as_str().unwrap(), "https://example.com");
        let nested_key = val["nested"]["key"].as_str().unwrap();
        assert!(nested_key.contains("REDACTED"));
    }

    #[test]
    fn test_redact_leaves_innocent_strings() {
        let mut val = serde_json::json!({
            "name": "hello world",
            "count": 42
        });
        redact_api_keys(&mut val);
        assert_eq!(val["name"].as_str().unwrap(), "hello world");
    }

    #[tokio::test]
    async fn test_tail_file_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let lines = tail_file(&tmp.path().join("nonexistent.json"), 10).await;
        assert!(lines.is_empty());
    }

    #[tokio::test]
    async fn test_tail_file_returns_lines() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.log");
        tokio::fs::write(&path, "line1\nline2\nline3\n")
            .await
            .unwrap();
        let lines = tail_file(&path, 10).await;
        assert_eq!(lines.len(), 3);
    }

    #[tokio::test]
    async fn test_tail_file_respects_max_lines() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.log");
        let content = (0..100)
            .map(|i| format!("line{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        tokio::fs::write(&path, &content).await.unwrap();
        let lines = tail_file(&path, 10).await;
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "line90");
    }

    #[tokio::test]
    async fn test_load_json_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(load_json(&tmp.path().join("nope.json")).await.is_none());
    }

    #[tokio::test]
    async fn test_find_pending_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pending = find_pending_approvals(tmp.path()).await;
        assert!(pending.is_empty());
    }
}
