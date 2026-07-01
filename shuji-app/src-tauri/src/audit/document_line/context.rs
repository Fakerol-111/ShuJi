//! Internal context aggregation and graph building for document lines.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use super::events::load_line_events;
use super::scan::{
    extract_detail_field, is_doc_stale, load_validation, scan_all_docs, scan_diff_files,
};
use super::types::{DocInfo, EvidenceRef, ImpactAnalysis, ImpactNode, LineEdge, LineNode};
use crate::audit::log::{read_all, AuditEntry};
use crate::audit::ref_index::{build_ref_index, RefIndex};
use crate::pipeline::PlanRuntime;
use crate::storage::checkpoint::{load_index, CheckpointEntry};

pub(super) struct LineContext {
    audit: Vec<AuditEntry>,
    ref_index: RefIndex,
    pipeline: Option<PlanRuntime>,
    checkpoints: Vec<CheckpointEntry>,
    validation_ts: Option<String>,
    validation_pass: Option<bool>,
    docs: HashMap<String, DocInfo>,
    diff_files: Vec<(String, String, String)>,
    #[allow(dead_code)]
    line_events: Vec<super::types::LineEventRecord>,
    doc_to_run: HashMap<String, String>,
}

impl LineContext {
    pub(super) async fn load(working_dir: &Path) -> Self {
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

    pub(super) fn is_empty(&self) -> bool {
        self.docs.is_empty() && self.audit.is_empty() && self.pipeline.is_none()
    }

    pub(super) fn primary_run_id(&self) -> String {
        if let Some(ref rt) = self.pipeline {
            return rt.plan.plan_id.clone();
        }
        "legacy".into()
    }

    pub(super) fn run_ids(&self) -> Vec<String> {
        let mut ids: HashSet<String> = self.doc_to_run.values().cloned().collect();
        if ids.is_empty() {
            ids.insert("legacy".into());
        }
        let mut v: Vec<_> = ids.into_iter().collect();
        v.sort();
        v
    }

    pub(super) fn find_run_for_doc(&self, doc_id: &str) -> String {
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

    pub(super) fn build_run(
        &self,
        run_id: &str,
        focus_doc_id: Option<&str>,
    ) -> super::types::DocumentLineRun {
        use super::types::DocumentLineRun;

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

    pub(super) fn analyze_impact(&self, doc_id: &str) -> ImpactAnalysis {
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
