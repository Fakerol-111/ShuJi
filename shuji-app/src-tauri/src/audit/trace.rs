use std::path::Path;

use serde::Serialize;

use crate::tool::documents;

use super::{doc_store, ref_index::RefIndex};

/// Represents a node in the full development chain.
#[derive(Debug, Clone, Serialize)]
pub struct ChainNode {
    pub id: String,
    pub doc_type: String,
    pub author: String,
    pub timestamp: String,
    pub stage: String, // reqs / design / plan / contract / code
    pub content_preview: String,
    /// Relationship direction: "upstream" (this doc references the target),
    /// "downstream" (target references this doc), or "self" (the searched doc).
    pub direction: String,
}

/// Result of tracing a document: the doc itself, what it references, and what references it.
#[derive(Debug, Clone, Serialize)]
pub struct TraceResult {
    /// The document that was searched for.
    pub target: Option<ChainNode>,
    /// Documents that the target references (downstream: target → refs).
    pub downstream: Vec<ChainNode>,
    /// Documents that reference the target (upstream: refs → target).
    pub upstream: Vec<ChainNode>,
}

/// Trace a document in both directions: what it references AND what references it.
pub async fn trace_document(working_dir: &Path, doc_id: &str) -> TraceResult {
    // 1. Get the target document itself
    let target_node = match doc_store::read_doc_by_id(working_dir, doc_id).await {
        Some(content) => documents::parse_doc(&content).ok().map(|(meta, body)| {
            let preview = body.lines().next().unwrap_or("").chars().take(80).collect();
            let stage = stage_for_type(&meta.doc_type);
            ChainNode {
                id: doc_id.to_string(),
                doc_type: meta.doc_type,
                author: meta.author,
                timestamp: meta.timestamp,
                stage,
                content_preview: preview,
                direction: "self".to_string(),
            }
        }),
        None => None,
    };

    // 2. Downstream: docs referenced by the target
    let mut downstream = Vec::new();
    if let Some(content) = doc_store::read_doc_by_id(working_dir, doc_id).await {
        if let Ok((meta, _)) = documents::parse_doc(&content) {
            for num in documents::parse_refs(&meta.refs) {
                if let Some((ref_id, ref_content)) =
                    doc_store::find_by_numeric_id(working_dir, num).await
                {
                    if let Ok((ref_meta, ref_body)) = documents::parse_doc(&ref_content) {
                        let preview = ref_body
                            .lines()
                            .next()
                            .unwrap_or("")
                            .chars()
                            .take(80)
                            .collect();
                        let stage = stage_for_type(&ref_meta.doc_type);
                        downstream.push(ChainNode {
                            id: ref_id,
                            doc_type: ref_meta.doc_type,
                            author: ref_meta.author,
                            timestamp: ref_meta.timestamp,
                            stage,
                            content_preview: preview,
                            direction: "downstream".to_string(),
                        });
                    }
                }
            }
        }
    }

    // 3. Upstream: docs that reference the target (via RefIndex)
    let mut upstream = Vec::new();
    let index = RefIndex::load(working_dir).await;
    let index = if index.entries.is_empty() {
        super::ref_index::build_ref_index(working_dir).await
    } else {
        index
    };
    for ref_by_id in index.get_ref_by(doc_id) {
        if let Some(content) = doc_store::read_doc_by_id(working_dir, &ref_by_id).await {
            if let Ok((meta, body)) = documents::parse_doc(&content) {
                let preview = body.lines().next().unwrap_or("").chars().take(80).collect();
                let stage = stage_for_type(&meta.doc_type);
                upstream.push(ChainNode {
                    id: ref_by_id,
                    doc_type: meta.doc_type,
                    author: meta.author,
                    timestamp: meta.timestamp,
                    stage,
                    content_preview: preview,
                    direction: "upstream".to_string(),
                });
            }
        }
    }

    upstream.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    downstream.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    TraceResult {
        target: target_node,
        downstream,
        upstream,
    }
}

pub(crate) fn stage_for_type(doc_type: &str) -> String {
    match doc_type {
        "reqs" => "reqs",
        "dsgn" | "pdsg" | "ddtl" => "design",
        "plan" => "plan",
        "ctrt" => "contract",
        _ => "other",
    }
    .to_string()
}
