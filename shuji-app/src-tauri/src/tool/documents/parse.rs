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
            .map_err(|e| format!("计数器锁失败: {}", e))?;
        let current: u64 = std::fs::read_to_string(&counter_path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(1);
        std::fs::write(&counter_path, (current + 1).to_string())
            .map_err(|e| format!("计数器写入失败: {}", e))?;
        Ok(current)
    })
    .await
    .map_err(|_| "后台任务异常: next_id".to_string())?
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
}

/// Parse the YAML frontmatter and body from a document string.
pub(crate) fn parse_doc(content: &str) -> Result<(DocMeta, &str), String> {
    let body = content
        .strip_prefix("---\n")
        .ok_or_else(|| "缺少 YAML frontmatter 起始标记".to_string())?;
    let end = body
        .find("\n---")
        .ok_or_else(|| "缺少 YAML frontmatter 结束标记".to_string())?;
    let header = &body[..end];
    let body_text = body[end + 4..].trim_start();

    let mut id = String::new();
    let mut doc_type = String::new();
    let mut author = String::new();
    let mut timestamp = String::new();
    let mut refs = String::from("[-1]");
    let mut status = String::new();
    let mut notes = String::new();

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
                "notes" => notes = val.to_string(),
                _ => {}
            }
        }
    }

    if id.is_empty() {
        return Err("文档缺少 id 字段".to_string());
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
    format!("{}\n---\n{}", frontmatter, body)
}

/// Timestamp string for the current moment.
pub(super) fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Map English dept string to Chinese author name.
pub(crate) fn dept_to_author(dept: &str) -> &'static str {
    match dept {
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
pub(super) const MUST_APPROVE_TYPES: &[&str] = &["plan", "revw"];

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

/// Resolve the full path for a document by its ID.
pub(super) async fn resolve_doc_path(working_dir: &Path, id: &str) -> Result<PathBuf, String> {
    use crate::tool::resolve_scoped_path;
    let type_prefix = id.split('_').next().unwrap_or("");
    if type_prefix == "rprt" {
        find_rprt_path(working_dir, id)
            .await
            .ok_or_else(|| format!("文档 {} 不存在", id))
    } else if MUST_APPROVE_TYPES.contains(&type_prefix) {
        let dir = type_to_dir(type_prefix);
        let rel_path = format!(".shuji/{}/{}.md", dir, id);
        resolve_scoped_path(working_dir, &rel_path)
            .await
            .map_err(|e| format!("路径错误: {}", e))
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        resolve_scoped_path(working_dir, &rel_path)
            .await
            .map_err(|e| format!("路径错误: {}", e))
    }
}
