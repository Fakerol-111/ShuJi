//! Structured error codes for i18n — frontend maps `code` to translated message.
//!
//! The `ShujiError` enum serializes to JSON so the frontend can distinguish
//! structured errors (with a `code` field) from plain text fallbacks.
//!
//! `friendly_error()` in `friendly_error.rs` delegates to
//! `friendly_error_code()` here, so this module is the single source of truth
//! for error classification.

use serde::Serialize;

/// 结构化错误 — 前端可直接用 `code` 做 i18n 映射，`message` 做展示。
///
/// 序列化格式：
/// - `Structured`: `{"type":"Structured","data":{"code":"api_key_invalid","message":"...","detail":"..."}}`
/// - `Plain`: `{"type":"Plain","data":"System error: ..."}`
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ShujiError {
    /// 结构化错误 — 前端根据 `code` 查 i18n key `error.api.<code>`，
    /// `message` 为可直接展示的人类可读文本。
    Structured {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// 兜底：纯文本错误（无法分类的错误）
    Plain(String),
}

impl ShujiError {
    /// 返回人类可读的消息文本（不论 Structured 还是 Plain）。
    #[allow(dead_code)]
    pub fn message(&self) -> &str {
        match self {
            ShujiError::Structured { message, .. } => message,
            ShujiError::Plain(msg) => msg,
        }
    }

    /// 序列化为 JSON 字符串，供 Tauri command 返回给前端。
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"type":"Plain","data":"Internal error serialization failed"}"#.to_string()
        })
    }
}

/// 错误码常量 — 与前端 i18n key `error.api.<code>` 对应。
pub mod codes {
    pub const API_KEY_INVALID: &str = "api_key_invalid";
    pub const API_FORBIDDEN: &str = "api_forbidden";
    pub const API_RATE_LIMITED: &str = "api_rate_limited";
    pub const API_TIMEOUT: &str = "api_timeout";
    pub const API_CONNECTION_FAILED: &str = "api_connection_failed";
    pub const API_SERVER_ERROR: &str = "api_server_error";
    pub const API_SERVICE_UNAVAILABLE: &str = "api_service_unavailable";
    pub const API_BAD_REQUEST: &str = "api_bad_request";
    pub const API_NOT_FOUND: &str = "api_not_found";
}

/// 从 HTTP 状态码到人类可读消息的映射。
fn http_status_message(code: &str) -> Option<&'static str> {
    Some(match code {
        "400" => "Invalid request parameters, please check your input",
        "401" => "API key is invalid or expired, please reconfigure in settings",
        "403" => "API access denied, please check key permissions",
        "404" => "API endpoint not found, please check the API URL",
        "408" => "API request timed out, please retry later",
        "429" => "API request rate limited, please retry later",
        "500" => "API server internal error, please retry later",
        "502" | "503" => "API service temporarily unavailable, please retry later",
        _ => return None,
    })
}

/// 从错误码常量到人类可读消息的映射。
fn code_message(code: &str) -> &'static str {
    match code {
        codes::API_KEY_INVALID => "API key is invalid or expired, please reconfigure in settings",
        codes::API_FORBIDDEN => "API access denied, please check key permissions",
        codes::API_RATE_LIMITED => "API request rate limited, please retry later",
        codes::API_TIMEOUT => "API request timed out, please retry later or check network",
        codes::API_CONNECTION_FAILED => {
            "Unable to connect to API server, please check network or API URL configuration"
        }
        codes::API_SERVER_ERROR => "API server internal error, please retry later",
        codes::API_SERVICE_UNAVAILABLE => "API service temporarily unavailable, please retry later",
        codes::API_BAD_REQUEST => "Invalid request parameters, please check your input",
        codes::API_NOT_FOUND => "API endpoint not found, please check the API URL",
        _ => "Unknown error",
    }
}

/// 从任意错误生成结构化 `ShujiError`。
///
/// 优先匹配已知错误模式返回 `Structured`（含 `code` + `message` + `detail`），
/// 无法匹配时返回 `Plain`。
pub fn friendly_error_code(e: impl std::fmt::Display) -> ShujiError {
    let raw = e.to_string();
    let lower = raw.to_lowercase();

    // Handle "API error (XXX): ..." format
    if lower.starts_with("api error") {
        if let Some(paren) = lower.find('(') {
            if let Some(close) = lower[paren..].find(')') {
                let code = &lower[paren + 1..paren + close];
                let mapped = match code {
                    "400" => Some(codes::API_BAD_REQUEST),
                    "401" => Some(codes::API_KEY_INVALID),
                    "403" => Some(codes::API_FORBIDDEN),
                    "404" => Some(codes::API_NOT_FOUND),
                    "408" => Some(codes::API_TIMEOUT),
                    "429" => Some(codes::API_RATE_LIMITED),
                    "500" => Some(codes::API_SERVER_ERROR),
                    "502" | "503" => Some(codes::API_SERVICE_UNAVAILABLE),
                    _ => None,
                };
                if let Some(c) = mapped {
                    return ShujiError::Structured {
                        code: c.to_string(),
                        message: http_status_message(code)
                            .unwrap_or("Unknown API error")
                            .to_string(),
                        detail: Some(raw),
                    };
                }
            }
        }
    }

    // Keyword-based matching
    let code = if lower.contains("connection refused")
        || lower.contains("connect error")
        || lower.contains("tcp connect")
    {
        Some(codes::API_CONNECTION_FAILED)
    } else if lower.contains("401")
        || lower.contains("unauthorized")
        || lower.contains("invalid api key")
    {
        Some(codes::API_KEY_INVALID)
    } else if lower.contains("403") || lower.contains("forbidden") {
        Some(codes::API_FORBIDDEN)
    } else if lower.contains("429") || lower.contains("rate limit") {
        Some(codes::API_RATE_LIMITED)
    } else if lower.contains("timeout") || lower.contains("timed out") {
        Some(codes::API_TIMEOUT)
    } else if lower.contains("500") || lower.contains("internal server error") {
        Some(codes::API_SERVER_ERROR)
    } else if lower.contains("502") || lower.contains("503") {
        Some(codes::API_SERVICE_UNAVAILABLE)
    } else if lower.contains("400") || lower.contains("bad request") {
        Some(codes::API_BAD_REQUEST)
    } else if lower.contains("404") || lower.contains("not found") {
        Some(codes::API_NOT_FOUND)
    } else {
        None
    };

    match code {
        Some(c) => ShujiError::Structured {
            code: c.to_string(),
            message: code_message(c).to_string(),
            detail: Some(raw),
        },
        None => ShujiError::Plain(format!("System error: {}", lower)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_401_structured() {
        let err = friendly_error_code("API error (401): invalid key");
        match err {
            ShujiError::Structured { code, .. } => {
                assert_eq!(code, codes::API_KEY_INVALID);
            }
            ShujiError::Plain(_) => panic!("expected Structured"),
        }
    }

    #[test]
    fn test_api_429_structured() {
        let err = friendly_error_code("API error (429): rate limit exceeded");
        match err {
            ShujiError::Structured { code, .. } => {
                assert_eq!(code, codes::API_RATE_LIMITED);
            }
            ShujiError::Plain(_) => panic!("expected Structured"),
        }
    }

    #[test]
    fn test_connection_refused_structured() {
        let err = friendly_error_code("connection refused: tcp connect error");
        match err {
            ShujiError::Structured { code, .. } => {
                assert_eq!(code, codes::API_CONNECTION_FAILED);
            }
            ShujiError::Plain(_) => panic!("expected Structured"),
        }
    }

    #[test]
    fn test_timeout_structured() {
        let err = friendly_error_code("request timed out after 30s");
        match err {
            ShujiError::Structured { code, .. } => {
                assert_eq!(code, codes::API_TIMEOUT);
            }
            ShujiError::Plain(_) => panic!("expected Structured"),
        }
    }

    #[test]
    fn test_unknown_error_plain() {
        let err = friendly_error_code("some weird error occurred");
        match err {
            ShujiError::Plain(msg) => {
                assert!(msg.contains("System error"));
            }
            ShujiError::Structured { .. } => panic!("expected Plain"),
        }
    }

    #[test]
    fn test_unauthorized_without_code() {
        let err = friendly_error_code("401 Unauthorized");
        match err {
            ShujiError::Structured { code, .. } => {
                assert_eq!(code, codes::API_KEY_INVALID);
            }
            ShujiError::Plain(_) => panic!("expected Structured"),
        }
    }

    #[test]
    fn test_serialization_structured() {
        let err = friendly_error_code("API error (503): Service unavailable");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"type\":\"Structured\""));
        assert!(json.contains("\"code\":\"api_service_unavailable\""));
        assert!(json.contains("\"message\""));
    }

    #[test]
    fn test_serialization_plain() {
        let err = friendly_error_code("weird stuff");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"type\":\"Plain\""));
    }

    #[test]
    fn test_structured_has_message() {
        let err = friendly_error_code("API error (401): invalid key");
        match err {
            ShujiError::Structured { message, .. } => {
                assert!(message.contains("API key"));
            }
            _ => panic!("expected Structured"),
        }
    }

    #[test]
    fn test_message_method() {
        let err = friendly_error_code("API error (429): rate limit");
        assert!(err.message().contains("rate limited"));
    }

    #[test]
    fn test_to_json_string() {
        let err = friendly_error_code("API error (500): server error");
        let json = err.to_json_string();
        assert!(json.contains("\"type\":\"Structured\""));
        assert!(json.contains("\"code\":\"api_server_error\""));
    }
}
