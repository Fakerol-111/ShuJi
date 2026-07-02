//! Completion preview logging for the Session module.
//!
//! Extracted from `Session::step()` in `mod.rs`. Provides a helper to log
//! a short preview of the assistant message content and tool calls.

impl super::Session {
    /// Log a short preview of the assistant's response for debugging.
    /// Shows text preview and/or tool call names.
    pub(super) fn log_completion_preview(&self, msg: &serde_json::Value) {
        let text = msg["content"].as_str().unwrap_or("");
        let has_tools = msg["tool_calls"].as_array().is_some_and(|a| !a.is_empty());
        if has_tools {
            let tool_names: Vec<&str> = msg["tool_calls"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|tc| tc["function"]["name"].as_str())
                        .collect()
                })
                .unwrap_or_default();
            if text.is_empty() {
                log_console!("[{}] → tools: {:?}", self.role(), tool_names);
            } else {
                let preview = Self::preview(text);
                log_console!("[{}] → {} | tools: {:?}", self.role(), preview, tool_names);
            }
        } else if !text.is_empty() {
            log_console!("[{}] → {}", self.role(), Self::preview(text));
        }
    }
}
