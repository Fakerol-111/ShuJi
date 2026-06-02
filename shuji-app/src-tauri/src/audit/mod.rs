use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::tool::documents;

/// A single audit log entry persisted to .shuji/audit.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub ts: String,
    pub event: String,
    pub role: String,
    pub doc_id: String,
    pub detail: String,
}

/// Append a single audit entry to .shuji/audit.jsonl.
pub async fn append(
    working_dir: &Path,
    event: &str,
    role: &str,
    doc_id: &str,
    detail: &str,
) {
    let entry = AuditEntry {
        ts: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        event: event.to_string(),
        role: role.to_string(),
        doc_id: doc_id.to_string(),
        detail: detail.to_string(),
    };
    let path = working_dir.join(".shuji").join("audit.jsonl");
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(json) = serde_json::to_string(&entry) {
        use tokio::io::AsyncWriteExt;
        if let Ok(mut f) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            let _ = f.write_all(format!("{}\n", json).as_bytes()).await;
        }
    }
}

/// Read all audit entries from .shuji/audit.jsonl.
pub async fn read_all(working_dir: &Path) -> Vec<AuditEntry> {
    let path = working_dir.join(".shuji").join("audit.jsonl");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

// ── Lineage ────────────────────────────────────────────────

const ALL_DOC_TYPES: &[&str] = &[
    "dsgn", "plan", "pdsg", "ddtl", "revw", "task", "ctrt", "rprt", "anls", "reqs", "precepts",
];

/// Read a document's content by its full ID (e.g. "dsgn_005").
async fn read_doc_by_id(working_dir: &Path, doc_id: &str) -> Option<String> {
    let prefix = doc_id.split('_').next().unwrap_or("");
    let dir = documents::type_to_dir(prefix);
    let rel_path = if dir.is_empty() {
        format!(".shuji/{}.md", doc_id)
    } else {
        format!(".shuji/{}/{}.md", dir, doc_id)
    };
    let full = crate::tool::resolve_scoped_path(working_dir, &rel_path)
        .await
        .ok()?;
    tokio::fs::read_to_string(&full).await.ok()
}

/// Find a document by its numeric ID (from refs), searching across all types.
async fn find_by_numeric_id(working_dir: &Path, num: u64) -> Option<(String, String)> {
    for prefix in ALL_DOC_TYPES {
        let doc_id = format!("{}_{:03}", prefix, num);
        if let Some(content) = read_doc_by_id(working_dir, &doc_id).await {
            return Some((doc_id, content));
        }
    }
    None
}

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
    let content = read_doc_by_id(working_dir, doc_id).await?;
    let (meta, _body) = documents::parse_doc(&content).ok()?;
    let ref_nums = documents::parse_refs(&meta.refs);

    let mut children = Vec::new();
    for num in &ref_nums {
        if let Some((child_id, _)) = find_by_numeric_id(working_dir, *num).await {
            if let Some(child) = Box::pin(build_lineage(working_dir, &child_id)).await {
                children.push(child);
            }
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

// ── Timeline ────────────────────────────────────────────────

/// Aggregated view of the audit log.
#[derive(Debug, Clone, Serialize)]
pub struct TimelineData {
    pub entries: Vec<AuditEntry>,
    pub summary: TimelineSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineSummary {
    pub total_events: usize,
    pub by_event: Vec<(String, usize)>,
    pub by_role: Vec<(String, usize)>,
}

/// Build an aggregated timeline from the audit log.
pub async fn build_timeline(working_dir: &Path) -> TimelineData {
    let entries = read_all(working_dir).await;

    use std::collections::HashMap;
    let mut by_event: HashMap<String, usize> = HashMap::new();
    let mut by_role: HashMap<String, usize> = HashMap::new();

    for e in &entries {
        *by_event.entry(e.event.clone()).or_default() += 1;
        *by_role.entry(e.role.clone()).or_default() += 1;
    }

    let mut by_event_vec: Vec<_> = by_event.into_iter().collect();
    let mut by_role_vec: Vec<_> = by_role.into_iter().collect();
    by_event_vec.sort_by(|a, b| b.1.cmp(&a.1));
    by_role_vec.sort_by(|a, b| b.1.cmp(&a.1));

    TimelineData {
        summary: TimelineSummary {
            total_events: entries.len(),
            by_event: by_event_vec,
            by_role: by_role_vec,
        },
        entries,
    }
}

// ── Delivery Report ────────────────────────────────────────

/// Generate a delivery report as markdown text.
pub async fn generate_report(working_dir: &Path) -> String {
    let entries = read_all(working_dir).await;
    if entries.is_empty() {
        return "## 交付报告\n\n尚无审计记录。\n".to_string();
    }

    use std::collections::HashMap;
    let mut by_event: HashMap<String, usize> = HashMap::new();
    let mut by_role: HashMap<String, usize> = HashMap::new();
    let mut docs_created: Vec<&AuditEntry> = Vec::new();

    for e in &entries {
        *by_event.entry(e.event.clone()).or_default() += 1;
        *by_role.entry(e.role.clone()).or_default() += 1;
        if e.event == "create_document" {
            docs_created.push(e);
        }
    }

    let first = entries.first().unwrap();
    let last = entries.last().unwrap();

    let mut report = String::new();
    report.push_str("## 交付报告\n\n");
    report.push_str(&format!("**工起**: {}\n\n", first.ts));
    report.push_str(&format!("**工讫**: {}\n\n", last.ts));
    report.push_str(&format!("**事件总数**: {}\n\n", entries.len()));

    let mut by_event_vec: Vec<_> = by_event.into_iter().collect();
    let mut by_role_vec: Vec<_> = by_role.into_iter().collect();
    by_event_vec.sort_by(|a, b| b.1.cmp(&a.1));
    by_role_vec.sort_by(|a, b| b.1.cmp(&a.1));

    report.push_str("### 事件统计\n\n");
    report.push_str("| 事件 | 次数 |\n|------|------|\n");
    for (evt, count) in &by_event_vec {
        let label = match evt.as_str() {
            "create_document" => "创建文档",
            "set_document_status" => "文档状态变更",
            "checkpoint" => "存档",
            "milestone" => "里程碑",
            _ => evt,
        };
        report.push_str(&format!("| {} | {} |\n", label, count));
    }

    report.push_str("\n### 部门活跃\n\n");
    report.push_str("| 部门 | 操作次数 |\n|------|----------|\n");
    for (role, count) in &by_role_vec {
        report.push_str(&format!("| {} | {} |\n", role, count));
    }

    report.push_str("\n### 文档产出\n\n");
    for doc in &docs_created {
        report.push_str(&format!("- `{}` — {}\n", doc.doc_id, doc.detail));
    }

    report
}
