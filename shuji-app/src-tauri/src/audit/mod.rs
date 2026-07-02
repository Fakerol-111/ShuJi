pub mod checklist;
pub mod diff;
pub mod doc_store;
pub mod document_line;
pub mod lineage;
pub mod log;
pub mod query;
pub mod reauth;
pub mod ref_index;
pub mod report;
pub mod timeline;
pub mod trace;
pub mod violation;

// ── Backward-compatibility re-exports ────────────────────────────
//
// This facade is kept for callers that use `crate::audit::*` paths.
// All types and functions are defined in their respective sub-modules
// below; the re-exports make them available at `crate::audit::name`.
// These re-exports should NOT be removed in this pass — many tests and
// runtime call sites depend on them.

// Re-export document_line public API at crate::audit level for backward compatibility
pub use document_line::{
    active_run_id, analyze_impact, append_line_event, build_document_line,
    build_document_line_for_doc, list_document_line_runs, DocumentLineRun, EvidenceRef,
    ImpactAnalysis, ImpactNode, LineEdge, LineNode,
};

// Re-export log types and functions
pub use log::{append, read_all, verify_audit_trail, AuditEntry, BrokenLink, VerificationReport};

// Re-export ref_index types and functions
pub use ref_index::{build_ref_index, check_immutability, sync_ref_index, RefIndex, RefIndexEntry};

// Re-export diff
pub use diff::save_diff;

// Re-export timeline
pub use timeline::{build_timeline, TimelineData, TimelineSummary};

// Re-export checklist
pub use checklist::{
    init_checklist, load_checklist, save_checklist, update_checklist_item, Checklist, ChecklistItem,
};

// Re-export violation
pub use violation::{add_violation, load_violations, update_violation_status, Violation};

// Re-export reauth
pub use reauth::{consume_reauth_request, request_reauth};

// Re-export trace
pub use trace::{trace_document, ChainNode, TraceResult};

// Re-export report
pub use report::generate_report;

// Re-export lineage
pub use lineage::{build_lineage, LineageNode};

// Re-export query
pub use query::{query_documents, DocQuery, DocSummary};
