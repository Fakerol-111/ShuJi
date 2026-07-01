//! Shared types for the document line subsystem.

use serde::{Deserialize, Serialize};

// ── Constants ───────────────────────────────────────────────

pub(crate) const ALL_DOC_TYPES: &[&str] = &[
    "dsgn", "plan", "pdsg", "ddtl", "revw", "task", "ctrt", "rprt", "anls", "reqs", "precepts",
];

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

// ── Public types ────────────────────────────────────────────

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

// ── Internal types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LineEventRecord {
    pub ts: String,
    pub run_id: String,
    pub event: String,
    pub node_id: String,
    #[serde(default)]
    pub detail: serde_json::Value,
}

pub(crate) struct DocInfo {
    pub doc_type: String,
    pub author: String,
    pub timestamp: String,
    pub status: String,
    pub approved_hash: String,
    pub body_hash: String,
    pub refs: Vec<String>,
}
