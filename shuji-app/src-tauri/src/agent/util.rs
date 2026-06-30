use crate::models::role::Role;

/// Tag parsing error with structured context for LLM feedback.
///
/// When LLM output contains a malformed tag, callers can use [`TagParseError::feedback_hint`]
/// to inject a corrective prompt back into the conversation, mirroring the Watchdog
/// intervention pattern. This converts silent format drift into an explicit retry loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagParseError {
    /// `<skill>` opening tag found but no closing `</skill>`.
    SkillMissingClose,
    /// `<skill>` content empty or exceeds 50 chars.
    SkillInvalidContent(String),
    /// `<skill>` content contains chars other than `[A-Za-z0-9_-]`.
    SkillInvalidChars(String),
    /// `<route ...>` tag found but malformed (missing `/>` or required attrs).
    RouteMalformed(String),
}

impl TagParseError {
    /// Human/LLM-facing hint describing the expected format and what went wrong.
    ///
    /// Inject this string into the tool result or user message so the LLM can
    /// self-correct on the next round — same pattern as Watchdog intervention hints.
    pub fn feedback_hint(&self) -> String {
        match self {
            TagParseError::SkillMissingClose => {
                "上一次输出中的 <skill> 标签缺少闭合 </skill>。请用 <skill>name</skill> 格式，name 仅含字母数字与下划线，≤50 字符。".to_string()
            }
            TagParseError::SkillInvalidContent(raw) => {
                format!(
                    "上一次 <skill> 标签内容无效（空或超长）: \"{}\"。name 须非空且 ≤50 字符。",
                    truncate_for_hint(raw, 30)
                )
            }
            TagParseError::SkillInvalidChars(raw) => {
                format!(
                    "上一次 <skill> 标签含非法字符: \"{}\"。仅允许字母数字、下划线与连字符。",
                    truncate_for_hint(raw, 30)
                )
            }
            TagParseError::RouteMalformed(detail) => {
                format!(
                    "上一次 <route> 标签格式错误: {}。正确格式：<route to=\"部门名\" type=\"task\" subject=\"doc_id\" />",
                    detail
                )
            }
        }
    }
}

fn truncate_for_hint(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}…", truncated)
    }
}

/// Result of attempting to extract a skill tag, distinguishing
/// "no tag present" from "tag present but malformed".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillExtractResult {
    /// No `<skill>` tag found in the text — normal case, caller proceeds.
    NoTag,
    /// Tag found and parsed successfully.
    Ok(String),
    /// Tag found but malformed — caller may inject [`TagParseError::feedback_hint`].
    Malformed(TagParseError),
}

/// Attempt to extract the first `<skill>xxx</skill>` tag.
///
/// Returns [`SkillExtractResult`] so callers can distinguish "no tag" (normal)
/// from "malformed tag" (should feed back to LLM). Use this in new code instead of
/// [`extract_skill`] when you want the structured error path.
///
/// Only accepts alphanumeric + underscore + hyphen names, max 50 chars.
pub fn try_extract_skill(text: &str) -> SkillExtractResult {
    let Some(start) = text.find("<skill>") else {
        return SkillExtractResult::NoTag;
    };
    let after = &text[start + 7..];
    let Some(end) = after.find("</skill>") else {
        return SkillExtractResult::Malformed(TagParseError::SkillMissingClose);
    };
    let name = &after[..end];
    if name.is_empty() || end > 50 {
        return SkillExtractResult::Malformed(TagParseError::SkillInvalidContent(name.to_string()));
    }
    if name
        .chars()
        .any(|c| !c.is_alphanumeric() && c != '_' && c != '-')
    {
        return SkillExtractResult::Malformed(TagParseError::SkillInvalidChars(name.to_string()));
    }
    SkillExtractResult::Ok(name.to_string())
}

/// Extract the first `<skill>xxx</skill>` tag from text.
/// Only accepts alphanumeric + underscore names, max 50 chars.
///
/// **Note**: returns `None` for both "no tag" and "malformed tag". For LLM-feedback
/// semantics use [`try_extract_skill`] instead, which distinguishes the two cases.
pub fn extract_skill(text: &str) -> Option<String> {
    match try_extract_skill(text) {
        SkillExtractResult::Ok(name) => Some(name),
        SkillExtractResult::NoTag | SkillExtractResult::Malformed(_) => None,
    }
}

/// Remove `<skill>xxx</skill>` tag from text.
pub fn strip_skill_tag(mut text: String) -> String {
    if let Some(start) = text.find("<skill>") {
        if let Some(end) = text[start..].find("</skill>") {
            text.replace_range(start..start + end + 8, "");
        }
    }
    text.trim().to_string()
}

/// Extract a `<route to="X" type="Y" subject="Z" />` tag from text.
/// Returns (target_role, subject) on success.
/// Falls back to lenient parsing if the strict XML format is not matched.
pub fn extract_route(text: &str) -> Option<(Role, String)> {
    // Strict format: <route to="中书令" type="task" subject="task_5" />
    let strict = extract_route_strict(text);
    if strict.is_some() {
        return strict;
    }
    // Fallback: lenient parsing for common LLM mistakes
    extract_route_lenient(text)
}

/// Strict XML-attribute parser for route tags.
fn extract_route_strict(text: &str) -> Option<(Role, String)> {
    let start = text.find("<route")?;
    let slice = &text[start..];
    let end = slice.find("/>")?;
    let tag = &slice[..end + 2];

    let to = extract_attr(tag, "to")?;
    let role = Role::from_name(&to)?;
    let subject = extract_attr(tag, "subject").unwrap_or_default();
    Some((role, subject))
}

/// Extract an XML attribute value. e.g. extract_attr(tag, "to") → "中书令"
fn extract_attr(tag: &str, name: &str) -> Option<String> {
    let prefix = format!("{}=\"", name);
    let pos = tag.find(&prefix)?;
    let after = &tag[pos + prefix.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

/// Lenient fallback: look for known role names and document IDs in the text.
fn extract_route_lenient(text: &str) -> Option<(Role, String)> {
    let known_roles = [
        "中书令",
        "门下侍中",
        "尚书令",
        "内阁",
        "吏部",
        "兵部",
        "工部",
        "刑部",
        "礼部",
    ];
    for role_name in known_roles {
        if text.contains(role_name) {
            let role = Role::from_name(role_name)?;
            // Try to find a document ID pattern: xxx_NN
            let subject = find_doc_id_in_text(text).unwrap_or_default();
            return Some((role, subject));
        }
    }
    None
}

/// Find a document ID pattern like "task_5" or "dsgn_003" in text.
pub fn find_doc_id_in_text(text: &str) -> Option<String> {
    find_all_doc_ids_in_text(text).into_iter().next()
}

/// Collect all document ID patterns in text (order preserved, deduplicated).
pub fn find_all_doc_ids_in_text(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for word in text.split_whitespace() {
        if let Some(id) = parse_doc_id_token(word) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

fn parse_doc_id_token(word: &str) -> Option<String> {
    let trimmed = word.trim_matches(|c: char| {
        c == '"'
            || c == '\''
            || c == '>'
            || c == '/'
            || c == ','
            || c == '.'
            || c == ':'
            || c == '('
            || c == ')'
    });
    let trimmed = trimmed.strip_suffix(".md").unwrap_or(trimmed);
    if let Some(pos) = trimmed.find('_') {
        let prefix = &trimmed[..pos];
        let suffix = &trimmed[pos + 1..];
        if !prefix.is_empty()
            && prefix.chars().all(|c| c.is_ascii_lowercase())
            && !suffix.is_empty()
            && suffix.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return Some(trimmed.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_skill_valid() {
        assert_eq!(
            extract_skill("some text <skill>workflow_standard</skill> more"),
            Some("workflow_standard".into())
        );
    }

    #[test]
    fn test_extract_skill_rejects_empty() {
        assert_eq!(extract_skill("<skill></skill>"), None);
    }

    #[test]
    fn test_extract_skill_rejects_long() {
        let long = "a".repeat(51);
        assert_eq!(extract_skill(&format!("<skill>{}</skill>", long)), None);
    }

    #[test]
    fn test_extract_skill_rejects_special_chars() {
        assert_eq!(extract_skill("<skill>bad name!</skill>"), None);
    }

    #[test]
    fn test_extract_route_strict() {
        let result = extract_route(r#"<route to="中书令" type="task" subject="dsgn_003" />"#);
        assert!(result.is_some());
        let (role, subject) = result.unwrap();
        assert_eq!(role, Role::Zhongshuling);
        assert_eq!(subject, "dsgn_003");
    }

    #[test]
    fn test_extract_route_lenient() {
        let result = extract_route("下一个需要工部处理 task_7 这个任务");
        assert!(result.is_some());
        let (role, subject) = result.unwrap();
        assert_eq!(role, Role::GongbuShangshu);
        assert_eq!(subject, "task_7");
    }

    #[test]
    fn test_strip_skill_tag() {
        assert_eq!(
            strip_skill_tag("hello <skill>test</skill> world".into()),
            "hello  world"
        );
    }

    #[test]
    fn test_find_doc_id() {
        assert_eq!(find_doc_id_in_text("请处理 task_5"), Some("task_5".into()));
        assert_eq!(
            find_doc_id_in_text("文档 dsgn_003 需要审查"),
            Some("dsgn_003".into())
        );
        assert_eq!(find_doc_id_in_text("no doc id here"), None);
    }

    // ── try_extract_skill structured-error path tests ──────────────

    #[test]
    fn test_try_extract_skill_no_tag() {
        assert_eq!(try_extract_skill("no tag here"), SkillExtractResult::NoTag);
    }

    #[test]
    fn test_try_extract_skill_ok() {
        assert_eq!(
            try_extract_skill("<skill>workflow_standard</skill>"),
            SkillExtractResult::Ok("workflow_standard".into())
        );
    }

    #[test]
    fn test_try_extract_skill_missing_close() {
        assert_eq!(
            try_extract_skill("text <skill>workflow_standard more text"),
            SkillExtractResult::Malformed(TagParseError::SkillMissingClose)
        );
    }

    #[test]
    fn test_try_extract_skill_empty_content() {
        assert_eq!(
            try_extract_skill("<skill></skill>"),
            SkillExtractResult::Malformed(TagParseError::SkillInvalidContent(String::new()))
        );
    }

    #[test]
    fn test_try_extract_skill_too_long() {
        let long = "a".repeat(51);
        assert_eq!(
            try_extract_skill(&format!("<skill>{}</skill>", long)),
            SkillExtractResult::Malformed(TagParseError::SkillInvalidContent(long))
        );
    }

    #[test]
    fn test_try_extract_skill_invalid_chars() {
        assert_eq!(
            try_extract_skill("<skill>bad name!</skill>"),
            SkillExtractResult::Malformed(TagParseError::SkillInvalidChars("bad name!".into()))
        );
    }

    #[test]
    fn test_feedback_hint_non_empty() {
        // Every variant must produce a non-empty, actionable hint
        assert!(!TagParseError::SkillMissingClose.feedback_hint().is_empty());
        assert!(!TagParseError::SkillInvalidContent("x".into())
            .feedback_hint()
            .is_empty());
        assert!(!TagParseError::SkillInvalidChars("x".into())
            .feedback_hint()
            .is_empty());
        assert!(!TagParseError::RouteMalformed("missing to".into())
            .feedback_hint()
            .is_empty());
    }

    #[test]
    fn test_feedback_hint_truncates_long_content() {
        let long = "a".repeat(100);
        let hint = TagParseError::SkillInvalidContent(long.clone()).feedback_hint();
        // Hint should contain the truncation marker, not the full 100-char string
        assert!(hint.contains('…'));
        assert!(!hint.contains(&long));
    }

    #[test]
    fn test_extract_skill_backward_compatible() {
        // Old API returns None for both no-tag and malformed — preserved for compat
        assert_eq!(extract_skill("no tag"), None);
        assert_eq!(
            extract_skill("<skill>workflow_standard</skill>"),
            Some("workflow_standard".into())
        );
        assert_eq!(extract_skill("<skill>bad name!</skill>"), None);
        assert_eq!(extract_skill("<skill>unclosed"), None);
    }
}
