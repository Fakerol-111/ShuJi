//! Debug helpers for the Session module.
//!
//! Extracted from `mod.rs`. Two `impl Session` methods:
//! - `preview()` — truncate content for log display (multi-byte safe).
//! - `write_debug_truncated()` — dump truncated responses to
//!   `.shuji/debug/truncated.md` for post-mortem analysis.
//!
//! Kept as `impl Session` methods (not free functions) to avoid passing a
//! long parameter list (messages, role, model, max_tokens, debug_dir).

use std::path::PathBuf;

impl super::Session {
    /// Truncate content for log preview, safe for multi-byte text (e.g. Chinese).
    pub(super) fn preview(s: &str) -> String {
        let char_count = s.chars().count();
        if char_count <= 80 {
            return s.to_string();
        }
        let lines: Vec<&str> = s.lines().collect();
        if lines.len() <= 1 {
            // Single line: show first 30 chars + "..." + last 30 chars
            let head: String = s.chars().take(30).collect();
            let tail: String = s.chars().skip(char_count.saturating_sub(30)).collect();
            format!("{}...{}", head, tail)
        } else {
            // Multi-line: show first 20 chars of first line + last 20 chars of last line
            let first: String = lines[0].chars().take(20).collect();
            let last: String = lines[lines.len() - 1]
                .chars()
                .rev()
                .take(20)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            format!("{}...{} ({} lines)", first, last, lines.len())
        }
    }

    /// Write truncated output to `.shuji/debug/truncated.md` for debugging.
    /// Dumps the raw message, partial tool calls, and the last messages of
    /// session context (so we can see what caused the truncation).
    pub(super) async fn write_debug_truncated(&self, msg: &serde_json::Value, retry: u32) {
        let dir = match self.debug_dir() {
            Some(d) => d,
            None => return,
        };
        let path: PathBuf = dir.join("debug").join("truncated.md");
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let sep = "─".repeat(60);
        let content = msg["content"].as_str().unwrap_or("");
        let finish = msg
            .get("finish_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        let max_tokens = self
            .max_tokens()
            .map(|tokens| tokens.to_string())
            .unwrap_or_else(|| "unlimited".to_string());

        let mut lines = vec![
            sep.clone(),
            "# Truncated Output".to_string(),
            format!(
                "Timestamp: {}",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")
            ),
            format!("Role: {}", self.role()),
            format!("Model: {}", self.model()),
            format!("Max Tokens: {}", max_tokens),
            format!("Retry: {}/5 — finish_reason: {}", retry + 1, finish),
            format!("Session messages: {} total", self.messages_len()),
            String::new(),
        ];

        // 1. Dump the truncated response content
        lines.push("## Response Content".to_string());
        if content.is_empty() {
            lines.push("(empty — content was fully truncated)".to_string());
            // Dump the raw message JSON to see if anything survived
            lines.push(format!(
                "\nRaw message:\n```json\n{}\n```",
                serde_json::to_string_pretty(msg).unwrap_or_default()
            ));
        } else {
            lines.push(format!("({} chars)", content.chars().count()));
            lines.push(String::new());
            lines.push(content.to_string());
        }
        lines.push(String::new());

        // 2. Dump tool calls (even partial ones with broken JSON args)
        if let Some(tcs) = msg["tool_calls"].as_array() {
            let valid = tcs
                .iter()
                .filter(|tc| match &tc["function"]["arguments"] {
                    serde_json::Value::String(s) => {
                        serde_json::from_str::<serde_json::Value>(s).is_ok()
                    }
                    serde_json::Value::Object(_) => true,
                    _ => false,
                })
                .count();
            lines.push(format!(
                "## Tool Calls ({} total, {} with valid args)",
                tcs.len(),
                valid
            ));
            for (i, tc) in tcs.iter().enumerate() {
                let name = tc["function"]["name"].as_str().unwrap_or("?");
                let args_raw = tc["function"]["arguments"].as_str().unwrap_or("");
                let args_valid = match &tc["function"]["arguments"] {
                    serde_json::Value::String(s) => {
                        serde_json::from_str::<serde_json::Value>(s).is_ok()
                    }
                    serde_json::Value::Object(_) => true,
                    _ => false,
                };
                let status = if args_valid { "✓" } else { "✗ BROKEN" };
                lines.push(format!(
                    "{}. {} {} — args ({} chars):",
                    i + 1,
                    name,
                    status,
                    args_raw.len()
                ));
                // Always show the raw args for broken ones; truncate valid ones only if huge
                if !args_valid || args_raw.len() <= 1000 {
                    lines.push(format!("```\n{}\n```", args_raw));
                } else {
                    let preview: String = args_raw.chars().take(500).collect();
                    lines.push(format!("```\n{}...\n```", preview));
                }
                lines.push(String::new());
            }
        } else {
            lines.push("## Tool Calls".to_string());
            lines.push("(none)".to_string());
        }
        lines.push(String::new());

        // 3. Dump last N session messages to show what led to truncation
        let last_n = 8usize;
        let start = self.messages_len().saturating_sub(last_n);
        lines.push(format!(
            "## Last {} Session Messages (of {})",
            last_n,
            self.messages_len()
        ));
        lines.push("(most recent at bottom — shows what triggered this response)".to_string());
        lines.push(String::new());
        for (j, m) in self.messages_iter().enumerate().skip(start) {
            let role = m["role"].as_str().unwrap_or("");
            let c = m["content"].as_str().unwrap_or("");
            let tool_names: Vec<&str> = m["tool_calls"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|tc| tc["function"]["name"].as_str())
                        .collect()
                })
                .unwrap_or_default();
            let tool_id = m["tool_call_id"].as_str().unwrap_or("");

            if role == "tool" {
                let preview: String = c.chars().take(150).collect();
                let suffix = if c.chars().count() > 150 { "..." } else { "" };
                lines.push(format!(
                    "[{}] tool (id={}) → {}{}",
                    j, tool_id, preview, suffix
                ));
            } else if !tool_names.is_empty() {
                lines.push(format!("[{}] {} → tools: {:?}", j, role, tool_names));
            } else {
                let preview: String = c.chars().take(200).collect();
                let suffix = if c.chars().count() > 200 { "..." } else { "" };
                lines.push(format!("[{}] {} → {}{}", j, role, preview, suffix));
            }
        }

        if let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(&path)
            .await
        {
            use tokio::io::AsyncWriteExt;
            let _ = file
                .write_all(format!("{}\n", lines.join("\n")).as_bytes())
                .await;
        }
    }
}
