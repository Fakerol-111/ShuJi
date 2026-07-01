use std::path::Path;

use serde::Serialize;

use super::log::{self, AuditEntry};

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
    let entries = log::read_all(working_dir).await;

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
