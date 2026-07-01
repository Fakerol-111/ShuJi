use std::path::Path;

use super::log;

/// Generate a delivery report as markdown text.
pub async fn generate_report(working_dir: &Path) -> String {
    let entries = log::read_all(working_dir).await;
    if entries.is_empty() {
        return "## Delivery Report\n\nNo audit records yet.\n".to_string();
    }

    use std::collections::HashMap;
    let mut by_event: HashMap<String, usize> = HashMap::new();
    let mut by_role: HashMap<String, usize> = HashMap::new();
    let mut docs_created: Vec<&log::AuditEntry> = Vec::new();

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

    if let Some(line) = super::document_line::build_document_line(working_dir, None).await {
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
