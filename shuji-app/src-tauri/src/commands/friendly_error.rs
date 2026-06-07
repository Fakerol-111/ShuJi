/// Translate raw technical errors into user-friendly Chinese messages.
pub fn friendly_error(e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    let msg = raw.to_lowercase();

    // Handle "API error (XXX): ..." format from session.rs
    if msg.starts_with("api error") {
        if let Some(paren) = msg.find('(') {
            if let Some(close) = msg[paren..].find(')') {
                let code = &msg[paren + 1..paren + close];
                return match code {
                    "400" => "请求参数错误，请检查输入".to_string(),
                    "401" => "API 密钥无效或已过期，请在设置中重新配置".to_string(),
                    "403" => "API 访问被拒绝，请检查密钥权限".to_string(),
                    "404" => "API 端点不存在，请检查 API URL".to_string(),
                    "408" => "API 请求超时，请稍后重试".to_string(),
                    "429" => "API 请求过于频繁，请稍后重试".to_string(),
                    "500" => "API 服务器内部错误，请稍后重试".to_string(),
                    "502" | "503" => "API 服务暂时不可用，请稍后重试".to_string(),
                    _ => format!("API 错误 ({}): 请稍后重试", code),
                };
            }
        }
    }

    if msg.contains("connection refused")
        || msg.contains("connect error")
        || msg.contains("tcp connect")
    {
        "无法连接 API 服务器，请检查网络或 API URL 配置".to_string()
    } else if msg.contains("401") || msg.contains("unauthorized") || msg.contains("invalid api key")
    {
        "API 密钥无效或已过期，请在设置中重新配置".to_string()
    } else if msg.contains("403") || msg.contains("forbidden") {
        "API 访问被拒绝，请检查密钥权限".to_string()
    } else if msg.contains("429") || msg.contains("rate limit") {
        "API 请求过于频繁，请稍后重试".to_string()
    } else if msg.contains("timeout") || msg.contains("timed out") {
        "API 请求超时，请稍后重试或检查网络".to_string()
    } else if msg.contains("500") || msg.contains("internal server error") {
        "API 服务器内部错误，请稍后重试".to_string()
    } else if msg.contains("502") || msg.contains("503") {
        "API 服务暂时不可用，请稍后重试".to_string()
    } else {
        format!("系统错误: {}", msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_401() {
        let msg = friendly_error("API error (401): invalid key");
        assert!(msg.contains("密钥"), "expected key-related msg, got: {}", msg);
    }

    #[test]
    fn test_api_429() {
        let msg = friendly_error("API error (429): rate limit exceeded");
        assert!(msg.contains("频繁"), "expected rate-limit msg, got: {}", msg);
    }

    #[test]
    fn test_api_500() {
        let msg = friendly_error("API error (500): internal error");
        assert!(msg.contains("内部错误"), "expected internal error msg, got: {}", msg);
    }

    #[test]
    fn test_connection_refused() {
        let msg = friendly_error("connection refused: tcp connect error");
        assert!(msg.contains("无法连接"), "expected connection msg, got: {}", msg);
    }

    #[test]
    fn test_timeout() {
        let msg = friendly_error("request timed out after 30s");
        assert!(msg.contains("超时"), "expected timeout msg, got: {}", msg);
    }

    #[test]
    fn test_unknown_error() {
        let msg = friendly_error("some weird error occurred");
        assert!(msg.contains("系统错误"), "expected system error fallback, got: {}", msg);
    }

    #[test]
    fn test_unauthorized_without_code() {
        let msg = friendly_error("401 Unauthorized");
        assert!(msg.contains("密钥"), "expected key-related msg, got: {}", msg);
    }

    #[test]
    fn test_service_unavailable() {
        let msg = friendly_error("503 Service Unavailable");
        assert!(msg.contains("暂时不可用"), "expected unavailable msg, got: {}", msg);
    }
}
