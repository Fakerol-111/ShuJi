use std::path::Path;

pub mod document_line;

pub use document_line::{
    active_run_id, analyze_impact, append_line_event, build_document_line,
    build_document_line_for_doc, list_document_line_runs, DocumentLineRun, EvidenceRef,
    ImpactAnalysis, ImpactNode, LineEdge, LineNode,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::tool::documents;

/// A single audit log entry persisted to .shuji/audit.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub ts: String,
    pub event: String,
    pub role: String,
    pub doc_id: String,
    pub detail: String,
    #[serde(default)]
    pub prev_hash: String,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub seq: u64,
}

/// Result of verifying the hash chain integrity of the audit log.
#[derive(Debug, Clone, Serialize)]
pub struct VerificationReport {
    pub total_entries: u64,
    pub chain_intact: bool,
    pub first_entry_hash: String,
    pub last_entry_hash: String,
    pub broken_links: Vec<BrokenLink>,
    pub first_tampered_seq: Option<u64>,
    /// Number of entries before hash chain was established (pre-upgrade records).
    pub pre_chain_entries: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrokenLink {
    pub seq: u64,
    pub expected_prev_hash: String,
    pub actual_prev_hash: String,
}

/// Compute SHA-256 of the canonical JSON + prev_hash chain.
fn compute_hash(json: &str, prev_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    hasher.update(b"\n");
    hasher.update(prev_hash.as_bytes());
    format!("{:064x}", hasher.finalize())
}

const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Get the hash of the last entry in the audit log.
async fn get_last_hash(path: &Path) -> String {
    let content = tokio::fs::read_to_string(path).await.unwrap_or_default();
    content
        .lines()
        .last()
        .and_then(|line| {
            serde_json::from_str::<AuditEntry>(line)
                .ok()
                .filter(|e| !e.hash.is_empty())
                .map(|e| e.hash)
        })
        .unwrap_or_else(|| ZERO_HASH.to_string())
}

/// Count the number of lines in the audit log.
async fn count_lines(path: &Path) -> u64 {
    let content = tokio::fs::read_to_string(path).await.unwrap_or_default();
    content.lines().count() as u64
}

/// Append a single audit entry to .shuji/audit.jsonl with hash chain.
pub async fn append(working_dir: &Path, event: &str, role: &str, doc_id: &str, detail: &str) {
    let path = working_dir.join(".shuji").join("audit.jsonl");
    if let Some(parent) = path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            log_console!("[audit] failed to create directory: {}", e);
        }
    }

    let prev_hash = get_last_hash(&path).await;
    let seq = count_lines(&path).await + 1;

    let mut entry = AuditEntry {
        ts: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        event: event.to_string(),
        role: role.to_string(),
        doc_id: doc_id.to_string(),
        detail: detail.to_string(),
        prev_hash: prev_hash.clone(),
        hash: String::new(),
        seq,
    };

    // Compute hash from entry JSON (without hash field) + prev_hash
    let json_no_hash = serde_json::to_string(&serde_json::json!({
        "ts": entry.ts,
        "event": entry.event,
        "role": entry.role,
        "doc_id": entry.doc_id,
        "detail": entry.detail,
        "prev_hash": entry.prev_hash,
        "seq": entry.seq,
    }))
    .unwrap_or_default();
    entry.hash = compute_hash(&json_no_hash, &prev_hash);

    if let Ok(json) = serde_json::to_string(&entry) {
        use tokio::io::AsyncWriteExt;
        match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            Ok(mut f) => {
                if let Err(e) = f.write_all(format!("{}\n", json).as_bytes()).await {
                    log_console!("[audit] failed to write audit.jsonl: {}", e);
                }
            }
            Err(e) => {
                log_console!("[audit] failed to open audit.jsonl: {}", e);
            }
        }
    } else {
        log_console!("[audit] failed to serialize audit entry");
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

/// Verify the SHA-256 hash chain integrity of the entire audit log.
/// Returns a report with all broken links detected.
///
/// Entries without hash fields (pre-upgrade records) are reported separately
/// and the chain is considered to start from the first entry with a non-empty hash.
pub async fn verify_audit_trail(working_dir: &Path) -> Result<VerificationReport, String> {
    let entries = read_all(working_dir).await;
    if entries.is_empty() {
        return Ok(VerificationReport {
            total_entries: 0,
            chain_intact: true,
            first_entry_hash: String::new(),
            last_entry_hash: String::new(),
            broken_links: vec![],
            first_tampered_seq: None,
            pre_chain_entries: 0,
        });
    }

    let mut pre_chain = 0u64;
    let mut expected_prev = ZERO_HASH.to_string();
    let mut broken_links = Vec::new();

    for entry in &entries {
        if entry.hash.is_empty() {
            // Pre-upgrade entry �?skip chain check
            pre_chain += 1;
            continue;
        }

        // Recompute expected hash
        let json_no_hash = serde_json::to_string(&serde_json::json!({
            "ts": entry.ts,
            "event": entry.event,
            "role": entry.role,
            "doc_id": entry.doc_id,
            "detail": entry.detail,
            "prev_hash": entry.prev_hash,
            "seq": entry.seq,
        }))
        .unwrap_or_default();
        let expected_hash = compute_hash(&json_no_hash, &expected_prev);

        if entry.prev_hash != expected_prev {
            broken_links.push(BrokenLink {
                seq: entry.seq,
                expected_prev_hash: expected_prev.clone(),
                actual_prev_hash: entry.prev_hash.clone(),
            });
        }

        if entry.hash != expected_hash {
            broken_links.push(BrokenLink {
                seq: entry.seq,
                expected_prev_hash: expected_hash,
                actual_prev_hash: entry.hash.clone(),
            });
        }

        expected_prev = entry.hash.clone();
    }

    let chain_intact = broken_links.is_empty();
    let last_entry = entries.iter().filter(|e| !e.hash.is_empty()).next_back();
    let first_entry = entries.iter().find(|e| !e.hash.is_empty());

    Ok(VerificationReport {
        total_entries: entries.len() as u64,
        chain_intact,
        first_entry_hash: first_entry.map(|e| e.hash.clone()).unwrap_or_default(),
        last_entry_hash: last_entry.map(|e| e.hash.clone()).unwrap_or_default(),
        first_tampered_seq: broken_links.first().map(|b| b.seq),
        broken_links,
        pre_chain_entries: pre_chain,
    })
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

    let content = read_doc_by_id(working_dir, doc_id).await?;
    let (meta, _body) = documents::parse_doc(&content).ok()?;
    let ref_nums = documents::parse_refs(&meta.refs);

    let mut children = Vec::new();
    for num in &ref_nums {
        if let Some((child_id, _)) = find_by_numeric_id(working_dir, *num).await {
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
    by_event_vec.sort_by_key(|b| std::cmp::Reverse(b.1));
    by_role_vec.sort_by_key(|b| std::cmp::Reverse(b.1));

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

// ── Diff Storage ────────────────────────────────────────────

/// Save a unified diff patch to `.shuji/audit/diffs/` for document change tracking.
/// `old_body` and `new_body` are the document body text before and after the change.
/// `doc_id` is the document ID (e.g. "dsgn_003"), and `event` is e.g. "modify_document".
pub async fn save_diff(
    working_dir: &Path,
    doc_id: &str,
    event: &str,
    old_body: &str,
    new_body: &str,
) {
    let diff_dir = working_dir.join(".shuji").join("audit").join("diffs");
    let _ = tokio::fs::create_dir_all(&diff_dir).await;
    let ts = chrono::Local::now().format("%Y%m%d%H%M%S");
    let diff_path = diff_dir.join(format!("{}_{}_{}.patch", doc_id, event, ts));
    let patch = diffy::create_patch(old_body, new_body);
    let patch_str = patch.to_string();
    // Only save if there's actual change content
    if patch_str.lines().count() > 2 {
        let _ = tokio::fs::write(&diff_path, &patch_str).await;
    }
}

// ── Checklist (结构化审计检查项) ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: String,
    pub description: String,
    pub category: String,
    pub status: String, // pending | pass | fail | na
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checklist {
    pub items: Vec<ChecklistItem>,
}

/// Load the audit checklist from `.shuji/audit/checklist.json`.
pub async fn load_checklist(working_dir: &Path) -> Checklist {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("checklist.json");
    tokio::fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or(Checklist { items: vec![] })
}

/// Save the audit checklist to `.shuji/audit/checklist.json`.
pub async fn save_checklist(working_dir: &Path, checklist: &Checklist) {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("checklist.json");
    let _ = tokio::fs::create_dir_all(path.parent().unwrap()).await;
    if let Ok(json) = serde_json::to_string_pretty(checklist) {
        let _ = tokio::fs::write(&path, &json).await;
    }
}

/// Initialize a checklist with standard items for a given audit category.
pub async fn init_checklist(working_dir: &Path, category: &str) -> String {
    let items = match category {
        "spec" => vec![
            ChecklistItem {
                id: "spec-001".into(),
                description: "All public functions have doc comments".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "spec-002".into(),
                description: "Naming follows Rust conventions (snake_case / CamelCase)".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "spec-003".into(),
                description: "No unused imports or variables".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "spec-004".into(),
                description: "Error handling complete (no unwrap/expect abuse)".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
        ],
        "test" => vec![
            ChecklistItem {
                id: "test-001".into(),
                description: "All public functions have corresponding tests".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "test-002".into(),
                description: "Tests cover edge cases".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "test-003".into(),
                description: "Tests can run independently (no shared mutable state)".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
        ],
        _ => vec![ChecklistItem {
            id: "gen-001".into(),
            description: format!("Audit category: {}", category),
            category: category.into(),
            status: "pending".into(),
            note: String::new(),
        }],
    };
    let count = items.len();
    let checklist = Checklist { items };
    save_checklist(working_dir, &checklist).await;
    format!("Created {} checklist items", count)
}

/// Update a single checklist item's status and note.
pub async fn update_checklist_item(
    working_dir: &Path,
    id: &str,
    status: &str,
    note: &str,
) -> Result<String, String> {
    let mut checklist = load_checklist(working_dir).await;
    if let Some(item) = checklist.items.iter_mut().find(|i| i.id == id) {
        item.status = status.to_string();
        if !note.is_empty() {
            item.note = note.to_string();
        }
        save_checklist(working_dir, &checklist).await;
        Ok(format!("Checklist item {} marked as {}", id, status))
    } else {
        Err(format!("Checklist item {} not found", id))
    }
}

// ── Violations (违规记录) ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub ts: String,
    pub severity: String, // error | warning | info
    pub rule_id: String,
    pub location: String,
    pub description: String,
    pub status: String, // open | fixed | waived
}

/// Record a violation to `.shuji/audit/violations.jsonl`.
pub async fn add_violation(
    working_dir: &Path,
    severity: &str,
    rule_id: &str,
    location: &str,
    description: &str,
) {
    let violation = Violation {
        ts: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        severity: severity.to_string(),
        rule_id: rule_id.to_string(),
        location: location.to_string(),
        description: description.to_string(),
        status: "open".to_string(),
    };
    let dir = working_dir.join(".shuji").join("audit");
    let _ = tokio::fs::create_dir_all(&dir).await;
    let path = dir.join("violations.jsonl");
    if let Ok(json) = serde_json::to_string(&violation) {
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

/// Read all violations.
pub async fn load_violations(working_dir: &Path) -> Vec<Violation> {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("violations.jsonl");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Update a violation's status (e.g. mark as fixed or waived).
pub async fn update_violation_status(
    working_dir: &Path,
    ts: &str,
    new_status: &str,
) -> Result<String, String> {
    let mut violations = load_violations(working_dir).await;
    if let Some(v) = violations.iter_mut().find(|v| v.ts == ts) {
        v.status = new_status.to_string();
        // Rewrite the file
        let path = working_dir
            .join(".shuji")
            .join("audit")
            .join("violations.jsonl");
        let _ = tokio::fs::create_dir_all(path.parent().unwrap()).await;
        let mut content = String::new();
        for v in &violations {
            if let Ok(json) = serde_json::to_string(v) {
                content.push_str(&json);
                content.push('\n');
            }
        }
        let _ = tokio::fs::write(&path, &content).await;
        Ok(format!("Violation record updated to {}", new_status))
    } else {
        Err(format!("No matching violation record found (ts={})", ts))
    }
}

// ── Auto-retrigger ─────────────────────────────────────────

/// Write a re-audit request file that the actor system can detect.
/// The `subject` is a document ID that 礼部 should re-audit.
pub async fn request_reauth(working_dir: &Path, subject: &str, reason: &str) -> String {
    let dir = working_dir.join(".shuji").join("audit");
    let _ = tokio::fs::create_dir_all(&dir).await;
    let path = dir.join("reauth_request.json");
    let request = serde_json::json!({
        "ts": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        "subject": subject,
        "reason": reason,
    });
    if let Ok(json) = serde_json::to_string_pretty(&request) {
        let _ = tokio::fs::write(&path, &json).await;
    }
    append(
        working_dir,
        "reauth_request",
        "system",
        subject,
        &format!("Requesting re-audit: {}", reason),
    )
    .await;
    format!("Re-audit request submitted: {} ({})", subject, reason)
}

/// Check if there's a pending re-auth request and clear it.
pub async fn consume_reauth_request(working_dir: &Path) -> Option<(String, String)> {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("reauth_request.json");
    let content = tokio::fs::read_to_string(&path).await.ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    let _ = tokio::fs::remove_file(&path).await;
    Some((
        v.get("subject")?.as_str()?.to_string(),
        v.get("reason")?.as_str()?.to_string(),
    ))
}

// ── Full-chain traceability ──────────────────────────────

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
    /// Documents that the target references (downstream: target �?refs).
    pub downstream: Vec<ChainNode>,
    /// Documents that reference the target (upstream: refs �?target).
    pub upstream: Vec<ChainNode>,
}

/// Trace a document in both directions: what it references AND what references it.
pub async fn trace_document(working_dir: &Path, doc_id: &str) -> TraceResult {
    // 1. Get the target document itself
    let target_node = match read_doc_by_id(working_dir, doc_id).await {
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
    if let Some(content) = read_doc_by_id(working_dir, doc_id).await {
        if let Ok((meta, _)) = documents::parse_doc(&content) {
            for num in documents::parse_refs(&meta.refs) {
                if let Some((ref_id, ref_content)) = find_by_numeric_id(working_dir, num).await {
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
        build_ref_index(working_dir).await
    } else {
        index
    };
    for ref_by_id in index.get_ref_by(doc_id) {
        if let Some(content) = read_doc_by_id(working_dir, &ref_by_id).await {
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

fn stage_for_type(doc_type: &str) -> String {
    match doc_type {
        "reqs" => "reqs",
        "dsgn" | "pdsg" | "ddtl" => "design",
        "plan" => "plan",
        "ctrt" => "contract",
        _ => "other",
    }
    .to_string()
}

/// Generate a delivery report as markdown text.
pub async fn generate_report(working_dir: &Path) -> String {
    let entries = read_all(working_dir).await;
    if entries.is_empty() {
        return "## Delivery Report\n\nNo audit records yet.\n".to_string();
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
    report.push_str("## Delivery Report\n\n");
    report.push_str(&format!("**Start**: {}\n\n", first.ts));
    report.push_str(&format!("**End**: {}\n\n", last.ts));
    report.push_str(&format!("**Total Events**: {}\n\n", entries.len()));

    let mut by_event_vec: Vec<_> = by_event.into_iter().collect();
    let mut by_role_vec: Vec<_> = by_role.into_iter().collect();
    by_event_vec.sort_by_key(|b| std::cmp::Reverse(b.1));
    by_role_vec.sort_by_key(|b| std::cmp::Reverse(b.1));

    report.push_str("### Event Summary\n\n");
    report.push_str("| Event | Count |\n|------|------|\n");
    for (evt, count) in &by_event_vec {
        let label = match evt.as_str() {
            "create_document" => "Create Document",
            "set_document_status" => "Document Status Change",
            "checkpoint" => "Checkpoint",
            "milestone" => "Milestone",
            _ => evt,
        };
        report.push_str(&format!("| {} | {} |\n", label, count));
    }

    report.push_str("\n### Department Activity\n\n");
    report.push_str("| Department | Operations |\n|------|----------|\n");
    for (role, count) in &by_role_vec {
        report.push_str(&format!("| {} | {} |\n", role, count));
    }

    report.push_str("\n### Document Output\n\n");
    for doc in &docs_created {
        report.push_str(&format!("- `{}` — {}\n", doc.doc_id, doc.detail));
    }

    if let Some(line) = build_document_line(working_dir, None).await {
        report.push_str("\n### Document Line Summary\n\n");
        report.push_str(&format!(
            "**Run**: {} ({}) — {}\n\n",
            line.run_id,
            line.status,
            line.session_label.as_deref().unwrap_or("-")
        ));
        let key_docs: Vec<_> = line
            .nodes
            .iter()
            .filter(|n| n.kind == "document")
            .take(12)
            .collect();
        if !key_docs.is_empty() {
            report.push_str(
                "| Document | Type | Status | Stale |\n|----------|------|--------|-------|\n",
            );
            for n in key_docs {
                let dtype = n.doc_type.as_deref().unwrap_or("-");
                let stale = if n.stale { "yes" } else { "-" };
                report.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    n.label, dtype, n.status, stale
                ));
            }
        }
        let semantic_ckpts: Vec<_> = line
            .nodes
            .iter()
            .filter(|n| n.kind == "checkpoint")
            .collect();
        if !semantic_ckpts.is_empty() {
            report.push_str("\n**Semantic Checkpoints**:\n");
            for c in semantic_ckpts {
                report.push_str(&format!(
                    "- {} ({}) — {}\n",
                    c.label,
                    c.status,
                    c.role.as_deref().unwrap_or("-")
                ));
            }
        }
        if let Some(v) = line.nodes.iter().find(|n| n.kind == "validation") {
            report.push_str(&format!("\n**Validation**: {}\n", v.status));
        }
    }

    report
}

// ── RefIndex: O(1) document reference lookup ─────────────────

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

/// Document type �?directory mapping (mirrors documents.rs TYPE_TO_DIR).
const TYPE_TO_DIR: &[(&str, &str)] = &[
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
        let _ = tokio::fs::create_dir_all(path.parent().unwrap()).await;
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

// ── Document query ────────────────────────────────────────────

/// Filter parameters for querying documents.
#[derive(Debug, Clone, Default, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize)]
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
