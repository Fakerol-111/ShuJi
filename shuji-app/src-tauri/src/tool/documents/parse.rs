use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// ── Counter ─────────────────────────────────────────────────────────
static COUNTER_LOCK: Mutex<()> = Mutex::new(());

/// Get the next ID from the project-local counter at `.shuji/_counter`.
pub(crate) async fn next_id(working_dir: &Path) -> Result<u64, String> {
    let counter_path = working_dir.join(".shuji/_counter");
    tokio::task::spawn_blocking(move || {
        let _lock = COUNTER_LOCK
            .lock()
            .map_err(|e| format!("Counter lock failed: {}", e))?;
        let current: u64 = std::fs::read_to_string(&counter_path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(1);
        std::fs::write(&counter_path, (current + 1).to_string())
            .map_err(|e| format!("Counter write failed: {}", e))?;
        Ok(current)
    })
    .await
    .map_err(|_| "Background task failure: next_id".to_string())?
}

/// ── YAML frontmatter helpers ───────────────────────────────────────
pub(crate) struct DocMeta {
    pub(crate) id: String,
    pub(crate) doc_type: String,
    pub(crate) author: String,
    pub(crate) timestamp: String,
    pub(crate) refs: String,
    pub(crate) status: String,
    pub(crate) notes: String,
    pub(crate) approved_hash: String,
}

/// Separator for multiple 朱批 entries in frontmatter `notes` (must stay single-line).
pub(super) const NOTES_ENTRY_SEP: &str = " | ";

/// Append a note entry; keeps notes on one frontmatter line for line-based parsing.
pub(super) fn append_note_entry(existing: &str, entry: &str) -> String {
    let existing = existing.replace('\n', NOTES_ENTRY_SEP);
    if existing.is_empty() {
        entry.to_string()
    } else {
        format!("{existing}{NOTES_ENTRY_SEP}{entry}")
    }
}

/// Parse the YAML frontmatter and body from a document string.
pub(crate) fn parse_doc(content: &str) -> Result<(DocMeta, &str), String> {
    let body = content
        .strip_prefix("---\n")
        .ok_or_else(|| "Missing YAML frontmatter start marker".to_string())?;
    let end = body
        .find("\n---")
        .ok_or_else(|| "Missing YAML frontmatter end marker".to_string())?;
    let header = &body[..end];
    let body_text = body[end + 4..].trim_start();

    let mut id = String::new();
    let mut doc_type = String::new();
    let mut author = String::new();
    let mut timestamp = String::new();
    let mut refs = String::from("[-1]");
    let mut status = String::new();
    let mut notes = String::new();
    let mut approved_hash = String::new();

    for line in header.lines() {
        if let Some((key, val)) = line.split_once(": ") {
            let val = val.trim();
            match key {
                "id" => id = val.to_string(),
                "type" => doc_type = val.to_string(),
                "author" => author = val.to_string(),
                "timestamp" => timestamp = val.to_string(),
                "refs" => refs = val.to_string(),
                "status" => status = val.to_string(),
                "notes" => notes = val.replace('\n', NOTES_ENTRY_SEP),
                "approved_hash" => approved_hash = val.to_string(),
                _ => {}
            }
        }
    }

    if id.is_empty() {
        return Err("Document missing id field".to_string());
    }

    Ok((
        DocMeta {
            id,
            doc_type,
            author,
            timestamp,
            refs,
            status,
            notes,
            approved_hash,
        },
        body_text,
    ))
}

/// Build a full document string from metadata and body.
pub(super) fn build_doc(meta: &DocMeta, body: &str) -> String {
    let mut frontmatter = format!(
        "---\nid: {}\ntype: {}\nauthor: {}\ntimestamp: {}\nrefs: {}",
        meta.id, meta.doc_type, meta.author, meta.timestamp, meta.refs,
    );
    if !meta.status.is_empty() {
        frontmatter += &format!("\nstatus: {}", meta.status);
    }
    if !meta.notes.is_empty() {
        frontmatter += &format!("\nnotes: {}", meta.notes);
    }
    if !meta.approved_hash.is_empty() {
        frontmatter += &format!("\napproved_hash: {}", meta.approved_hash);
    }
    format!("{}\n---\n{}", frontmatter, body)
}

/// Timestamp string for the current moment.
pub(super) fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Map English dept string to Chinese author name.
pub(crate) fn dept_to_author(dept: &str) -> &'static str {
    match dept.to_lowercase().as_str() {
        "zhongshuling" => "中书令",
        "menxiashizhong" => "门下侍中",
        "neige" => "内阁",
        "shangshuling" => "尚书令",
        "libushangshu" => "吏部",
        "bingbushangshu" => "兵部",
        "gongbushangshu" => "工部",
        "xingbushangshu" => "刑部",
        "liburshangshu" => "礼部",
        "hubu" => "户部",
        "requirements_agent" | "survey_agent" => "内阁",
        _ => "未知",
    }
}

/// Map document type prefix to subdirectory under `.shuji/`.
pub(crate) fn type_to_dir(doc_type: &str) -> &'static str {
    match doc_type {
        "dsgn" | "plan" | "pdsg" => "designs",
        "ddtl" => "designs/detail",
        "revw" => "reviews",
        "task" => "tasks",
        "ctrt" => "contracts",
        "rprt" => "reports",
        "anls" => "analysis",
        "reqs" => "requirements",
        "precepts" => "",
        _ => "misc",
    }
}

/// Build the full relative path for a rprt document, scoped to the author department.
pub(super) fn rprt_rel_path(dept: &str, doc_id: &str) -> String {
    format!(".shuji/reports/{}/{}.md", dept, doc_id)
}

/// Search for a report document across all dept subdirectories.
pub(super) async fn find_rprt_path(working_dir: &Path, id: &str) -> Option<PathBuf> {
    let reports_dir = working_dir.join(".shuji/reports");
    let id = id.to_string();
    tokio::task::spawn_blocking(move || {
        let entries = std::fs::read_dir(&reports_dir).ok()?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let candidate = path.join(format!("{}.md", id));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

/// Must-approve document types that require emperor approval.
pub(super) const MUST_APPROVE_TYPES: &[&str] = &["revw"];

/// Parse refs string like "[3, 4]" or "[-1]" into a Vec<u64>.
pub(crate) fn parse_refs(refs: &str) -> Vec<u64> {
    let inner = refs.trim().trim_start_matches('[').trim_end_matches(']');
    if inner.is_empty() || inner == "-1" {
        return vec![];
    }
    inner
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

const ALL_DOC_TYPE_PREFIXES: &[&str] = &[
    "dsgn", "plan", "pdsg", "ddtl", "revw", "task", "ctrt", "rprt", "reqs", "anls", "precepts",
];

/// Resolve a numeric ref (e.g. 3) to a full document ID by scanning known directories.
pub(crate) async fn resolve_ref_doc_id(working_dir: &Path, num: u64) -> Option<String> {
    for prefix in ALL_DOC_TYPE_PREFIXES {
        for candidate in [format!("{prefix}_{num}"), format!("{prefix}_{num:03}")] {
            if let Ok(path) = resolve_doc_path(working_dir, &candidate).await {
                if path.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Normalize a raw document ID from LLM tool calls (strip `.md`, resolve numeric refs).
pub(crate) async fn normalize_doc_id(working_dir: &Path, raw: &str) -> String {
    let mut id = raw.trim().to_string();
    if id.ends_with(".md") {
        id.truncate(id.len() - 3);
    }
    if id.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(num) = id.parse::<u64>() {
            if let Some(resolved) = resolve_ref_doc_id(working_dir, num).await {
                return resolved;
            }
        }
    }
    id
}

/// Collect document IDs (without `.md`) from a `.shuji` subdirectory for error hints.
pub(crate) async fn list_doc_ids_in_subdir(
    working_dir: &Path,
    subdir: &str,
    limit: usize,
) -> Vec<String> {
    let dir = working_dir.join(".shuji").join(subdir);
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(rd) => rd,
        Err(_) => return vec![],
    };
    let mut ids = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".md") {
            if id.contains('_') {
                ids.push(id.to_string());
            }
        }
    }
    ids.sort();
    ids.truncate(limit);
    ids
}

/// Find the newest design/plan document by numeric suffix (for pipeline review handoff).
pub(crate) async fn find_latest_design_doc_id(working_dir: &Path) -> Option<String> {
    find_latest_doc_with_prefixes(working_dir, &["dsgn", "plan", "pdsg", "ddtl"]).await
}

/// Find the newest document matching any of the given type prefixes.
pub(crate) async fn find_latest_doc_with_prefixes(
    working_dir: &Path,
    type_prefixes: &[&str],
) -> Option<String> {
    let mut best: Option<(u64, String)> = None;
    let subdirs = [
        "designs",
        "designs/detail",
        "reviews",
        "requirements",
        "tasks",
        "contracts",
        "analysis",
    ];
    for subdir in subdirs {
        let dir = working_dir.join(".shuji").join(subdir);
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(id) = name.strip_suffix(".md") else {
                continue;
            };
            let prefix = id.split('_').next().unwrap_or("");
            if !type_prefixes.contains(&prefix) {
                continue;
            }
            let num = id.rsplit('_').next()?.parse::<u64>().ok()?;
            if best.as_ref().is_none_or(|(n, _)| num >= *n) {
                best = Some((num, id.to_string()));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// Like `find_latest_doc_with_prefixes`, but only returns documents whose body
/// is non-empty (after trimming).  Used by artifact fallback to avoid picking
/// up empty `revw` shells that were created but never appended to.
pub(crate) async fn find_latest_non_empty_doc_with_prefixes(
    working_dir: &Path,
    type_prefixes: &[&str],
) -> Option<String> {
    let mut candidates: Vec<(u64, String)> = Vec::new();
    let subdirs = [
        "designs",
        "designs/detail",
        "reviews",
        "requirements",
        "tasks",
        "contracts",
        "analysis",
    ];
    for subdir in subdirs {
        let dir = working_dir.join(".shuji").join(subdir);
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy().to_string();
            let Some(id) = name.strip_suffix(".md") else {
                continue;
            };
            let prefix = id.split('_').next().unwrap_or("");
            if !type_prefixes.contains(&prefix) {
                continue;
            }
            // Read and parse to check body is non-empty
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                if let Ok((_, body)) = parse_doc(&content) {
                    if body.trim().is_empty() {
                        continue;
                    }
                } else {
                    continue;
                }
            } else {
                continue;
            }
            let num = id.rsplit('_').next()?.parse::<u64>().ok()?;
            candidates.push((num, id.to_string()));
        }
    }
    candidates
        .into_iter()
        .max_by_key(|(n, _)| *n)
        .map(|(_, id)| id)
}

/// Resolve the full path for a document by its ID.
pub(super) async fn resolve_doc_path(working_dir: &Path, id: &str) -> Result<PathBuf, String> {
    use crate::tool::resolve_scoped_path;
    let type_prefix = id.split('_').next().unwrap_or("");
    if type_prefix == "rprt" {
        find_rprt_path(working_dir, id)
            .await
            .ok_or_else(|| format!("Document {} not found", id))
    } else if MUST_APPROVE_TYPES.contains(&type_prefix) {
        let dir = type_to_dir(type_prefix);
        let rel_path = format!(".shuji/{}/{}.md", dir, id);
        resolve_scoped_path(working_dir, &rel_path)
            .await
            .map_err(|e| format!("Path error: {}", e))
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        resolve_scoped_path(working_dir, &rel_path)
            .await
            .map_err(|e| format!("Path error: {}", e))
    }
}
