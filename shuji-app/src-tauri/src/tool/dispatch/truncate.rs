//! Tool result truncation — limits verbose tool outputs to avoid overflowing context.
//! Extracted from dispatch.rs.

/// Truncate verbose tool results to avoid blowing up context.
/// Uses per-tool limits. Truncated results include a hint for continuation.
pub fn truncate_tool_result_by_name(name: &str, content: &str) -> String {
    let limit = match name {
        "read_file" | "read_document" => 8000,
        "search_text" => 8000,
        "list_dir" => 8000,
        "execute_command" => 4000,
        "run_tests" => 4000,
        "run_lint" => 4000,
        "summarize_logs" => 4000,
        _ => 16000,
    };
    if content.len() > limit {
        let head: String = content.chars().take(limit).collect();
        format!(
            "{}...\n[Truncated: showing first {} of {} chars. To continue, narrow your search and retry (truncated: true)]",
            head,
            limit,
            content.len()
        )
    } else {
        content.to_string()
    }
}
