//! Document read and find operations.

use std::path::Path;

use crate::tool::{resolve_scoped_path, ToolOutput};

use super::parse::{
    find_rprt_path, list_doc_ids_in_subdir, normalize_doc_id, parse_doc, resolve_doc_path,
    type_to_dir,
};

/// Build a helpful hint when read_document fails (list nearby valid IDs).
async fn doc_not_found_hint(working_dir: &Path, raw_id: &str, normalized_id: &str) -> String {
    let mut parts = vec![
        "read_document expects a document ID like dsgn_3 or plan_1 — not a filename path or context JSON."
            .to_string(),
    ];
    if raw_id != normalized_id {
        parts.push(format!(
            "Normalized \"{raw_id}\" → \"{normalized_id}\" but still not found."
        ));
    }
    if raw_id.ends_with(".md") || raw_id.contains('/') || raw_id.contains('\\') {
        parts.push("Do not pass file paths or .md extensions; use the ID only.".to_string());
    }
    let mut samples = list_doc_ids_in_subdir(working_dir, "designs", 8).await;
    if samples.is_empty() {
        samples = list_doc_ids_in_subdir(working_dir, "requirements", 5).await;
    }
    if !samples.is_empty() {
        parts.push(format!(
            "Available IDs in .shuji/designs: {}",
            samples.join(", ")
        ));
    } else {
        parts.push(
            "No documents in .shuji/designs yet — list_dir .shuji/designs first.".to_string(),
        );
    }
    parts.join(" ")
}

/// ── read_document ─────────────────────────────────────────────────
pub async fn tool_read_document(working_dir: &Path, args: &serde_json::Value) -> String {
    let raw_id = args["id"].as_str().unwrap_or("");
    if raw_id.is_empty() {
        return ToolOutput::error(
            "read_document",
            "",
            "empty_id",
            "Document ID cannot be empty. Use list_dir on .shuji/designs to find IDs like dsgn_3 (no .md suffix).",
        );
    }

    let id = normalize_doc_id(working_dir, raw_id).await;
    let full = match resolve_doc_path(working_dir, &id).await {
        Ok(p) => p,
        Err(e) => {
            let hint = doc_not_found_hint(working_dir, raw_id, &id).await;
            return ToolOutput::error(
                "read_document",
                raw_id,
                "not_found",
                &format!("{e}. {hint}"),
            );
        }
    };
    if !full.exists() {
        let hint = doc_not_found_hint(working_dir, raw_id, &id).await;
        return ToolOutput::error(
            "read_document",
            raw_id,
            "not_found",
            &format!("Document {id} does not exist. {hint}"),
        );
    }

    if let Some(cached) = crate::tool::cache_lookup(working_dir, &full) {
        return cached;
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => {
            if let Ok(meta) = tokio::fs::metadata(&full).await {
                if let Ok(mtime) = meta.modified() {
                    crate::tool::cache_insert(working_dir, full.clone(), mtime, c.clone());
                }
            }
            c
        }
        Err(e) => return ToolOutput::error("read_document", &id, "read_error", &e.to_string()),
    };

    let (meta, body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error("read_document", &id, "parse_error", &e),
    };

    let target_section = args["section"].as_str().filter(|s| !s.is_empty());
    let extracted = if let Some(section_name) = target_section {
        match extract_section(body, section_name) {
            Ok(content) => content,
            Err(msg) => {
                return ToolOutput::error("read_document", &id, "section_not_found", &msg);
            }
        }
    } else {
        body.to_string()
    };

    let max_chars = args["max_chars"].as_u64().unwrap_or(4000) as usize;
    let display_body = if max_chars > 0 && extracted.len() > max_chars {
        let cutoff = extracted.floor_char_boundary(max_chars);
        format!(
            "{}...\n\n[Truncated: showing first {} of {} chars]",
            &extracted[..cutoff],
            cutoff,
            extracted.len()
        )
    } else {
        extracted
    };

    let rel_path = full
        .strip_prefix(working_dir)
        .unwrap_or(&full)
        .to_string_lossy();
    let meta_line = format!(
        "{} | type: {} | author: {} | time: {} | status: {} | refs: {}",
        meta.id,
        meta.doc_type,
        meta.author,
        meta.timestamp,
        if meta.status.is_empty() {
            "-"
        } else {
            &meta.status
        },
        meta.refs
    );
    let result = if let Some(ref section) = target_section {
        format!(
            "{}\n--- Section [{}] ---\n{}",
            meta_line, section, display_body
        )
    } else {
        format!("{}\n--- Body ---\n{}", meta_line, display_body)
    };

    ToolOutput::read_file("read_document", &rel_path, &result)
}

/// Extract a section (## or ### heading) from markdown body text.
fn extract_section(body: &str, section_name: &str) -> Result<String, String> {
    let heading = format!("## {}", section_name);
    let heading3 = format!("### {}", section_name);
    let lines: Vec<&str> = body.lines().collect();
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == heading
            || trimmed.starts_with(&heading)
            || trimmed == heading3
            || trimmed.starts_with(&heading3)
        {
            start = Some(i);
        } else if start.is_some()
            && end.is_none()
            && (line.starts_with("## ") || line.starts_with("### "))
        {
            end = Some(i);
            break;
        }
    }

    if let Some(s) = start {
        let e = end.unwrap_or(lines.len());
        Ok(lines[s..e].join("\n"))
    } else {
        let available: Vec<&str> = lines
            .iter()
            .filter(|l| l.starts_with("## ") || l.starts_with("### "))
            .map(|l| l.trim())
            .collect();
        Err(format!(
            "Section \"{}\" not found in the document.\nAvailable sections ({} total):\n{}",
            section_name,
            available.len(),
            available.join("\n")
        ))
    }
}

/// ── find_document ─────────────────────────────────────────────────
pub async fn tool_find_document(working_dir: &Path, args: &serde_json::Value) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error(
            "find_document",
            "",
            "empty_id",
            "Document ID cannot be empty",
        );
    }

    let type_prefix = id.split('_').next().unwrap_or("");

    if type_prefix == "rprt" {
        match find_rprt_path(working_dir, id).await {
            Some(p) => {
                let rel = p.strip_prefix(working_dir).unwrap_or(&p);
                ToolOutput::success("find_document", id, &format!("{}", rel.display()))
            }
            None => ToolOutput::error(
                "find_document",
                id,
                "not_found",
                &format!("Document {} does not exist", id),
            ),
        }
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        match resolve_scoped_path(working_dir, &rel_path).await {
            Ok(full) if full.exists() => {
                let rel = full.strip_prefix(working_dir).unwrap_or(&full);
                ToolOutput::success("find_document", id, &format!("{}", rel.display()))
            }
            Ok(_) => ToolOutput::error(
                "find_document",
                id,
                "not_found",
                &format!("Document {} does not exist", id),
            ),
            Err(e) => ToolOutput::error("find_document", id, "path_error", &e),
        }
    }
}
