//! Pipeline artifact handling: extract doc IDs from agent output, collect from upstream steps.

use std::collections::HashMap;

use crate::models::role::Role;

/// Collect document IDs produced by upstream steps (from `depends_on` → `artifacts`).
pub fn collect_upstream_doc_ids(
    artifacts: &HashMap<String, String>,
    depends_on: &[String],
) -> Vec<String> {
    let mut ids = Vec::new();
    for dep in depends_on {
        if let Some(id) = artifacts.get(dep) {
            if !ids.iter().any(|existing| existing == id) {
                ids.push(id.clone());
            }
        }
    }
    ids
}

/// Pick the document ID for an approval_gate step (revw only).
pub fn approval_doc_from_upstream(
    artifacts: &HashMap<String, String>,
    depends_on: &[String],
) -> Option<String> {
    collect_upstream_doc_ids(artifacts, depends_on)
        .into_iter()
        .find(|id| id.starts_with("revw_"))
}

/// Extract the primary document artifact from department output text.
pub fn extract_artifact_from_output(output: &str, target_dept: &str) -> Option<String> {
    let ids = crate::agent::util::find_all_doc_ids_in_text(output);
    let preferred = artifact_prefixes_for_dept(target_dept);
    pick_by_prefix(&ids, preferred).or_else(|| ids.last().cloned())
}

fn pick_by_prefix(ids: &[String], prefixes: &[&str]) -> Option<String> {
    for prefix in prefixes {
        if let Some(id) = ids.iter().find(|id| id.starts_with(&format!("{prefix}_"))) {
            return Some(id.clone());
        }
    }
    None
}

fn artifact_prefixes_for_dept(target: &str) -> &'static [&'static str] {
    match Role::from_name(target) {
        Some(Role::Zhongshuling) => &["dsgn", "plan", "pdsg", "ddtl", "anls", "reqs"],
        Some(Role::MenxiaShizhong) => &["revw"],
        Some(Role::LiBuShangshu) => &["pdsg", "ddtl"],
        Some(Role::BingbuShangshu) => &["ctrt"],
        Some(Role::GongbuShangshu) | Some(Role::XingbuShangshu) => &["task", "rprt"],
        Some(Role::LiBuRShangshu) => &["rprt"],
        Some(Role::Shangshuling) => &["task", "plan"],
        _ => &[],
    }
}

pub fn looks_like_doc_id(s: &str) -> bool {
    let s = s.trim().strip_suffix(".md").unwrap_or(s.trim());
    let Some((prefix, suffix)) = s.split_once('_') else {
        return false;
    };
    !prefix.is_empty()
        && prefix.chars().all(|c| c.is_ascii_lowercase())
        && !suffix.is_empty()
        && suffix.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Split legacy route_to: subject field carries doc id, optional inline carries task text.
pub fn split_route_task_and_doc_ids(subject: &str, inline: Option<&str>) -> (String, Vec<String>) {
    if looks_like_doc_id(subject) {
        let task = inline
            .filter(|s| !s.is_empty())
            .unwrap_or("请完成本部门职责范围内的相关工作")
            .to_string();
        (task, vec![subject.to_string()])
    } else {
        (subject.to_string(), Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_upstream_from_depends_on() {
        let mut artifacts = HashMap::new();
        artifacts.insert("design".into(), "dsgn_3".into());
        let ids = collect_upstream_doc_ids(&artifacts, &["design".into()]);
        assert_eq!(ids, vec!["dsgn_3".to_string()]);
    }

    #[test]
    fn approval_requires_revw() {
        let mut artifacts = HashMap::new();
        artifacts.insert("design".into(), "dsgn_3".into());
        artifacts.insert("review".into(), "revw_2".into());
        assert_eq!(
            approval_doc_from_upstream(&artifacts, &["design".into(), "review".into()]).as_deref(),
            Some("revw_2")
        );
    }

    #[test]
    fn approval_ignores_plan_without_revw() {
        let mut artifacts = HashMap::new();
        artifacts.insert("design".into(), "plan_1".into());
        assert_eq!(
            approval_doc_from_upstream(&artifacts, &["design".into()]).as_deref(),
            None
        );
    }

    #[test]
    fn split_route_subject_as_doc_id() {
        let (task, ids) = split_route_task_and_doc_ids("dsgn_5", None);
        assert!(!task.contains("dsgn_5"));
        assert_eq!(ids, vec!["dsgn_5".to_string()]);
    }

    #[test]
    fn extract_zhongshuling_artifact() {
        let out = "Design complete.\nReview Basis: dsgn_5\n";
        assert_eq!(
            extract_artifact_from_output(out, "中书令").as_deref(),
            Some("dsgn_5")
        );
    }
}
