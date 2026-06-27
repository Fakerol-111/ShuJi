//! Load document metadata for chat attachment cards.

use std::path::Path;

use crate::models::chat::ChatDocument;

use super::{parse_doc, resolve_doc_path};

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
        format!("{}…", &s[..end])
    }
}

fn title_from_body(body: &str) -> String {
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(h) = t.strip_prefix('#') {
            return truncate(h.trim_start(), 80);
        }
        return truncate(t, 80);
    }
    String::new()
}

/// Resolve a document ID to chat-card metadata (no full body).
pub async fn chat_document_from_id(working_dir: &Path, doc_id: &str) -> Option<ChatDocument> {
    let full = resolve_doc_path(working_dir, doc_id).await.ok()?;
    let content = tokio::fs::read_to_string(&full).await.ok()?;
    let (meta, body) = parse_doc(&content).ok()?;
    let path = full
        .strip_prefix(working_dir)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"));
    Some(ChatDocument {
        id: meta.id,
        doc_type: meta.doc_type,
        title: title_from_body(body),
        status: meta.status,
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_from_body_prefers_heading() {
        assert_eq!(title_from_body("# 总体设计\n\n正文"), "总体设计");
    }

    #[test]
    fn title_from_body_falls_back_to_first_line() {
        assert_eq!(title_from_body("摘要行\n\n更多"), "摘要行");
    }
}
