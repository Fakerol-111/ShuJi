//! Helper functions for soul store: markdown manipulation, file I/O, and content utilities.
//!
//! Extracted from `store.rs` for separation of concerns.

use std::path::{Path, PathBuf};

use super::entry::LearningEntry;

/// Delegate to config's home_dir to avoid circular dependency.
pub fn home_dir() -> Option<PathBuf> {
    super::config::home_dir()
}

/// Normalize content for comparison (trim + lowercase).
pub fn normalize_content(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Check if a markdown string already contains an entry with the given content.
pub fn markdown_contains_entry(markdown: &str, content: &str) -> bool {
    markdown.contains(&format!("- {}", content.trim()))
}

/// Truncate markdown to a budget, keeping only complete lines (entry boundaries).
/// Prevents cutting entries mid-way. Falls back to raw char truncation if no line
/// boundary is found within the budget.
pub fn truncate_to_entry_boundary(s: &str, max: usize, label: &str) -> String {
    if s.len() <= max || max == 0 {
        return s.chars().take(max).collect();
    }
    log_console!(
        "[learning] soul injection truncated {label}: {} -> {} chars (entry-boundary)",
        s.len(),
        max
    );

    // Walk back to find the last complete line boundary within the budget.
    let mut end = max;
    let chars: Vec<char> = s.chars().collect();
    if end > chars.len() {
        end = chars.len();
    }
    // Ensure we don't cut mid-line: walk back to the previous '\n' if we're
    // in the middle of a line, but only if there's a '\n' before `end`.
    if end > 0 && end < chars.len() && chars[end - 1] != '\n' {
        if let Some(newline_pos) = chars[..end].iter().rposition(|&c| c == '\n') {
            end = newline_pos + 1; // Include the newline
        }
    }

    let result: String = chars[..end].iter().collect();
    if result.is_empty() {
        // Fallback: if entry-boundary truncation produces nothing, use raw prefix
        return s.chars().take(max).collect();
    }
    result
}

/// Insert an entry line under a specific markdown heading.
/// If the heading doesn't exist, it's appended at the end.
pub fn insert_under_heading(existing: &str, heading: &str, entry_line: &str) -> String {
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

/// Atomically write content to a file via temp file + rename.
pub async fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let tmp = {
        let mut name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("tmp")
            .to_string();
        name.push('.');
        name.push_str(&uuid::Uuid::new_v4().to_string());
        name.push_str(".tmp");
        path.with_file_name(name)
    };
    tokio::fs::write(&tmp, content)
        .await
        .map_err(|e| e.to_string())?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| e.to_string())
}

/// Append a learning entry as a JSON line to a JSONL file.
pub async fn append_jsonl(path: &Path, entry: &LearningEntry) -> Result<(), String> {
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

/// Read all non-empty lines from a JSONL file.
pub async fn read_jsonl_lines(path: &Path) -> Vec<String> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}
