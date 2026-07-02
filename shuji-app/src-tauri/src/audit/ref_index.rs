use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::tool::documents;

use super::document_line::analyze_impact;

/// Document type to directory mapping (mirrors documents.rs TYPE_TO_DIR).
pub(crate) const TYPE_TO_DIR: &[(&str, &str)] = &[
    ("dsgn", "designs"),
    ("plan", "designs"),
    ("pdsg", "designs"),
    ("ddtl", "designs/detail"),
    ("revw", "reviews"),
    ("task", "tasks"),
    ("ctrt", "contracts"),
    ("rprt", "reports"),
    ("reqs", "requirements"),
    ("anls", "analysis"),
];

/// Maps doc_id to its forward refs and reverse refs (who references it).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RefIndex {
    pub entries: std::collections::HashMap<String, RefIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefIndexEntry {
    pub path: String,
    pub refs: Vec<u64>,
    pub ref_by: Vec<String>,
}

impl RefIndex {
    /// Load ref index from `.shuji/audit/ref_index.json`.
    pub async fn load(working_dir: &Path) -> Self {
        let path = working_dir
            .join(".shuji")
            .join("audit")
            .join("ref_index.json");
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save ref index to `.shuji/audit/ref_index.json`.
    pub async fn save(&self, working_dir: &Path) {
        let path = working_dir
            .join(".shuji")
            .join("audit")
            .join("ref_index.json");
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = tokio::fs::write(&path, &json).await;
        }
    }

    /// Get documents that reference `doc_id` (reverse refs — downstream impact).
    pub fn get_ref_by(&self, doc_id: &str) -> Vec<String> {
        self.entries
            .get(doc_id)
            .map(|e| e.ref_by.clone())
            .unwrap_or_default()
    }
}

/// Build a complete ref index by scanning all .shuji document directories.
pub async fn build_ref_index(working_dir: &Path) -> RefIndex {
    let mut index = RefIndex::default();
    let shuji_dir = working_dir.join(".shuji");

    for (_type_prefix, dir_name) in TYPE_TO_DIR {
        let dir = shuji_dir.join(dir_name);
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "md") {
                continue;
            }
            let Ok(content) = tokio::fs::read_to_string(&path).await else {
                continue;
            };
            let Ok((meta, _body)) = documents::parse_doc(&content) else {
                continue;
            };
            let ref_nums = documents::parse_refs(&meta.refs);
            let rel_path = path.strip_prefix(&shuji_dir).unwrap_or(&path);
            // Map numeric refs to full doc_ids for reverse-indexing
            for num in &ref_nums {
                let resolved = resolve_numeric_ref(working_dir, *num).await;
                let entry = index.entries.entry(resolved).or_insert(RefIndexEntry {
                    path: String::new(),
                    refs: vec![],
                    ref_by: vec![],
                });
                if !entry.ref_by.contains(&meta.id) {
                    entry.ref_by.push(meta.id.clone());
                }
            }
            index.entries.insert(
                meta.id.clone(),
                RefIndexEntry {
                    path: rel_path.to_string_lossy().to_string(),
                    refs: ref_nums,
                    ref_by: index
                        .entries
                        .get(&meta.id)
                        .map(|e| e.ref_by.clone())
                        .unwrap_or_default(),
                },
            );
        }
    }

    index
}

/// Resolve a numeric ref (e.g., "3") to a full doc ID by scanning known documents.
async fn resolve_numeric_ref(working_dir: &Path, num: u64) -> String {
    for (_type_prefix, dir_name) in TYPE_TO_DIR {
        let dir = working_dir.join(".shuji").join(dir_name);
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Some(id_part) = name_str.strip_suffix(".md") {
                if let Some(num_str) = id_part.rsplit('_').next() {
                    if num_str.parse::<u64>().ok() == Some(num) {
                        return id_part.to_string();
                    }
                }
            }
        }
    }
    format!("ref_{}", num)
}

/// Check immutability: returns the list of documents that reference `doc_id`.
/// If non-empty, modifying `doc_id` would impact downstream documents.
pub async fn check_immutability(working_dir: &Path, doc_id: &str) -> Vec<String> {
    let analysis = analyze_impact(working_dir, doc_id).await;
    if !analysis.blocking_chain.is_empty() && analysis.blocking_chain != doc_id {
        // Return downstream doc IDs from impact analysis
        return analysis
            .impacted
            .iter()
            .map(|n| n.node_id.clone())
            .collect();
    }
    let index = RefIndex::load(working_dir).await;
    if index.entries.is_empty() {
        let index = build_ref_index(working_dir).await;
        index.save(working_dir).await;
        return index.get_ref_by(doc_id);
    }
    index.get_ref_by(doc_id)
}

/// Synchronize the ref index after a document change.
pub async fn sync_ref_index(working_dir: &Path, _doc_id: &str) {
    let index = build_ref_index(working_dir).await;
    index.save(working_dir).await;
}
