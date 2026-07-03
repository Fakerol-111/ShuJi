//! Tool definition for summarize_logs and request_decision.
//! Extracted from dispatch.rs.

use crate::tool::ToolOutput;

/// Read `.shuji/logs/activity.log`, parse JSON lines, return as formatted text.
pub async fn tool_summarize_logs(
    working_dir: &std::path::Path,
    args: &serde_json::Value,
) -> String {
    let log_path = working_dir.join(".shuji").join("logs").join("activity.log");
    if !log_path.exists() {
        return ToolOutput::success_raw("summarize_logs", "No log entries yet");
    }

    let content = match tokio::fs::read_to_string(&log_path).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("summarize_logs", "", "read_error", &e.to_string()),
    };

    let since = args["since"].as_u64().unwrap_or(0) as usize;
    let mut entries: Vec<serde_json::Value> = Vec::new();
    let total_lines;

    {
        let lines: Vec<&str> = content.lines().collect();
        total_lines = lines.len();
        for line in lines.iter().skip(since) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                entries.push(val);
            }
        }
    }

    if entries.is_empty() {
        return ToolOutput::success_raw("summarize_logs", "No log entries yet");
    }

    let mut result = Vec::new();
    result.push(format!(
        "{} log entries (file has {} lines, starting from line {}):",
        entries.len(),
        total_lines,
        since
    ));
    result.push(String::new());

    for entry in &entries {
        let ts = entry["ts"].as_str().unwrap_or("?");
        let author = entry["author"].as_str().unwrap_or("?");
        let summary = entry["summary"].as_str().unwrap_or("");

        let short_ts = if ts.len() > 19 { &ts[..19] } else { ts };
        result.push(format!("[{}] {}: {}", short_ts, author, summary));
    }

    ToolOutput::success_raw("summarize_logs", &result.join("\n"))
}

/// Tool definition for summarize_logs.
pub fn summarize_logs_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "summarize_logs".into(),
            description: "Read activity.log; can incrementally read by line number".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "since": {
                        "type": "integer",
                        "description": "Starting line number (0-based), omit to read from the beginning"
                    }
                }
            }),
        },
    }
}

/// Tool definition for request_decision.
pub fn request_decision_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "request_decision".into(),
            description: "Call when the emperor's decision is needed. Pass a list of options for the emperor to choose from. Must include context before the options explaining why a decision is needed.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "options": {
                        "type": "array",
                        "description": "List of options for the emperor to choose from (at least 1)",
                        "items": {"type": "string"},
                        "minItems": 1
                    }
                },
                "required": ["options"]
            }),
        },
    }
}

pub async fn tool_request_decision(args: &serde_json::Value) -> String {
    let options = match args["options"].as_array() {
        Some(arr) if !arr.is_empty() => arr,
        _ => {
            return ToolOutput::error(
                "request_decision",
                "",
                "empty_options",
                "options cannot be empty",
            )
        }
    };
    let mut msg = "[Waiting for emperor's decision] Please choose one:\n".to_string();
    for (i, opt) in options.iter().enumerate() {
        let text = opt.as_str().unwrap_or("(invalid option)");
        msg.push_str(&format!("{}. {}\n", i + 1, text));
    }
    ToolOutput::success_raw("request_decision", msg.trim())
}
