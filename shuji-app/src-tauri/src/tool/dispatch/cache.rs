//! Cache invalidation helpers for tool dispatch.
//! These are called from dispatch/core.rs via super::cache::*.

/// Resolve a document ID from tool args or success JSON result.
pub fn doc_id_from_write_result(
    name: &str,
    args: &serde_json::Value,
    raw_result: &str,
) -> Option<String> {
    if name != "create_document" {
        if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw_result) {
        if v.get("ok").and_then(|o| o.as_bool()) == Some(true) {
            if let Some(path) = v.get("path").and_then(|p| p.as_str()) {
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }
    }
    None
}
