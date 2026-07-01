use std::path::Path;

use serde::Serialize;

use crate::tool::documents;

use super::doc_store;

/// A node in the document lineage tree.
#[derive(Debug, Clone, Serialize)]
pub struct LineageNode {
    pub id: String,
    pub doc_type: String,
    pub author: String,
    pub timestamp: String,
    pub status: String,
    pub refs: Vec<u64>,
    pub children: Vec<LineageNode>,
}

/// Build a lineage tree for a document by recursively following refs.
pub async fn build_lineage(working_dir: &Path, doc_id: &str) -> Option<LineageNode> {
    let mut visited = std::collections::HashSet::new();
    build_lineage_inner(working_dir, doc_id, &mut visited).await
}

async fn build_lineage_inner(
    working_dir: &Path,
    doc_id: &str,
    visited: &mut std::collections::HashSet<String>,
) -> Option<LineageNode> {
    if visited.contains(doc_id) {
        return None;
    }
    visited.insert(doc_id.to_string());

    let content = doc_store::read_doc_by_id(working_dir, doc_id).await?;
    let (meta, _body) = documents::parse_doc(&content).ok()?;
    let ref_nums = documents::parse_refs(&meta.refs);

    let mut children = Vec::new();
    for num in &ref_nums {
        if let Some((child_id, _)) = doc_store::find_by_numeric_id(working_dir, *num).await {
            if let Some(child) =
                Box::pin(build_lineage_inner(working_dir, &child_id, visited)).await
            {
                children.push(child);
            }
        } else {
            children.push(LineageNode {
                id: format!("ref_{num}"),
                doc_type: "missing".to_string(),
                author: String::new(),
                timestamp: String::new(),
                status: "missing".to_string(),
                refs: vec![],
                children: vec![],
            });
        }
    }

    Some(LineageNode {
        id: doc_id.to_string(),
        doc_type: meta.doc_type,
        author: meta.author,
        timestamp: meta.timestamp,
        status: meta.status,
        refs: ref_nums,
        children,
    })
}
