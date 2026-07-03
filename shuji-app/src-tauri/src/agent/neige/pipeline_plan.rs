//! Pipeline plan extraction — finds the latest `submit_pipeline_plan` tool call
//! from a slice of session messages. Moved from mod.rs for separation of concerns.

/// Extract the plan_json from the most recent `submit_pipeline_plan` tool call
/// in the given message slice. Uses `rev()` to find the last occurrence.
pub(crate) fn extract_plan_json_from_messages<'a, I>(messages: I) -> Option<String>
where
    I: DoubleEndedIterator<Item = &'a serde_json::Value>,
{
    messages.rev().find_map(|m| {
        m.get("tool_calls")
            .and_then(|tc| tc.as_array())
            .and_then(|calls| {
                calls.iter().find(|c| {
                    c.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        == Some("submit_pipeline_plan")
                })
            })
            .and_then(|call| {
                let args_str = call
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())?;
                serde_json::from_str::<serde_json::Value>(args_str)
                    .ok()
                    .and_then(|v| {
                        v.get("plan_json")
                            .and_then(|pj| pj.as_str())
                            .map(String::from)
                    })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::extract_plan_json_from_messages;

    #[test]
    fn neige_does_not_replay_stale_pipeline_plan() {
        let stale = serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "function": {
                    "name": "submit_pipeline_plan",
                    "arguments": r#"{"plan_json":"{\"plan_id\":\"old-plan\"}"}"#
                }
            }]
        });
        let summary_only = vec![serde_json::json!({
            "role": "assistant",
            "content": "任务已完成，总结如下..."
        })];
        // Only scan new messages (summary turn) — stale plan in history is skipped.
        let plan = extract_plan_json_from_messages(summary_only.iter());
        assert!(plan.is_none());

        // Full history would incorrectly find stale plan if scanned entirely.
        let mut all = vec![stale];
        all.extend(summary_only);
        let stale_found = extract_plan_json_from_messages(all.iter());
        assert_eq!(stale_found.as_deref(), Some(r#"{"plan_id":"old-plan"}"#));
    }

    #[test]
    fn neige_extracts_only_current_turn_pipeline_plan() {
        let old = serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "function": {
                    "name": "submit_pipeline_plan",
                    "arguments": r#"{"plan_json":"{\"plan_id\":\"old\"}"}"#
                }
            }]
        });
        let new_msg = serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "function": {
                    "name": "submit_pipeline_plan",
                    "arguments": r#"{"plan_json":"{\"plan_id\":\"new-plan\"}"}"#
                }
            }]
        });
        let before_len = 1;
        let all = [old, new_msg];
        let plan = extract_plan_json_from_messages(all.iter().skip(before_len));
        assert_eq!(plan.as_deref(), Some(r#"{"plan_id":"new-plan"}"#));
    }
}
