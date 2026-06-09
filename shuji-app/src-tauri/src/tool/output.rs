use serde::Serialize;

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
            "共 {} 字节。内容如下：\n{}",
            content.len(),
            content
        ));
        serde_json::to_string(&o).unwrap_or_else(|_| content.to_string())
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

