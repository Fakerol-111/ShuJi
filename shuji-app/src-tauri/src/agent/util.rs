use crate::models::role::Role;

/// Extract the first `<skill>xxx</skill>` tag from text.
/// Only accepts alphanumeric + underscore names, max 50 chars.
pub fn extract_skill(text: &str) -> Option<String> {
    let start = text.find("<skill>")?;
    let after = &text[start + 7..];
    let end = after.find("</skill>")?;
    let name = &after[..end];
    // Validate: non-empty, ≤50 chars, alphanumeric + underscore only
    if name.is_empty() || end > 50 {
        return None;
    }
    if name
        .chars()
        .any(|c| !c.is_alphanumeric() && c != '_' && c != '-')
    {
        return None;
    }
    Some(name.to_string())
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
            let subject = find_doc_id(text).unwrap_or_default();
            return Some((role, subject));
        }
    }
    None
}

/// Find a document ID pattern like "task_5" or "dsgn_003" in text.
fn find_doc_id(text: &str) -> Option<String> {
    for word in text.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| c == '"' || c == '\'' || c == '>' || c == '/');
        if let Some(pos) = trimmed.find('_') {
            let prefix = &trimmed[..pos];
            let suffix = &trimmed[pos + 1..];
            if !prefix.is_empty()
                && prefix.chars().all(|c| c.is_ascii_lowercase())
                && suffix.chars().all(|c| c.is_ascii_digit())
            {
                return Some(trimmed.to_string());
            }
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
        assert_eq!(find_doc_id("请处理 task_5"), Some("task_5".into()));
        assert_eq!(
            find_doc_id("文档 dsgn_003 需要审查"),
            Some("dsgn_003".into())
        );
        assert_eq!(find_doc_id("no doc id here"), None);
    }
}
