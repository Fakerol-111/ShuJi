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
    let path = working_dir.join(".shuji").join("audit").join("checklist.json");
    tokio::fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or(Checklist { items: vec![] })
}

/// Save the audit checklist to `.shuji/audit/checklist.json`.
pub async fn save_checklist(working_dir: &Path, checklist: &Checklist) {
    let path = working_dir.join(".shuji").join("audit").join("checklist.json");
    let _ = tokio::fs::create_dir_all(path.parent().unwrap()).await;
    if let Ok(json) = serde_json::to_string_pretty(checklist) {
        let _ = tokio::fs::write(&path, &json).await;
    }
}

/// Initialize a checklist with standard items for a given audit category.
pub async fn init_checklist(working_dir: &Path, category: &str) -> String {
    let items = match category {
        "spec" => vec![
            ChecklistItem { id: "spec-001".into(), description: "所有公共函数有文档注释".into(), category: category.into(), status: "pending".into(), note: String::new() },
            ChecklistItem { id: "spec-002".into(), description: "命名符合 Rust 命名规范（snake_case / CamelCase）".into(), category: category.into(), status: "pending".into(), note: String::new() },
            ChecklistItem { id: "spec-003".into(), description: "无未使用的导入或变量".into(), category: category.into(), status: "pending".into(), note: String::new() },
            ChecklistItem { id: "spec-004".into(), description: "错误处理完整（无 unwrap/expect 滥用）".into(), category: category.into(), status: "pending".into(), note: String::new() },
        ],
        "test" => vec![
            ChecklistItem { id: "test-001".into(), description: "所有公共函数有对应测试".into(), category: category.into(), status: "pending".into(), note: String::new() },
            ChecklistItem { id: "test-002".into(), description: "测试覆盖边界条件".into(), category: category.into(), status: "pending".into(), note: String::new() },
            ChecklistItem { id: "test-003".into(), description: "测试可独立运行（无共享可变状态）".into(), category: category.into(), status: "pending".into(), note: String::new() },
        ],
        _ => vec![
            ChecklistItem { id: "gen-001".into(), description: format!("审计类别：{}", category), category: category.into(), status: "pending".into(), note: String::new() },
        ],
    };
    let count = items.len();
    let checklist = Checklist { items };
    save_checklist(working_dir, &checklist).await;
    format!("已创建 {} 条检查项", count)
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
        Ok(format!("检查项 {} 已标记为 {}", id, status))
    } else {
        Err(format!("检查项 {} 不存在", id))
    }
}

// ── Violations (违规记录) ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub ts: String,
    pub severity: String,   // error | warning | info
    pub rule_id: String,
    pub location: String,
    pub description: String,
    pub status: String,     // open | fixed | waived
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
        if let Ok(mut f) = tokio::fs::OpenOptions::new().create(true).append(true).open(&path).await {
            let _ = f.write_all(format!("{}\n", json).as_bytes()).await;
        }
    }
}

/// Read all violations.
pub async fn load_violations(working_dir: &Path) -> Vec<Violation> {
    let path = working_dir.join(".shuji").join("audit").join("violations.jsonl");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    content.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
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
        let path = working_dir.join(".shuji").join("audit").join("violations.jsonl");
        let _ = tokio::fs::create_dir_all(path.parent().unwrap()).await;
        let mut content = String::new();
        for v in &violations {
            if let Ok(json) = serde_json::to_string(v) {
                content.push_str(&json);
                content.push('\n');
            }
        }
        let _ = tokio::fs::write(&path, &content).await;
        Ok(format!("违规记录已更新为 {}", new_status))
    } else {
        Err(format!("未找到匹配的违规记录 (ts={})", ts))
    }
}

// ── Auto-retrigger ─────────────────────────────────────────

/// Write a re-audit request file that the actor system can detect.
/// The `subject` is a document ID that 礼部 should re-audit.
pub async fn request_reauth(
    working_dir: &Path,
    subject: &str,
    reason: &str,
) -> String {
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
    append(&working_dir, "reauth_request", "系统", subject, &format!("请求复验: {}", reason)).await;
    format!("已提交复验请求：{} ({})", subject, reason)
}

/// Check if there's a pending re-auth request and clear it.
pub async fn consume_reauth_request(working_dir: &Path) -> Option<(String, String)> {
    let path = working_dir.join(".shuji").join("audit").join("reauth_request.json");
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
    /// Documents that the target references (downstream: target → refs).
    pub downstream: Vec<ChainNode>,
    /// Documents that reference the target (upstream: refs → target).
    pub upstream: Vec<ChainNode>,
}

/// Trace a document in both directions: what it references AND what references it.
pub async fn trace_document(working_dir: &Path, doc_id: &str) -> TraceResult {
    // 1. Get the target document itself
    let target_node = match read_doc_by_id(working_dir, doc_id).await {
        Some(content) => {
            documents::parse_doc(&content).ok().map(|(meta, body)| {
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
            })
        }
        None => None,
    };

    // 2. Downstream: docs referenced by the target
    let mut downstream = Vec::new();
    if let Some(content) = read_doc_by_id(working_dir, doc_id).await {
        if let Ok((meta, _)) = documents::parse_doc(&content) {
            for num in documents::parse_refs(&meta.refs) {
                if let Some((ref_id, ref_content)) = find_by_numeric_id(working_dir, num).await {
                    if let Ok((ref_meta, ref_body)) = documents::parse_doc(&ref_content) {
                        let preview = ref_body.lines().next().unwrap_or("").chars().take(80).collect();
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

    // 3. Upstream: docs that reference the target
    let mut upstream = Vec::new();
    let num_part = doc_id.split('_').nth(1).and_then(|n| n.parse::<u64>().ok());
    if let Some(num) = num_part {
        for type_prefix in ALL_DOC_TYPES {
            let dir = documents::type_to_dir(type_prefix);
            let rel_dir = if dir.is_empty() {
                format!(".shuji/")
            } else {
                format!(".shuji/{}/", dir)
            };
            let dir_path = working_dir.join(&rel_dir);
            let mut rd = match tokio::fs::read_dir(&dir_path).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            while let Ok(Some(entry)) = rd.next_entry().await {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    if let Ok((meta, body)) = documents::parse_doc(&content) {
                        let ref_nums = documents::parse_refs(&meta.refs);
                        if ref_nums.contains(&num) {
                            let fname = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                            let preview = body.lines().next().unwrap_or("").chars().take(80).collect();
                            let stage = stage_for_type(&meta.doc_type);
                            upstream.push(ChainNode {
                                id: fname,
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

/// Document type → directory mapping (mirrors documents.rs TYPE_TO_DIR).
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
        let path = working_dir.join(".shuji").join("audit").join("ref_index.json");
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save ref index to `.shuji/audit/ref_index.json`.
    pub async fn save(&self, working_dir: &Path) {
        let path = working_dir.join(".shuji").join("audit").join("ref_index.json");
        let _ = tokio::fs::create_dir_all(path.parent().unwrap()).await;
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = tokio::fs::write(&path, &json).await;
        }
    }

    /// Add or update an entry for a document.
    pub fn upsert(&mut self, doc_id: &str, path: &str, refs: &[u64]) {
        // Clone old refs to avoid borrow conflicts
        let old_refs: Vec<u64> = self.entries.get(doc_id)
            .map(|e| e.refs.clone())
            .unwrap_or_default();
        let old_ref_by: Vec<String> = self.entries.get(doc_id)
            .map(|e| e.ref_by.clone())
            .unwrap_or_default();

        // Clean up old reverse refs
        for old_ref_id in &old_refs {
            let old_key = Self::numeric_to_doc_id(*old_ref_id);
            if let Some(e) = self.entries.get_mut(&old_key) {
                e.ref_by.retain(|d| d != doc_id);
            }
        }

        // Add new reverse refs
        for new_ref_id in refs {
            let new_key = Self::numeric_to_doc_id(*new_ref_id);
            let entry = self.entries.entry(new_key).or_insert(RefIndexEntry {
                path: String::new(),
                refs: vec![],
                ref_by: vec![],
            });
            if !entry.ref_by.contains(&doc_id.to_string()) {
                entry.ref_by.push(doc_id.to_string());
            }
        }

        self.entries.insert(doc_id.to_string(), RefIndexEntry {
            path: path.to_string(),
            refs: refs.to_vec(),
            ref_by: old_ref_by,
        });
    }

    /// Get documents that reference `doc_id` (reverse refs — downstream impact).
    pub fn get_ref_by(&self, doc_id: &str) -> Vec<String> {
        self.entries.get(doc_id)
            .map(|e| e.ref_by.clone())
            .unwrap_or_default()
    }

    fn numeric_to_doc_id(num: u64) -> String {
        // Scan TYPE_TO_DIR to find which prefix maps the numeric ID.
        // Since we don't know the type from just the number, we return
        // a placeholder — the index is keyed by full doc ID (type_num).
        // If the numeric ref doesn't resolve to a known entry, it's a
        // forward reference only.
        format!("ref_{}", num)
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
            if path.extension().map_or(true, |e| e != "md") { continue; }
            let Ok(content) = tokio::fs::read_to_string(&path).await else { continue };
            let Ok((meta, _body)) = documents::parse_doc(&content) else { continue };
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
            index.entries.insert(meta.id.clone(), RefIndexEntry {
                path: rel_path.to_string_lossy().to_string(),
                refs: ref_nums,
                ref_by: index.entries.get(&meta.id)
                    .map(|e| e.ref_by.clone())
                    .unwrap_or_default(),
            });
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
    // Simple approach: rebuild the index. For large projects,
    // incremental update can be implemented later.
    let index = build_ref_index(working_dir).await;
    index.save(working_dir).await;
}
