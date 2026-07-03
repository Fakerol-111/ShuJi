//! Decision option extraction — finds the last `request_decision` tool call
//! and returns its options array. Moved from mod.rs.

/// Extract decision options from the last `request_decision` tool call in session messages.
pub(crate) fn extract_decision_options(messages: &[serde_json::Value]) -> Vec<String> {
    for msg in messages.iter().rev() {
        let Some(calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) else {
            continue;
        };
        for call in calls.iter().rev() {
            if call
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                != Some("request_decision")
            {
                continue;
            }
            let Some(args_str) = call
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(|a| a.as_str())
            else {
                continue;
            };
            let Ok(args) = serde_json::from_str::<serde_json::Value>(args_str) else {
                continue;
            };
            let Some(options) = args.get("options").and_then(|v| v.as_array()) else {
                continue;
            };
            return options
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
    }
    vec![]
}

#[cfg(test)]
mod tests {
    use super::extract_decision_options;

    #[test]
    fn extract_decision_options_from_tool_call() {
        let messages = vec![serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "function": {
                    "name": "request_decision",
                    "arguments": r#"{"options":["选项A","选项B"]}"#
                }
            }]
        })];
        let opts = extract_decision_options(&messages);
        assert_eq!(opts, vec!["选项A", "选项B"]);
    }
}
