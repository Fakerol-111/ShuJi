//! Incremental line events — append, load, and active run resolution.

use std::path::Path;

use super::types::LineEventRecord;
use crate::pipeline::PlanRuntime;

impl LineEventRecord {
    pub(crate) async fn append(
        working_dir: &Path,
        run_id: &str,
        event: &str,
        node_id: &str,
        detail: serde_json::Value,
    ) {
        let dir = working_dir.join(".shuji").join("audit").join("doc_lines");
        let _ = tokio::fs::create_dir_all(&dir).await;
        let path = dir.join("events.jsonl");
        let record = LineEventRecord {
            ts: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
            run_id: run_id.to_string(),
            event: event.to_string(),
            node_id: node_id.to_string(),
            detail,
        };
        if let Ok(json) = serde_json::to_string(&record) {
            use tokio::io::AsyncWriteExt;
            if let Ok(mut f) = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await
            {
                let _ = f.write_all(format!("{json}\n").as_bytes()).await;
            }
        }
    }
}

pub(crate) async fn load_line_events(working_dir: &Path) -> Vec<LineEventRecord> {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("doc_lines")
        .join("events.jsonl");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .map(|mut r: LineEventRecord| {
            // M6: map legacy "legacy" run_id to "unassigned"
            if r.run_id == "legacy" {
                r.run_id = super::context::UNASSIGNED_RUN_ID.to_string();
            }
            r
        })
        .collect()
}

/// Resolve the active run_id from pipeline runtime or fallback.
pub async fn active_run_id(working_dir: &Path) -> String {
    if let Some(rt) = PlanRuntime::load_from(working_dir).await {
        return rt.plan.plan_id.clone();
    }
    super::context::UNASSIGNED_RUN_ID.into()
}
