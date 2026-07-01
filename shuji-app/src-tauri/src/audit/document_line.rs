//! Document line: end-to-end audit view linking docs, pipeline steps, approvals,
//! validation, diffs, and semantic checkpoints for a single task run.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::log::{read_all, AuditEntry};
use super::ref_index::{build_ref_index, RefIndex};
use crate::pipeline::PlanRuntime;
use crate::storage::checkpoint::{load_index, CheckpointEntry};
use crate::tool::documents;

const ALL_DOC_TYPES: &[&str] = &[
    "dsgn", "plan", "pdsg", "ddtl", "revw", "task", "ctrt", "rprt", "anls", "reqs", "precepts",
];

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

// ── Public types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub source: String,
    pub ref_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineNode {
    pub node_id: String,
    pub kind: String,
    pub label: String,
    #[serde(default)]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_type: Option<String>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub highlight: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentLineRun {
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub status: String,
    pub nodes: Vec<LineNode>,
    pub edges: Vec<LineEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_doc_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactNode {
    pub node_id: String,
    pub kind: String,
    pub status: String,
    pub stale: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    pub source_doc_id: String,
    pub impacted: Vec<ImpactNode>,
    pub blocking_chain: String,
    pub chain_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LineEventRecord {
    ts: String,
    run_id: String,
    event: String,
    node_id: String,
    #[serde(default)]
    detail: serde_json::Value,
}

// ── Query API ───────────────────────────────────────────────────

/// Build the document line for a pipeline run (or the active/legacy run when `run_id` is None).
pub async fn build_document_line(
    working_dir: &Path,
    run_id: Option<&str>,
) -> Option<DocumentLineRun> {
    let ctx = LineContext::load(working_dir).await;
    if ctx.is_empty() {
        return None;
    }
    let rid = run_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| ctx.primary_run_id());
    Some(ctx.build_run(&rid, None))
}

/// Build the document line containing `doc_id`, highlighting that node.
pub async fn build_document_line_for_doc(
    working_dir: &Path,
    doc_id: &str,
) -> Option<DocumentLineRun> {
    let ctx = LineContext::load(working_dir).await;
    if ctx.is_empty() {
        return None;
    }
    let run_id = ctx.find_run_for_doc(doc_id);
    Some(ctx.build_run(&run_id, Some(doc_id)))
}

/// List known run IDs (pipeline plan_id or legacy bucket).
pub async fn list_document_line_runs(working_dir: &Path) -> Vec<String> {
    let ctx = LineContext::load(working_dir).await;
    ctx.run_ids()
}

/// Analyze downstream impact when modifying `doc_id`.
pub async fn analyze_impact(working_dir: &Path, doc_id: &str) -> ImpactAnalysis {
    let ctx = LineContext::load(working_dir).await;
    ctx.analyze_impact(doc_id)
}

// ── Incremental line events ───────────────────────────────────

/// Append a structured line event (Phase 2 runtime facts).
pub async fn append_line_event(
    working_dir: &Path,
    run_id: &str,
    event: &str,
    node_id: &str,
    detail: serde_json::Value,
) {
    let dir = working_dir.join(".shuji").join("audit").join("doc_lines");
    let _ = tokio::fs::create_dir_all(&dir).await;
    let path = dir.join("events.jsonl");
    let record = LineEventRecord {
        ts: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        run_id: run_id.to_string(),
        event: event.to_string(),
        node_id: node_id.to_string(),
        detail,
    };
    if let Ok(json) = serde_json::to_string(&record) {
        use tokio::io::AsyncWriteExt;
        if let Ok(mut f) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            let _ = f.write_all(format!("{json}\n").as_bytes()).await;
        }
    }
}

/// Resolve the active run_id from pipeline runtime or fallback.
pub async fn active_run_id(working_dir: &Path) -> String {
    if let Some(rt) = PlanRuntime::load_from(working_dir).await {
        return rt.plan.plan_id.clone();
    }
    "legacy".into()
}

// ── Internal builder ────────────────────────────────────────────

struct DocInfo {
    doc_type: String,
    author: String,
    timestamp: String,
    status: String,
    approved_hash: String,
    body_hash: String,
    refs: Vec<String>,
}

struct LineContext {
    audit: Vec<AuditEntry>,
    ref_index: RefIndex,
    pipeline: Option<PlanRuntime>,
    checkpoints: Vec<CheckpointEntry>,
    validation_ts: Option<String>,
    validation_pass: Option<bool>,
    docs: HashMap<String, DocInfo>,
    diff_files: Vec<(String, String, String)>,
    #[allow(dead_code)]
    line_events: Vec<LineEventRecord>,
    doc_to_run: HashMap<String, String>,
}

impl LineContext {
    async fn load(working_dir: &Path) -> Self {
        let wd = working_dir.to_path_buf();
        let audit = read_all(&wd).await;
        let mut ref_index = RefIndex::load(&wd).await;
        if ref_index.entries.is_empty() {
            ref_index = build_ref_index(&wd).await;
        }
        let pipeline = PlanRuntime::load_from(&wd).await;
        let checkpoints = load_index(&wd).await;
        let (validation_ts, validation_pass) = load_validation(&wd).await;
        let docs = scan_all_docs(&wd).await;
        let diff_files = scan_diff_files(&wd).await;
        let line_events = load_line_events(&wd).await;

        let mut doc_to_run = HashMap::new();
        if let Some(ref rt) = pipeline {
            let run_id = rt.plan.plan_id.clone();
            for doc_id in rt.artifacts.values() {
                doc_to_run.insert(doc_id.clone(), run_id.clone());
            }
        }
        for doc_id in docs.keys() {
            doc_to_run
                .entry(doc_id.clone())
                .or_insert_with(|| "legacy".to_string());
        }

        Self {
            audit,
            ref_index,
            pipeline,
            checkpoints,
            validation_ts,
            validation_pass,
            docs,
            diff_files,
            line_events,
            doc_to_run,
        }
    }

    fn is_empty(&self) -> bool {
        self.docs.is_empty() && self.audit.is_empty() && self.pipeline.is_none()
    }

    fn primary_run_id(&self) -> String {
        if let Some(ref rt) = self.pipeline {
            return rt.plan.plan_id.clone();
        }
        "legacy".into()
    }

    fn run_ids(&self) -> Vec<String> {
        let mut ids: HashSet<String> = self.doc_to_run.values().cloned().collect();
        if ids.is_empty() {
            ids.insert("legacy".into());
        }
        let mut v: Vec<_> = ids.into_iter().collect();
        v.sort();
        v
    }

    fn find_run_for_doc(&self, doc_id: &str) -> String {
        self.doc_to_run
            .get(doc_id)
            .cloned()
            .unwrap_or_else(|| self.primary_run_id())
    }

    fn doc_in_run(&self, doc_id: &str, run_id: &str) -> bool {
        self.doc_to_run
            .get(doc_id)
            .map(|r| r == run_id)
            .unwrap_or(run_id == "legacy")
    }

    fn build_run(&self, run_id: &str, focus_doc_id: Option<&str>) -> DocumentLineRun {
        let mut nodes: Vec<LineNode> = Vec::new();
        let mut edges: Vec<LineEdge> = Vec::new();
        let mut node_ids: HashSet<String> = HashSet::new();

        let mut add_node = |node: LineNode| {
            if node_ids.insert(node.node_id.clone()) {
                nodes.push(node);
            }
        };

        // Pipeline steps + produced artifacts
        if let Some(ref rt) = self.pipeline {
            if rt.plan.plan_id == run_id || run_id == "legacy" {
                for step in &rt.plan.steps {
                    let step_node = format!("step:{}", step.step_id);
                    let status = rt
                        .step_status
                        .get(&step.step_id)
                        .map(|s| format!("{s:?}"))
                        .unwrap_or_else(|| "pending".into());
                    add_node(LineNode {
                        node_id: step_node.clone(),
                        kind: "pipeline_step".into(),
                        label: step.description.clone(),
                        status: status.to_lowercase(),
                        role: step
                            .action_params
                            .get("target")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        timestamp: None,
                        doc_type: None,
                        evidence: vec![EvidenceRef {
                            source: "step_id".into(),
                            ref_id: step.step_id.clone(),
                            label: Some(step.action.clone()),
                        }],
                        stale: false,
                        highlight: false,
                    });

                    if let Some(doc_id) = rt.artifacts.get(&step.step_id) {
                        if self.doc_in_run(doc_id, run_id) {
                            edges.push(LineEdge {
                                from: step_node,
                                to: format!("doc:{doc_id}"),
                                relation: "produced_by".into(),
                            });
                        }
                    }
                }
            }
        }

        // Document nodes
        for (doc_id, info) in &self.docs {
            if !self.doc_in_run(doc_id, run_id) {
                continue;
            }
            let stale = is_doc_stale(info);
            add_node(LineNode {
                node_id: format!("doc:{doc_id}"),
                kind: "document".into(),
                label: doc_id.clone(),
                status: if info.status.is_empty() {
                    "-".into()
                } else {
                    info.status.clone()
                },
                role: Some(info.author.clone()),
                timestamp: Some(info.timestamp.clone()),
                doc_type: Some(info.doc_type.clone()),
                evidence: vec![EvidenceRef {
                    source: "doc_id".into(),
                    ref_id: doc_id.clone(),
                    label: None,
                }],
                stale,
                highlight: focus_doc_id == Some(doc_id.as_str()),
            });

            // Reference edges
            for ref_id in &info.refs {
                if self.docs.contains_key(ref_id) && self.doc_in_run(ref_id, run_id) {
                    edges.push(LineEdge {
                        from: format!("doc:{doc_id}"),
                        to: format!("doc:{ref_id}"),
                        relation: "references".into(),
                    });
                }
            }
        }

        // Approval events from audit log
        for entry in &self.audit {
            if entry.event != "set_document_status" || entry.doc_id.is_empty() {
                continue;
            }
            if !self.doc_in_run(&entry.doc_id, run_id) {
                continue;
            }
            let approval_id = format!("approval:{}", entry.seq);
            let hash = extract_detail_field(&entry.detail, "hash");
            add_node(LineNode {
                node_id: approval_id.clone(),
                kind: "approval".into(),
                label: format!("朱批 {}", entry.doc_id),
                status: "approved".into(),
                role: Some(entry.role.clone()),
                timestamp: Some(entry.ts.clone()),
                doc_type: None,
                evidence: vec![
                    EvidenceRef {
                        source: "audit_seq".into(),
                        ref_id: entry.seq.to_string(),
                        label: None,
                    },
                    EvidenceRef {
                        source: "approved_hash".into(),
                        ref_id: hash.unwrap_or_default(),
                        label: None,
                    },
                ],
                stale: false,
                highlight: false,
            });
            edges.push(LineEdge {
                from: approval_id,
                to: format!("doc:{}", entry.doc_id),
                relation: "approved_by".into(),
            });
        }

        // Diff nodes
        for (doc_id, event, filename) in &self.diff_files {
            if !self.doc_in_run(doc_id, run_id) {
                continue;
            }
            let diff_id = format!("diff:{filename}");
            add_node(LineNode {
                node_id: diff_id.clone(),
                kind: "diff".into(),
                label: format!("{doc_id} ({event})"),
                status: event.clone(),
                role: None,
                timestamp: None,
                doc_type: None,
                evidence: vec![EvidenceRef {
                    source: "diff_filename".into(),
                    ref_id: filename.clone(),
                    label: None,
                }],
                stale: false,
                highlight: false,
            });
            edges.push(LineEdge {
                from: format!("doc:{doc_id}"),
                to: diff_id,
                relation: "modified_by".into(),
            });
        }

        // Validation node
        if let Some(ref ts) = self.validation_ts {
            let pass = self.validation_pass.unwrap_or(false);
            let val_id: String = "validation:latest".into();
            add_node(LineNode {
                node_id: val_id.clone(),
                kind: "validation".into(),
                label: "交付验证".into(),
                status: if pass { "pass".into() } else { "fail".into() },
                role: Some("system".into()),
                timestamp: Some(ts.clone()),
                doc_type: None,
                evidence: vec![EvidenceRef {
                    source: "validation_report_path".into(),
                    ref_id: ".shuji/validate/latest.json".into(),
                    label: None,
                }],
                stale: false,
                highlight: false,
            });
            for (doc_id, info) in &self.docs {
                if !self.doc_in_run(doc_id, run_id) {
                    continue;
                }
                if ["ctrt", "rprt"].contains(&info.doc_type.as_str()) {
                    edges.push(LineEdge {
                        from: val_id.clone(),
                        to: format!("doc:{doc_id}"),
                        relation: "validated_by".into(),
                    });
                }
            }
        }

        // Semantic checkpoints (workspace_only excluded from document line view)
        for ckpt in &self.checkpoints {
            let kind = ckpt.kind.as_deref().unwrap_or("workspace_only");
            if kind == "workspace_only" {
                continue;
            }
            if let Some(ref ckpt_run) = ckpt.run_id {
                if ckpt_run != run_id {
                    continue;
                }
            }
            let ckpt_id = format!("checkpoint:{}", &ckpt.commit[..8.min(ckpt.commit.len())]);
            add_node(LineNode {
                node_id: ckpt_id.clone(),
                kind: "checkpoint".into(),
                label: ckpt.description.clone(),
                status: kind.to_string(),
                role: Some(ckpt.role.clone()),
                timestamp: Some(ckpt.ts.clone()),
                doc_type: None,
                evidence: vec![EvidenceRef {
                    source: "checkpoint_commit".into(),
                    ref_id: ckpt.commit.clone(),
                    label: ckpt.reason.clone(),
                }],
                stale: false,
                highlight: false,
            });
            if let Some(ref doc_id) = ckpt.doc_id {
                if self.doc_in_run(doc_id, run_id) {
                    edges.push(LineEdge {
                        from: format!("doc:{doc_id}"),
                        to: ckpt_id,
                        relation: "checkpointed_by".into(),
                    });
                }
            }
        }

        // Sort nodes: pipeline steps first, then docs by timestamp, then others
        nodes.sort_by(|a, b| {
            let rank = |k: &str| match k {
                "pipeline_step" => 0,
                "document" => 1,
                "approval" => 2,
                "diff" => 3,
                "validation" => 4,
                "checkpoint" => 5,
                _ => 6,
            };
            rank(&a.kind)
                .cmp(&rank(&b.kind))
                .then_with(|| a.timestamp.cmp(&b.timestamp))
        });

        let (started_at, completed_at, status, plan_id, session_label) = self.run_metadata(run_id);

        DocumentLineRun {
            run_id: run_id.to_string(),
            plan_id,
            session_label,
            started_at,
            completed_at,
            status,
            nodes,
            edges,
            focus_doc_id: focus_doc_id.map(|s| s.to_string()),
        }
    }

    fn run_metadata(
        &self,
        run_id: &str,
    ) -> (
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    ) {
        if let Some(ref rt) = self.pipeline {
            if rt.plan.plan_id == run_id {
                let started = rt.plan.created.clone();
                let completed = if rt.all_done() {
                    self.audit.last().map(|e| e.ts.clone())
                } else {
                    None
                };
                let status = if rt.all_done() {
                    "complete"
                } else if rt.current_step.is_some() {
                    "active"
                } else {
                    "unknown"
                };
                return (
                    Some(started),
                    completed,
                    status.into(),
                    Some(rt.plan.plan_id.clone()),
                    Some(rt.plan.summary.clone()),
                );
            }
        }
        let started = self.audit.first().map(|e| e.ts.clone());
        let completed = self.audit.last().map(|e| e.ts.clone());
        (started, completed, "unknown".into(), None, None)
    }

    fn analyze_impact(&self, doc_id: &str) -> ImpactAnalysis {
        let mut impacted: Vec<ImpactNode> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        queue.push_back(doc_id.to_string());

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current.clone()) {
                continue;
            }
            let ref_by = self.ref_index.get_ref_by(&current);
            for downstream in ref_by {
                if visited.contains(&downstream) {
                    continue;
                }
                queue.push_back(downstream.clone());

                let (kind, status, stale, reason) = if let Some(info) = self.docs.get(&downstream) {
                    let stale = is_doc_stale(info)
                        || (info.doc_type == "revw"
                            && info.status == "approved"
                            && self.docs.get(doc_id).map(is_doc_stale).unwrap_or(false));
                    let reason = if stale {
                        format!("上游 {} 已变更，需重新确认", doc_id)
                    } else if info.status == "in_review" {
                        "审查中，阻塞下游执行".into()
                    } else if info.status == "rejected" {
                        "已驳回".into()
                    } else {
                        "下游引用".into()
                    };
                    ("document".into(), info.status.clone(), stale, reason)
                } else {
                    (
                        "document".into(),
                        "missing".into(),
                        true,
                        "引用文档不存在".into(),
                    )
                };

                impacted.push(ImpactNode {
                    node_id: downstream.clone(),
                    kind,
                    status: if status.is_empty() {
                        "-".into()
                    } else {
                        status
                    },
                    stale,
                    reason,
                });
            }
        }

        // Source doc stale check
        if let Some(info) = self.docs.get(doc_id) {
            if is_doc_stale(info) {
                impacted.insert(
                    0,
                    ImpactNode {
                        node_id: doc_id.to_string(),
                        kind: "document".into(),
                        status: info.status.clone(),
                        stale: true,
                        reason: "已批准文档内容 hash 与 approved_hash 不一致".into(),
                    },
                );
            }
        }

        let chain_path = build_blocking_chain(doc_id, &self.docs, &self.ref_index);
        let blocking_chain = chain_path.join(" -> ");

        ImpactAnalysis {
            source_doc_id: doc_id.to_string(),
            impacted,
            blocking_chain,
            chain_path,
        }
    }
}

fn build_blocking_chain(
    source: &str,
    docs: &HashMap<String, DocInfo>,
    ref_index: &RefIndex,
) -> Vec<String> {
    let mut chain = vec![source.to_string()];
    let mut current = source.to_string();
    let mut seen: HashSet<String> = HashSet::new();

    loop {
        if !seen.insert(current.clone()) {
            break;
        }
        let ref_by = ref_index.get_ref_by(&current);
        let next = ref_by.into_iter().find(|id| {
            docs.get(id)
                .map(|d| d.status == "in_review" || d.status == "rejected" || is_doc_stale(d))
                .unwrap_or(true)
        });
        match next {
            Some(id) => {
                let label = docs
                    .get(&id)
                    .map(|d| {
                        if d.status.is_empty() {
                            format!("{id}(pending)")
                        } else {
                            format!("{id}({})", d.status)
                        }
                    })
                    .unwrap_or_else(|| format!("{id}(blocked)"));
                chain.push(label);
                current = id;
            }
            None => break,
        }
    }
    chain
}

fn is_doc_stale(info: &DocInfo) -> bool {
    !info.approved_hash.is_empty() && info.approved_hash != info.body_hash
}

fn hash_body(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn extract_detail_field(detail: &str, key: &str) -> Option<String> {
    detail.split(';').find_map(|part| {
        let part = part.trim();
        part.strip_prefix(&format!("{key}="))
            .map(|v| v.trim().to_string())
    })
}

async fn scan_all_docs(working_dir: &Path) -> HashMap<String, DocInfo> {
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

async fn scan_diff_files(working_dir: &Path) -> Vec<(String, String, String)> {
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

async fn load_validation(working_dir: &Path) -> (Option<String>, Option<bool>) {
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

async fn load_line_events(working_dir: &Path) -> Vec<LineEventRecord> {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("doc_lines")
        .join("events.jsonl");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
