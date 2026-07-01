use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::tool::documents;

use super::ref_index::TYPE_TO_DIR;

/// Filter parameters for querying documents.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DocQuery {
    pub doc_type: Option<Vec<String>>,
    pub author: Option<String>,
    pub status: Option<Vec<String>>,
    pub refs_id: Option<u64>,
    pub keyword: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Summary of a document for query results.
#[derive(Debug, Clone, Serialize)]
pub struct DocSummary {
    pub id: String,
    pub doc_type: String,
    pub author: String,
    pub timestamp: String,
    pub status: String,
    pub refs: String,
    pub preview: String,
}

/// Query documents with combined filters.
pub async fn query_documents(working_dir: &Path, filter: &DocQuery) -> Vec<DocSummary> {
    let mut results = Vec::new();
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
            let Ok((meta, body)) = documents::parse_doc(&content) else {
                continue;
            };

            if let Some(ref types) = filter.doc_type {
                if !types.iter().any(|t| t == &meta.doc_type) {
                    continue;
                }
            }
            if let Some(ref author) = filter.author {
                if &meta.author != author {
                    continue;
                }
            }
            if let Some(ref statuses) = filter.status {
                let doc_status = if meta.status.is_empty() {
                    "-"
                } else {
                    &meta.status
                };
                if !statuses.iter().any(|s| s == doc_status) {
                    continue;
                }
            }
            if let Some(since) = &filter.since {
                if meta.timestamp < *since {
                    continue;
                }
            }
            if let Some(until) = &filter.until {
                if meta.timestamp > *until {
                    continue;
                }
            }
            if let Some(ref kw) = filter.keyword {
                if !body.contains(kw.as_str()) && !meta.id.contains(kw.as_str()) {
                    continue;
                }
            }
            if let Some(refs_num) = filter.refs_id {
                let ref_nums = documents::parse_refs(&meta.refs);
                if !ref_nums.contains(&refs_num) {
                    continue;
                }
            }

            let preview = body
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(120)
                .collect();
            results.push(DocSummary {
                id: meta.id.clone(),
                doc_type: meta.doc_type,
                author: meta.author,
                timestamp: meta.timestamp,
                status: meta.status,
                refs: meta.refs,
                preview,
            });
        }
    }

    results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let offset = filter.offset.unwrap_or(0);
    let limit = filter.limit.unwrap_or(100);
    results.into_iter().skip(offset).take(limit).collect()
}
