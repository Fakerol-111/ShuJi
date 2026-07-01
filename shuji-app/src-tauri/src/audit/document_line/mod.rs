//! Document line: end-to-end audit view linking docs, pipeline steps, approvals,
//! validation, diffs, and semantic checkpoints for a single task run.

mod context;
mod events;
mod scan;
pub(crate) mod types;

pub use types::{DocumentLineRun, EvidenceRef, ImpactAnalysis, ImpactNode, LineEdge, LineNode};

use context::LineContext;

/// Build the document line for a pipeline run (or the active/legacy run when `run_id` is None).
pub async fn build_document_line(
    working_dir: &std::path::Path,
    run_id: Option<&str>,
) -> Option<DocumentLineRun> {
    let ctx = LineContext::load(working_dir).await;
    if ctx.is_empty() {
        return None;
    }
    let rid = run_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| ctx.primary_run_id());
    Some(ctx.build_run(&rid, None))
}

/// Build the document line containing `doc_id`, highlighting that node.
pub async fn build_document_line_for_doc(
    working_dir: &std::path::Path,
    doc_id: &str,
) -> Option<DocumentLineRun> {
    let ctx = LineContext::load(working_dir).await;
    if ctx.is_empty() {
        return None;
    }
    let run_id = ctx.find_run_for_doc(doc_id);
    Some(ctx.build_run(&run_id, Some(doc_id)))
}

/// List known run IDs (pipeline plan_id or legacy bucket).
pub async fn list_document_line_runs(working_dir: &std::path::Path) -> Vec<String> {
    let ctx = LineContext::load(working_dir).await;
    ctx.run_ids()
}

/// Analyze downstream impact when modifying `doc_id`.
pub async fn analyze_impact(working_dir: &std::path::Path, doc_id: &str) -> ImpactAnalysis {
    let ctx = LineContext::load(working_dir).await;
    ctx.analyze_impact(doc_id)
}

pub use events::active_run_id;

pub async fn append_line_event(
    working_dir: &std::path::Path,
    run_id: &str,
    event: &str,
    node_id: &str,
    detail: serde_json::Value,
) {
    types::LineEventRecord::append(working_dir, run_id, event, node_id, detail).await;
}
