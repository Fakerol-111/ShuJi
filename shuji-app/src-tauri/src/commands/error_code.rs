//! Structured error codes for i18n — frontend maps `code` to translated message.
//!
//! The `ShujiError` enum serializes to JSON so the frontend can distinguish
//! structured errors (with a `code` field) from plain text fallbacks.

use serde::Serialize;

/// 结构化错误 — 前端可直接用 `code` 做 i18n 映射。
///
/// 序列化格式：
/// - `Structured`: `{"type":"Structured","data":{"code":"api_key_invalid","detail":"..."}}`
/// - `Plain`: `{"type":"Plain","data":"System error: ..."}`
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ShujiError {
    /// 结构化错误 — 前端根据 `code` 查 i18n key `error.api.<code>`
    Structured {
        code: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// 兜底：纯文本错误
    Plain(String),
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

/// 从任意错误生成结构化 `ShujiError`。
///
/// 优先匹配已知错误模式返回 `Structured`，无法匹配时返回 `Plain`。
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
    }

    #[test]
    fn test_serialization_plain() {
        let err = friendly_error_code("weird stuff");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"type\":\"Plain\""));
    }
}
