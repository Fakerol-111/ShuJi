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
}

impl ToolOutput {
    fn new(ok: bool, operation: &str) -> Self {
        Self {
            ok,
            operation: operation.to_string(),
            path: None,
            message: None,
            error_code: None,
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
}
