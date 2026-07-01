//! File scanning utilities for the document line subsystem.

use std::collections::HashMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::types::{DocInfo, ALL_DOC_TYPES, TYPE_TO_DIR};
use crate::tool::documents;

pub(crate) fn hash_body(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn extract_detail_field(detail: &str, key: &str) -> Option<String> {
    detail.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix(&format!("{key}="))
            .map(|v| v.trim().to_string())
    })
}

pub(crate) fn is_doc_stale(info: &DocInfo) -> bool {
    !info.approved_hash.is_empty() && info.approved_hash != info.body_hash
}

async fn resolve_numeric_ref(working_dir: &Path, num: u64) -> Option<String> {
    for prefix in ALL_DOC_TYPES {
        let doc_id = format!("{prefix}_{num:03}");
        let dir = documents::type_to_dir(prefix);
        let rel = if dir.is_empty() {
            format!(".shuji/{doc_id}.md")
        } else {
            format!(".shuji/{dir}/{doc_id}.md")
        };
        if crate::tool::resolve_scoped_path(working_dir, &rel)
            .await
            .map(|p| p.exists())
            .unwrap_or(false)
        {
            return Some(doc_id);
        }
    }
    None
}

pub(crate) async fn scan_all_docs(working_dir: &Path) -> HashMap<String, DocInfo> {
    let mut docs = HashMap::new();
    let shuji = working_dir.join(".shuji");

    for (_prefix, dir_name) in TYPE_TO_DIR {
        let dir = shuji.join(dir_name);
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
            let ref_nums = documents::parse_refs(&meta.refs);
            let mut refs = Vec::new();
            for num in ref_nums {
                if let Some(id) = resolve_numeric_ref(working_dir, num).await {
                    refs.push(id);
                }
            }
            docs.insert(
                meta.id.clone(),
                DocInfo {
                    doc_type: meta.doc_type,
                    author: meta.author,
                    timestamp: meta.timestamp,
                    status: meta.status,
                    approved_hash: meta.approved_hash,
                    body_hash: hash_body(body),
                    refs,
                },
            );
        }
    }
    docs
}

pub(crate) async fn scan_diff_files(working_dir: &Path) -> Vec<(String, String, String)> {
    let diff_dir = working_dir.join(".shuji").join("audit").join("diffs");
    let mut out = Vec::new();
    let mut rd = match tokio::fs::read_dir(&diff_dir).await {
        Ok(rd) => rd,
        Err(_) => return out,
    };
    while let Ok(Some(entry)) = rd.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".patch") {
            continue;
        }
        let stripped = name.strip_suffix(".patch").unwrap_or(&name);
        let parts: Vec<&str> = stripped.splitn(3, '_').collect();
        if parts.len() >= 2 {
            out.push((parts[0].to_string(), parts[1].to_string(), name));
        }
    }
    out
}

pub(crate) async fn load_validation(working_dir: &Path) -> (Option<String>, Option<bool>) {
    let path = working_dir
        .join(".shuji")
        .join("validate")
        .join("latest.json");
    let Ok(content) = tokio::fs::read_to_string(&path).await else {
        return (None, None);
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) else {
        return (None, None);
    };
    let ts = v.get("ts").and_then(|t| t.as_str()).map(|s| s.to_string());
    let pass = v.get("overall_pass").and_then(|p| p.as_bool());
    (ts, pass)
}
