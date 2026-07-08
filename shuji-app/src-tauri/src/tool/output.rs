use serde::Serialize;
use serde_json;

/// Structured tool result returned to the LLM as JSON.
/// Helps the model reliably determine operation outcomes.
#[derive(Debug, Serialize)]
pub struct ToolOutput {
    pub ok: bool,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    // ── P1.1: Command execution fields ────────────────────────────────
    /// Exit code of the executed command (command tools only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Stdout content (command tools only, truncated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    /// Stderr content (command tools only, truncated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    /// Whether the command timed out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
    /// Truncation info for stdout/stderr.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<TruncationInfo>,
    /// ── P2: Structured failure model fields ──────────────────────────
    /// Diagnostic quality grade for test/command failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_grade: Option<String>,
    /// Stable fingerprint for deduplicating repeated failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_fingerprint: Option<String>,
    /// Whether the agent should modify code in response to this error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub should_modify_code: Option<bool>,
    /// Suggested next diagnostic commands for the agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_next: Option<Vec<String>>,
}

/// Information about whether stdout/stderr were truncated.
#[derive(Debug, Serialize)]
pub struct TruncationInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<bool>,
}

impl ToolOutput {
    fn new(ok: bool, operation: &str) -> Self {
        Self {
            ok,
            operation: operation.to_string(),
            path: None,
            message: None,
            error_code: None,
            exit_code: None,
            stdout: None,
            stderr: None,
            timed_out: None,
            truncated: None,
            diagnostic_grade: None,
            failure_fingerprint: None,
            should_modify_code: None,
            suggested_next: None,
        }
    }

    pub fn success(operation: &str, path: &str, message: &str) -> String {
        let mut o = Self::new(true, operation);
        o.path = Some(path.to_string());
        o.message = Some(message.to_string());
        serde_json::to_string(&o).unwrap_or_else(|_| {
            format!(
                "{{\"ok\":true,\"operation\":\"{}\",\"message\":\"{}\"}}",
                operation, message
            )
        })
    }

    pub fn success_raw(operation: &str, message: &str) -> String {
        let mut o = Self::new(true, operation);
        o.message = Some(message.to_string());
        serde_json::to_string(&o).unwrap_or_else(|_| {
            format!(
                "{{\"ok\":true,\"operation\":\"{}\",\"message\":\"{}\"}}",
                operation, message
            )
        })
    }

    pub fn read_file(operation: &str, path: &str, content: &str) -> String {
        let mut o = Self::new(true, operation);
        o.path = Some(path.to_string());
        o.message = Some(format!(
            "Total {} bytes. Content:\n{}",
            content.len(),
            content
        ));
        serde_json::to_string(&o).unwrap_or_else(|_| content.to_string())
    }
    /// Check if a tool output string represents an error.
    /// Parses JSON `ok` field; falls back to keyword detection for non-JSON output.
    ///
    /// ── P1.2: Single entry point for all error classification.
    /// The watchdog, tool_exec::emit_tool_result, and UI all call this
    /// same function so error judgments are consistent everywhere.
    pub fn is_error(raw: &str) -> bool {
        serde_json::from_str::<serde_json::Value>(raw)
            .ok()
            .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
            .map(|ok| !ok)
            .unwrap_or_else(|| {
                let lower = raw.to_lowercase();
                lower.contains("failed")
                    || lower.contains("error")
                    || lower.contains("unknown tool")
                    || lower.contains("timed out")
                    || lower.contains("timeout")
            })
    }

    /// Extract the error_code from a tool output JSON, if it's an error.
    pub fn error_code(raw: &str) -> Option<String> {
        let v = serde_json::from_str::<serde_json::Value>(raw).ok()?;
        if v.get("ok")?.as_bool()? {
            return None;
        }
        v.get("error_code")
            .and_then(|c| c.as_str())
            .map(String::from)
    }

    /// Extract the message field from a tool output JSON.
    pub fn extract_message(raw: &str) -> Option<String> {
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        v.get("message").and_then(|m| m.as_str()).map(String::from)
    }

    /// Success with a warning. Creates ok=true with an attached warning field.
    pub fn success_with_warning(
        operation: &str,
        id: &str,
        warning_code: &str,
        warning_msg: &str,
    ) -> String {
        serde_json::json!({
            "ok": true,
            "operation": operation,
            "path": id,
            "message": format!("Created. Note: {}.", warning_msg),
            "warning": {
                "code": warning_code,
                "message": warning_msg,
            },
        })
        .to_string()
    }

    pub fn error(operation: &str, path: &str, code: &str, message: &str) -> String {
        let mut o = Self::new(false, operation);
        o.path = Some(path.to_string());
        o.error_code = Some(code.to_string());
        o.message = Some(message.to_string());
        serde_json::to_string(&o).unwrap_or_else(|_| {
            format!("{{\"ok\":false,\"operation\":\"{}\",\"path\":\"{}\",\"error_code\":\"{}\",\"message\":\"{}\"}}",
                operation, path, code, message)
        })
    }

    /// Error with structured diagnostic fields for the unified failure model.
    ///
    /// Includes `diagnostic_grade`, `failure_fingerprint`, `should_modify_code`,
    /// and `suggested_next` so agents and watchdog can make informed decisions
    /// without parsing the message text.
    pub fn diagnostic_error(
        operation: &str,
        path: &str,
        code: &str,
        message: &str,
        diagnostic_grade: &str,
        failure_fingerprint: &str,
        should_modify_code: bool,
        suggested_next: Vec<String>,
    ) -> String {
        let mut o = Self::new(false, operation);
        o.path = Some(path.to_string());
        o.error_code = Some(code.to_string());
        o.message = Some(message.to_string());
        o.diagnostic_grade = Some(diagnostic_grade.to_string());
        o.failure_fingerprint = Some(failure_fingerprint.to_string());
        o.should_modify_code = Some(should_modify_code);
        o.suggested_next = if suggested_next.is_empty() {
            None
        } else {
            Some(suggested_next)
        };
        serde_json::to_string(&o).unwrap_or_else(|_| {
            format!(
                "{{\"ok\":false,\"operation\":\"{}\",\"path\":\"{}\",\"error_code\":\"{}\",\"message\":\"{}\"}}",
                operation, path, code, message
            )
        })
    }

    /// ── P1.1: Build a structured command execution result ─────────────
    ///
    /// Captures exit code, stdout, stderr, timeout flag, and truncation info
    /// so the model, UI, and watchdog see the same structured fields.
    pub fn command(
        operation: &str,
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        timed_out: bool,
        max_stdout_chars: usize,
        max_stderr_chars: usize,
    ) -> String {
        let ok = exit_code == 0;
        let (stdout_truncated, display_stdout) = Self::truncate_if_needed(stdout, max_stdout_chars);
        let (stderr_truncated, display_stderr) = Self::truncate_if_needed(stderr, max_stderr_chars);

        let message = if ok {
            format!(
                "Command executed successfully (exit={}). stdout: {} bytes",
                exit_code,
                stdout.len()
            )
        } else if timed_out {
            format!(
                "Command timed out (exit={}). stdout: {} bytes, stderr: {} bytes",
                exit_code,
                stdout.len(),
                stderr.len()
            )
        } else {
            format!(
                "Command failed (exit={}). stdout: {} bytes, stderr: {} bytes",
                exit_code,
                stdout.len(),
                stderr.len()
            )
        };

        let error_code = if ok {
            None
        } else if timed_out {
            Some("timeout".to_string())
        } else {
            Some("non_zero_exit".to_string())
        };

        let truncated = if stdout_truncated || stderr_truncated {
            Some(TruncationInfo {
                stdout: if stdout_truncated { Some(true) } else { None },
                stderr: if stderr_truncated { Some(true) } else { None },
            })
        } else {
            None
        };

        let diagnostic_grade = if ok {
            None
        } else if exit_code == -1 && timed_out {
            Some("timeout".to_string())
        } else {
            Some("raw_excerpt".to_string())
        };

        let o = Self {
            ok,
            operation: operation.to_string(),
            path: None,
            message: Some(message.clone()),
            error_code: error_code.clone(),
            exit_code: Some(exit_code),
            stdout: Some(display_stdout),
            stderr: if display_stderr.is_empty() && ok {
                None
            } else {
                Some(display_stderr)
            },
            timed_out: if timed_out { Some(true) } else { None },
            truncated,
            diagnostic_grade,
            failure_fingerprint: None,
            should_modify_code: None,
            suggested_next: None,
        };
        serde_json::to_string(&o).unwrap_or_else(|_| {
            if ok {
                format!(
                    "{{\"ok\":true,\"operation\":\"{}\",\"message\":\"{}\"}}",
                    operation, message
                )
            } else {
                format!(
                    "{{\"ok\":false,\"operation\":\"{}\",\"error_code\":\"{}\",\"message\":\"{}\"}}",
                    operation,
                    error_code.unwrap_or_else(|| "unknown".to_string()),
                    message
                )
            }
        })
    }

    fn truncate_if_needed(s: &str, max_chars: usize) -> (bool, String) {
        if s.len() <= max_chars {
            (false, s.to_string())
        } else {
            let cutoff = s.floor_char_boundary(max_chars);
            (
                true,
                format!(
                    "{}...\n[Truncated: {} chars, showing first {}]",
                    &s[..cutoff],
                    s.len(),
                    cutoff
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── P1.1: command() structured output tests ────────────────────

    #[test]
    fn test_command_success() {
        let result =
            ToolOutput::command("execute_command", 0, "hello\nworld", "", false, 1000, 1000);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["operation"], "execute_command");
        assert_eq!(v["exit_code"], 0);
        assert_eq!(v["stdout"], "hello\nworld");
        assert!(v.get("stderr").is_none() || v["stderr"].as_str().unwrap_or("").is_empty());
        assert!(v.get("timed_out").is_none());
        assert!(v.get("truncated").is_none());
    }

    #[test]
    fn test_command_non_zero_exit() {
        let result = ToolOutput::command(
            "execute_command",
            1,
            "stdout data",
            "error msg",
            false,
            1000,
            1000,
        );
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error_code"], "non_zero_exit");
        assert_eq!(v["exit_code"], 1);
        assert_eq!(v["stderr"], "error msg");
    }

    #[test]
    fn test_command_timeout() {
        let result = ToolOutput::command("execute_command", -1, "", "timed out", true, 1000, 1000);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error_code"], "timeout");
        assert_eq!(v["timed_out"], true);
    }

    #[test]
    fn test_command_stdout_truncation() {
        let long_stdout = "a".repeat(500);
        let result = ToolOutput::command("execute_command", 0, &long_stdout, "", false, 100, 1000);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["ok"], true);
        let displayed = v["stdout"].as_str().unwrap();
        assert!(displayed.len() < 500);
        assert!(displayed.contains("[Truncated:"));
    }

    #[test]
    fn test_is_error_timeout_text() {
        // P1.2: timeout text should be detected as error by is_error
        assert!(ToolOutput::is_error("Command timed out"));
        assert!(ToolOutput::is_error("timeout"));
        assert!(ToolOutput::is_error("Task failed with error"));
    }

    #[test]
    fn test_is_error_json_ok() {
        // JSON with ok:true should NOT be detected as error
        let result = ToolOutput::success("test", "file.rs", "all good");
        assert!(!ToolOutput::is_error(&result));
    }

    #[test]
    fn test_is_error_json_not_ok() {
        // JSON with ok:false SHOULD be detected as error
        let result = ToolOutput::error("test", "", "some_error", "something broke");
        assert!(ToolOutput::is_error(&result));
    }

    // ── P2: diagnostic_grade in command output ─────────────────────

    #[test]
    fn test_command_diagnostic_grade_on_failure() {
        let result = ToolOutput::command("execute_command", 1, "", "error", false, 1000, 1000);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["diagnostic_grade"], "raw_excerpt");
    }

    #[test]
    fn test_command_diagnostic_grade_on_timeout() {
        let result = ToolOutput::command("execute_command", -1, "", "timeout", true, 1000, 1000);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["diagnostic_grade"], "timeout");
    }

    #[test]
    fn test_command_diagnostic_grade_on_success() {
        let result = ToolOutput::command("execute_command", 0, "ok", "", false, 1000, 1000);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(v.get("diagnostic_grade").is_none());
    }
}
