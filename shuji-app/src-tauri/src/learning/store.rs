use std::path::{Path, PathBuf};

use super::config::LearningConfig;
use super::entry::{LearningEntry, LearningKind, LearningScope};
use super::role::normalize_role_name;

pub const MAX_ENTRY_CHARS: usize = 500;
pub const MAX_INJECTED_CHARS: usize = 4000;
pub const GLOBAL_INJECT_BUDGET: usize = 1200;
pub const PROJECT_INJECT_BUDGET: usize = 2800;
pub const MAX_SOUL_FILE_BYTES: usize = 8 * 1024;

const GENERIC_SOUL_TEMPLATE: &str = r#"## Experience

## Lessons

## Preferences
"#;

pub struct SoulStore;

impl SoulStore {
    pub fn config() -> LearningConfig {
        super::config::load_config()
    }

    pub fn project_soul_dir(working_dir: &Path) -> PathBuf {
        working_dir.join(".shuji").join("soul")
    }

    pub fn project_soul_path(working_dir: &Path, role_name: &str) -> PathBuf {
        Self::project_soul_dir(working_dir).join(format!("{role_name}.md"))
    }

    pub fn project_index_path(working_dir: &Path) -> PathBuf {
        Self::project_soul_dir(working_dir).join("index.jsonl")
    }

    pub fn legacy_neige_paths(working_dir: &Path) -> Vec<PathBuf> {
        vec![
            Self::project_soul_dir(working_dir).join("neige.md"),
            working_dir.join(".shuji").join("soul.md"),
        ]
    }

    pub fn global_soul_dir() -> Option<PathBuf> {
        home_dir().map(|h| h.join(".shuji").join("soul"))
    }

    pub fn global_soul_path(role_name: &str) -> Option<PathBuf> {
        Self::global_soul_dir().map(|d| d.join(format!("{role_name}.md")))
    }

    pub fn pending_global_path() -> Option<PathBuf> {
        Self::global_soul_dir().map(|d| d.join("pending_global.jsonl"))
    }

    pub fn global_index_path() -> Option<PathBuf> {
        Self::global_soul_dir().map(|d| d.join("index.jsonl"))
    }

    /// Load soul markdown for prompt injection (global first, separate budgets).
    pub async fn load_for_injection(
        working_dir: &Path,
        role_name: &str,
        global_enabled: bool,
    ) -> Result<String, String> {
        let role_name = normalize_role_name(Some(role_name))?;
        let cfg = Self::config();
        if !cfg.project_enabled && !global_enabled {
            return Ok(String::new());
        }

        let mut parts = Vec::new();

        if global_enabled {
            if let Some(global) = Self::load_global_markdown(&role_name).await {
                if !global.trim().is_empty() {
                    let body = truncate_with_label(&global, GLOBAL_INJECT_BUDGET, "global");
                    parts.push(format!("---\n[global]\n{body}"));
                }
            }
        }

        if cfg.project_enabled {
            let project = Self::load_project_markdown(working_dir, &role_name).await;
            if !project.trim().is_empty() {
                let body = truncate_with_label(&project, PROJECT_INJECT_BUDGET, "project");
                parts.push(body);
            }
        }

        Ok(parts.join("\n\n"))
    }

    pub async fn load_project_markdown(working_dir: &Path, role_name: &str) -> String {
        let role_name = match normalize_role_name(Some(role_name)) {
            Ok(r) => r,
            Err(_) => return String::new(),
        };
        let canonical = Self::project_soul_path(working_dir, &role_name);
        if let Ok(content) = tokio::fs::read_to_string(&canonical).await {
            if !content.trim().is_empty() {
                return content;
            }
        }

        if role_name == "Neige" {
            for legacy in Self::legacy_neige_paths(working_dir) {
                if let Ok(content) = tokio::fs::read_to_string(&legacy).await {
                    if !content.trim().is_empty() {
                        Self::migrate_legacy_neige(working_dir, &content).await;
                        return content;
                    }
                }
            }
        }

        Self::bootstrap_project_soul(working_dir, &role_name).await
    }

    async fn bootstrap_project_soul(working_dir: &Path, role_name: &str) -> String {
        let dir = Self::project_soul_dir(working_dir);
        let _ = tokio::fs::create_dir_all(&dir).await;
        let path = Self::project_soul_path(working_dir, role_name);
        let default = if role_name == "Neige" {
            include_str!("../agent/neige/soul.md").to_string()
        } else {
            GENERIC_SOUL_TEMPLATE.to_string()
        };
        let _ = tokio::fs::write(&path, &default).await;
        default
    }

    async fn migrate_legacy_neige(working_dir: &Path, content: &str) {
        let canonical = Self::project_soul_path(working_dir, "Neige");
        if tokio::fs::metadata(&canonical).await.is_err() {
            let dir = Self::project_soul_dir(working_dir);
            let _ = tokio::fs::create_dir_all(&dir).await;
            let _ = tokio::fs::write(&canonical, content).await;
            // Record migration in audit log for traceability
            crate::audit::append(
                working_dir,
                "soul_migration",
                "learning",
                "Neige",
                "migrated legacy neige soul path → Neige.md",
            )
            .await;
            log_console!("[learning] migrated legacy neige soul → Neige.md");
        }
    }

    pub async fn load_global_markdown(role_name: &str) -> Option<String> {
        let path = Self::global_soul_path(role_name)?;
        tokio::fs::read_to_string(&path).await.ok()
    }

    pub async fn read_project_soul(working_dir: &Path, role_name: &str) -> String {
        Self::load_project_markdown(working_dir, role_name).await
    }

    pub async fn clear_project_soul(working_dir: &Path, role_name: &str) -> Result<(), String> {
        let dir = Self::project_soul_dir(working_dir);
        let _ = tokio::fs::create_dir_all(&dir).await;
        let path = Self::project_soul_path(working_dir, role_name);
        let default = if role_name == "Neige" {
            include_str!("../agent/neige/soul.md")
        } else {
            GENERIC_SOUL_TEMPLATE
        };
        tokio::fs::write(&path, default)
            .await
            .map_err(|e| format!("Failed to reset soul: {e}"))
    }

    pub fn list_soul_roles() -> Vec<String> {
        use crate::models::role::Role;
        [
            Role::Neige,
            Role::Zhongshuling,
            Role::MenxiaShizhong,
            Role::Shangshuling,
            Role::LiBuShangshu,
            Role::BingbuShangshu,
            Role::GongbuShangshu,
            Role::XingbuShangshu,
            Role::LiBuRShangshu,
        ]
        .iter()
        .map(|r| r.name().to_string())
        .collect()
    }

    pub async fn append_entry(
        working_dir: &Path,
        role_name: &str,
        kind: LearningKind,
        scope: LearningScope,
        content: &str,
        evidence: Vec<String>,
        tags: Vec<String>,
    ) -> Result<String, String> {
        let role_name = normalize_role_name(Some(role_name))?;
        if content.is_empty() {
            return Err("content cannot be empty".into());
        }
        if content.len() > MAX_ENTRY_CHARS {
            return Err(format!("Content too long (max {MAX_ENTRY_CHARS} chars)"));
        }

        let cfg = Self::config();
        if cfg.global_requires_approval && scope == LearningScope::Global {
            return Err("Direct global writes are not allowed; use global_candidate".into());
        }

        match scope {
            LearningScope::Global => {
                Err("Direct global writes are not allowed; use global_candidate".into())
            }
            LearningScope::GlobalCandidate => {
                Self::append_global_candidate(&role_name, kind, content, evidence, tags).await
            }
            LearningScope::Project => {
                Self::append_project_entry(working_dir, &role_name, kind, content, evidence, tags)
                    .await
            }
        }
    }

    async fn append_project_entry(
        working_dir: &Path,
        role_name: &str,
        kind: LearningKind,
        content: &str,
        evidence: Vec<String>,
        tags: Vec<String>,
    ) -> Result<String, String> {
        let entry = LearningEntry::new(
            role_name,
            LearningScope::Project,
            kind,
            content,
            evidence,
            tags,
        );

        if let Some(updated) = Self::find_duplicate_in_project_index(working_dir, &entry).await {
            Self::update_project_index_entry(working_dir, &updated).await?;
            return Ok(format!(
                "Duplicate entry refreshed (confidence={:.2})",
                updated.confidence
            ));
        }

        let dir = Self::project_soul_dir(working_dir);
        let _ = tokio::fs::create_dir_all(&dir).await;
        let soul_path = Self::project_soul_path(working_dir, role_name);

        let existing = if tokio::fs::metadata(&soul_path).await.is_ok() {
            tokio::fs::read_to_string(&soul_path)
                .await
                .unwrap_or_default()
        } else if role_name == "Neige" {
            Self::load_project_markdown(working_dir, role_name).await
        } else {
            Self::bootstrap_project_soul(working_dir, role_name).await
        };

        if markdown_contains_entry(&existing, content) {
            Self::append_index_line(working_dir, &entry).await?;
            return Ok("Duplicate entry already present in soul markdown".into());
        }

        let entry_line = format!("- {content}\n");
        let heading = kind.markdown_heading();
        let updated_md = insert_under_heading(&existing, heading, &entry_line);
        atomic_write(&soul_path, &updated_md).await?;
        Self::append_index_line(working_dir, &entry).await?;

        crate::audit::append(
            working_dir,
            "update_soul",
            role_name,
            &entry.id,
            &format!("kind={kind:?}"),
        )
        .await;

        if updated_md.len() > MAX_SOUL_FILE_BYTES {
            log_console!(
                "[learning] soul for {} exceeds {} bytes ({} bytes)",
                role_name,
                MAX_SOUL_FILE_BYTES,
                updated_md.len()
            );
        }

        Ok(format!(
            "Recorded under {}",
            heading.trim_start_matches("# ")
        ))
    }

    async fn append_global_candidate(
        role_name: &str,
        kind: LearningKind,
        content: &str,
        evidence: Vec<String>,
        tags: Vec<String>,
    ) -> Result<String, String> {
        if evidence.is_empty() {
            return Err("global_candidate requires evidence".into());
        }
        let entry = LearningEntry::new(
            role_name,
            LearningScope::GlobalCandidate,
            kind,
            content,
            evidence,
            tags,
        );
        let path = Self::pending_global_path()
            .ok_or_else(|| "Cannot resolve home directory for global learning".to_string())?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create global soul dir: {e}"))?;
        }
        append_jsonl(&path, &entry).await?;
        Ok(format!(
            "Global learning candidate queued (id={})",
            entry.id
        ))
    }

    async fn find_duplicate_in_project_index(
        working_dir: &Path,
        entry: &LearningEntry,
    ) -> Option<LearningEntry> {
        let index_path = Self::project_index_path(working_dir);
        let lines = read_jsonl_lines(&index_path).await;
        for line in &lines {
            if let Ok(existing) = serde_json::from_str::<LearningEntry>(line) {
                if existing.role == entry.role
                    && existing.kind == entry.kind
                    && normalize_content(&existing.content) == normalize_content(&entry.content)
                {
                    let mut updated = existing;
                    updated.last_seen = chrono::Utc::now().to_rfc3339();
                    updated.confidence = (updated.confidence + 0.05).min(1.0);
                    return Some(updated);
                }
            }
        }
        None
    }

    async fn update_project_index_entry(
        working_dir: &Path,
        updated: &LearningEntry,
    ) -> Result<(), String> {
        let index_path = Self::project_index_path(working_dir);
        let lines = read_jsonl_lines(&index_path).await;
        Self::rewrite_index(working_dir, &lines, updated, "").await
    }

    async fn find_duplicate_in_global_index(entry: &LearningEntry) -> Option<LearningEntry> {
        let index_path = Self::global_index_path()?;
        let lines = read_jsonl_lines(&index_path).await;
        for line in &lines {
            if let Ok(existing) = serde_json::from_str::<LearningEntry>(line) {
                if existing.role == entry.role
                    && existing.kind == entry.kind
                    && normalize_content(&existing.content) == normalize_content(&entry.content)
                {
                    let mut updated = existing;
                    updated.last_seen = chrono::Utc::now().to_rfc3339();
                    updated.confidence = (updated.confidence + 0.05).min(1.0);
                    return Some(updated);
                }
            }
        }
        None
    }

    async fn global_markdown_contains(role_name: &str, content: &str) -> bool {
        let Some(md) = Self::load_global_markdown(role_name).await else {
            return false;
        };
        markdown_contains_entry(&md, content)
    }

    async fn rewrite_index(
        working_dir: &Path,
        lines: &[String],
        updated: &LearningEntry,
        skip_id: &str,
    ) -> Result<(), String> {
        let index_path = Self::project_index_path(working_dir);
        let mut out = Vec::new();
        let mut replaced = false;
        for line in lines {
            if let Ok(entry) = serde_json::from_str::<LearningEntry>(line) {
                if !skip_id.is_empty() && entry.id == skip_id {
                    continue;
                }
                if entry.id == updated.id {
                    out.push(serde_json::to_string(updated).map_err(|e| e.to_string())?);
                    replaced = true;
                    continue;
                }
            }
            out.push(line.clone());
        }
        if !replaced {
            out.push(serde_json::to_string(updated).map_err(|e| e.to_string())?);
        }
        let body = out.join("\n");
        let body = if body.is_empty() {
            body
        } else {
            format!("{body}\n")
        };
        atomic_write(&index_path, &body).await
    }

    async fn append_index_line(working_dir: &Path, entry: &LearningEntry) -> Result<(), String> {
        let index_path = Self::project_index_path(working_dir);
        append_jsonl(&index_path, entry).await
    }

    pub async fn list_global_candidates() -> Result<Vec<LearningEntry>, String> {
        let path = Self::pending_global_path()
            .ok_or_else(|| "Cannot resolve home directory".to_string())?;
        let lines = read_jsonl_lines(&path).await;
        let mut entries = Vec::new();
        for line in lines {
            if let Ok(entry) = serde_json::from_str::<LearningEntry>(&line) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    pub async fn approve_global_candidate(candidate_id: &str) -> Result<(), String> {
        let pending_path = Self::pending_global_path()
            .ok_or_else(|| "Cannot resolve home directory".to_string())?;
        let lines = read_jsonl_lines(&pending_path).await;
        let mut candidate: Option<LearningEntry> = None;
        let mut remaining = Vec::new();
        for line in lines {
            if let Ok(entry) = serde_json::from_str::<LearningEntry>(&line) {
                if entry.id == candidate_id {
                    candidate = Some(entry);
                } else {
                    remaining.push(line);
                }
            }
        }
        let mut entry = candidate.ok_or_else(|| format!("Candidate not found: {candidate_id}"))?;
        entry.scope = LearningScope::Global;
        let role_name = normalize_role_name(Some(&entry.role))?;
        entry.role = role_name.clone();

        if Self::find_duplicate_in_global_index(&entry).await.is_some()
            || Self::global_markdown_contains(&role_name, &entry.content).await
        {
            let body = remaining.join("\n");
            let body = if body.is_empty() {
                body
            } else {
                format!("{body}\n")
            };
            atomic_write(&pending_path, &body).await?;
            return Ok(());
        }

        let soul_path = Self::global_soul_path(&role_name)
            .ok_or_else(|| "Cannot resolve home directory".to_string())?;
        if let Some(parent) = soul_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        let mut existing = tokio::fs::read_to_string(&soul_path)
            .await
            .unwrap_or_else(|_| {
                if role_name == "Neige" {
                    include_str!("../agent/neige/soul.md").to_string()
                } else {
                    GENERIC_SOUL_TEMPLATE.to_string()
                }
            });
        let heading = entry.kind.markdown_heading();
        existing = insert_under_heading(&existing, heading, &entry.markdown_line());
        atomic_write(&soul_path, &existing).await?;

        if let Some(index_path) = Self::global_index_path() {
            append_jsonl(&index_path, &entry).await?;
        }

        let body = remaining.join("\n");
        let body = if body.is_empty() {
            body
        } else {
            format!("{body}\n")
        };
        atomic_write(&pending_path, &body).await?;
        Ok(())
    }

    pub async fn reject_global_candidate(candidate_id: &str) -> Result<(), String> {
        let pending_path = Self::pending_global_path()
            .ok_or_else(|| "Cannot resolve home directory".to_string())?;
        let lines = read_jsonl_lines(&pending_path).await;
        let mut found = false;
        let mut remaining = Vec::new();
        for line in lines {
            if let Ok(entry) = serde_json::from_str::<LearningEntry>(&line) {
                if entry.id == candidate_id {
                    found = true;
                    continue;
                }
            }
            remaining.push(line);
        }
        if !found {
            return Err(format!("Candidate not found: {candidate_id}"));
        }
        let body = remaining.join("\n");
        let body = if body.is_empty() {
            body
        } else {
            format!("{body}\n")
        };
        atomic_write(&pending_path, &body).await
    }

    pub async fn compact_project_soul_with_llm(
        working_dir: &Path,
        role_name: &str,
        client: &crate::api::client::AnthropicClient,
        model: &str,
    ) -> Result<String, String> {
        let soul_path = Self::project_soul_path(working_dir, role_name);
        let content = tokio::fs::read_to_string(&soul_path)
            .await
            .map_err(|e| format!("Failed to read soul: {e}"))?;

        let prompt = format!(
            r#"You are a soul compaction tool. Distill the role soul into a concise version.

Requirements:
- Keep sections: ## Experience / ## Lessons / ## Preferences (and others if present)
- No more than 5 entries per section
- Each entry prefixed with `- `
- Total characters not exceeding 4000

Original soul:
{content}"#
        );

        let msg = crate::models::message::Message::user(&prompt);
        let compacted = client
            .send_message_with_reasoning(
                "Output compact Markdown soul only.",
                &[msg],
                model,
                crate::config::ResolvedReasoningPolicy {
                    enabled: true,
                    effort: crate::config::ReasoningEffort::Low,
                    budget_tokens: 0,
                },
            )
            .await
            .map_err(|e| format!("LLM compaction failed: {e}"))?
            .trim()
            .to_string();

        if compacted.is_empty() || compacted.len() >= content.len() {
            return Err("Compaction result invalid".into());
        }

        atomic_write(&soul_path, &compacted).await?;
        Ok(format!(
            "soul auto-compacted ({} -> {} bytes)",
            content.len(),
            compacted.len()
        ))
    }
}

fn home_dir() -> Option<PathBuf> {
    super::config::home_dir()
}

fn normalize_content(s: &str) -> String {
    s.trim().to_lowercase()
}

fn markdown_contains_entry(markdown: &str, content: &str) -> bool {
    markdown.contains(&format!("- {}", content.trim()))
}

fn truncate_with_label(s: &str, max: usize, label: &str) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    log_console!(
        "[learning] soul injection truncated {label}: {} -> {} chars",
        s.len(),
        max
    );
    s.chars().take(max).collect()
}

fn insert_under_heading(existing: &str, heading: &str, entry_line: &str) -> String {
    if let Some(pos) = existing.find(heading) {
        let after_heading = &existing[pos + heading.len()..];
        let next_heading = after_heading.find("\n## ");
        let insert_pos = pos + heading.len() + next_heading.unwrap_or(after_heading.len());
        let mut new_content = existing[..insert_pos].to_string();
        if !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        if !new_content.ends_with("\n\n") {
            new_content.push('\n');
        }
        new_content.push_str(entry_line);
        new_content.push_str(&existing[insert_pos..]);
        new_content
    } else {
        format!("{existing}\n\n{heading}\n\n{entry_line}")
    }
}

async fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, content)
        .await
        .map_err(|e| e.to_string())?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| e.to_string())
}

async fn append_jsonl(path: &Path, entry: &LearningEntry) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
        .map_err(|e| e.to_string())?;
    file.write_all(format!("{line}\n").as_bytes())
        .await
        .map_err(|e| e.to_string())
}

async fn read_jsonl_lines(path: &Path) -> Vec<String> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}
