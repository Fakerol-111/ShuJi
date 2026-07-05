//! 通用错误友好化处理 — 委托给 `error_code` 模块进行结构化分类。
//!
//! `friendly_error()` 返回 JSON 字符串，前端可 `JSON.parse` 后：
//! - 用 `data.code` 查 i18n key `error.api.<code>` 做翻译
//! - 用 `data.message` 直接展示
//! - 用 `data.detail` 调试原始错误

use crate::commands::error_code::{friendly_error_code, ShujiError};

/// 将任意错误转换为 JSON 字符串返回给前端。
///
/// 返回格式：
/// - 结构化：`{"type":"Structured","data":{"code":"api_key_invalid","message":"...","detail":"..."}}`
/// - 兜底：`{"type":"Plain","data":"System error: ..."}`
pub fn friendly_error(e: impl std::fmt::Display) -> String {
    friendly_error_code(e).to_json_string()
}

/// 仅返回人类可读消息文本（不包含 JSON 包装）— 适用于不需要结构化 code 的场景。
#[allow(dead_code)]
pub fn friendly_error_message(e: impl std::fmt::Display) -> String {
    friendly_error_code(e).message().to_string()
}

/// 从人类可读文本直接构造 `ShujiError::Plain`，用于非分类性错误（如 "no open project"）。
pub fn friendly_error_plain(msg: impl Into<String>) -> String {
    ShujiError::Plain(msg.into()).to_json_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_401() {
        let msg = friendly_error("API error (401): invalid key");
        assert!(
            msg.contains("api_key_invalid"),
            "expected structured code, got: {}",
            msg
        );
    }

    #[test]
    fn test_api_429() {
        let msg = friendly_error("API error (429): rate limit exceeded");
        assert!(
            msg.contains("api_rate_limited"),
            "expected rate-limited code, got: {}",
            msg
        );
    }

    #[test]
    fn test_api_500() {
        let msg = friendly_error("API error (500): internal error");
        assert!(
            msg.contains("api_server_error"),
            "expected server-error code, got: {}",
            msg
        );
    }

    #[test]
    fn test_connection_refused() {
        let msg = friendly_error("connection refused: tcp connect error");
        assert!(
            msg.contains("api_connection_failed"),
            "expected connection-failed code, got: {}",
            msg
        );
    }

    #[test]
    fn test_timeout() {
        let msg = friendly_error("request timed out after 30s");
        assert!(
            msg.contains("api_timeout"),
            "expected timeout code, got: {}",
            msg
        );
    }

    #[test]
    fn test_unknown_error() {
        let msg = friendly_error("some weird error occurred");
        assert!(
            msg.contains("System error"),
            "expected system error fallback, got: {}",
            msg
        );
    }

    #[test]
    fn test_unauthorized_without_code() {
        let msg = friendly_error("401 Unauthorized");
        assert!(
            msg.contains("api_key_invalid"),
            "expected key-invalid code, got: {}",
            msg
        );
    }

    #[test]
    fn test_service_unavailable() {
        let msg = friendly_error("503 Service Unavailable");
        assert!(
            msg.contains("api_service_unavailable"),
            "expected unavailable code, got: {}",
            msg
        );
    }

    #[test]
    fn test_returns_valid_json() {
        let msg = friendly_error("API error (401): invalid key");
        let parsed: serde_json::Value = serde_json::from_str(&msg).expect("should be valid JSON");
        assert_eq!(parsed["type"], "Structured");
        assert_eq!(parsed["data"]["code"], "api_key_invalid");
        assert!(parsed["data"]["message"].is_string());
    }

    #[test]
    fn test_friendly_error_message() {
        let msg = friendly_error_message("API error (429): rate limit");
        assert!(msg.contains("rate limited"));
    }

    #[test]
    fn test_friendly_error_plain() {
        let msg = friendly_error_plain("no open project");
        let parsed: serde_json::Value = serde_json::from_str(&msg).expect("should be valid JSON");
        assert_eq!(parsed["type"], "Plain");
        assert_eq!(parsed["data"], "no open project");
    }
}
