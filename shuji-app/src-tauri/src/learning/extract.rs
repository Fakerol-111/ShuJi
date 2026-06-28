use std::path::Path;

use super::config::load_config;
use super::entry::{LearningKind, LearningScope};
use super::store::{SoulStore, MAX_ENTRY_CHARS};

const MAX_EXTRACT_PER_RUN: usize = 5;

/// Conservative post-run learning extractor.
/// Only records evidence-backed items when explicitly called.
pub struct LearningExtractor;

impl LearningExtractor {
    pub async fn from_user_decision(
        working_dir: &Path,
        role_name: &str,
        approved: bool,
        note: &str,
        doc_id: &str,
    ) -> Result<(), String> {
        if note.trim().is_empty() {
            return Ok(());
        }
        let kind = if approved {
            LearningKind::Preference
        } else {
            LearningKind::Lesson
        };
        let content = truncate_content(note);
        SoulStore::append_entry(
            working_dir,
            role_name,
            kind,
            LearningScope::Project,
            &content,
            vec![format!("decision:{doc_id}")],
            vec![],
        )
        .await?;
        Ok(())
    }

    /// Extract conservative learnings after a pipeline completes successfully.
    pub async fn from_pipeline_complete(working_dir: &Path) -> Result<usize, String> {
        let cfg = load_config();
        if !cfg.auto_extract || !cfg.project_enabled {
            return Ok(0);
        }

        let mut recorded = 0usize;

        recorded += Self::extract_validation_failures(working_dir).await?;
        if recorded >= MAX_EXTRACT_PER_RUN {
            return Ok(recorded);
        }

        recorded +=
            Self::extract_fixed_violations(working_dir, MAX_EXTRACT_PER_RUN - recorded).await?;

        Ok(recorded)
    }

    async fn extract_validation_failures(working_dir: &Path) -> Result<usize, String> {
        let path = working_dir
            .join(".shuji")
            .join("validate")
            .join("latest.json");
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => return Ok(0),
        };
        let report: crate::validate::report::ValidationReport = match serde_json::from_str(&content)
        {
            Ok(r) => r,
            Err(_) => return Ok(0),
        };
        if report.overall_pass {
            return Ok(0);
        }

        let mut count = 0usize;
        for check in &report.checks {
            if check.pass || check.summary.trim().is_empty() {
                continue;
            }
            let raw = format!("[{}] {}", check.name, check.summary.trim());
            let content = truncate_content(&raw);
            SoulStore::append_entry(
                working_dir,
                "Xingbushangshu",
                LearningKind::Lesson,
                LearningScope::Project,
                &content,
                vec![format!("validate:latest:{}", check.name)],
                vec![],
            )
            .await?;
            count += 1;
            if count >= MAX_EXTRACT_PER_RUN {
                break;
            }
        }
        Ok(count)
    }

    async fn extract_fixed_violations(working_dir: &Path, limit: usize) -> Result<usize, String> {
        if limit == 0 {
            return Ok(0);
        }
        let violations = crate::audit::load_violations(working_dir).await;
        let mut count = 0usize;
        for v in violations {
            if v.status != "fixed" || v.description.trim().is_empty() {
                continue;
            }
            let raw = format!("[{}] {}", v.rule_id, v.description.trim());
            let content = truncate_content(&raw);
            SoulStore::append_entry(
                working_dir,
                "Liburshangshu",
                LearningKind::ReviewRule,
                LearningScope::Project,
                &content,
                vec![format!("violation:{}", v.rule_id)],
                vec![],
            )
            .await?;
            count += 1;
            if count >= limit {
                break;
            }
        }
        Ok(count)
    }
}

fn truncate_content(s: &str) -> String {
    s.chars().take(MAX_ENTRY_CHARS).collect()
}
