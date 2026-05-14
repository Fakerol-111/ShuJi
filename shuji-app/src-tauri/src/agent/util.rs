/// Extract the first `<skill>xxx</skill>` tag from text.
pub fn extract_skill(text: &str) -> Option<String> {
    let start = text.find("<skill>")?;
    let after = &text[start + 7..];
    let end = after.find("</skill>")?;
    if end > 50 { return None; }
    Some(after[..end].to_string())
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
