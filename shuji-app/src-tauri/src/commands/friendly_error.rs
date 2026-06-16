/// Translate raw technical errors into user-friendly English messages.
pub fn friendly_error(e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    let msg = raw.to_lowercase();

    // Handle "API error (XXX): ..." format from session.rs
    if msg.starts_with("api error") {
        if let Some(paren) = msg.find('(') {
            if let Some(close) = msg[paren..].find(')') {
                let code = &msg[paren + 1..paren + close];
                return match code {
                    "400" => "Invalid request parameters, please check your input".to_string(),
                    "401" => {
                        "API key is invalid or expired, please reconfigure in settings".to_string()
                    }
                    "403" => "API access denied, please check key permissions".to_string(),
                    "404" => "API endpoint not found, please check the API URL".to_string(),
                    "408" => "API request timed out, please retry later".to_string(),
                    "429" => "API request rate limited, please retry later".to_string(),
                    "500" => "API server internal error, please retry later".to_string(),
                    "502" | "503" => {
                        "API service temporarily unavailable, please retry later".to_string()
                    }
                    _ => format!("API error ({}): please retry later", code),
                };
            }
        }
    }

    if msg.contains("connection refused")
        || msg.contains("connect error")
        || msg.contains("tcp connect")
    {
        "Unable to connect to API server, please check network or API URL configuration".to_string()
    } else if msg.contains("401") || msg.contains("unauthorized") || msg.contains("invalid api key")
    {
        "API key is invalid or expired, please reconfigure in settings".to_string()
    } else if msg.contains("403") || msg.contains("forbidden") {
        "API access denied, please check key permissions".to_string()
    } else if msg.contains("429") || msg.contains("rate limit") {
        "API request rate limited, please retry later".to_string()
    } else if msg.contains("timeout") || msg.contains("timed out") {
        "API request timed out, please retry later or check network".to_string()
    } else if msg.contains("500") || msg.contains("internal server error") {
        "API server internal error, please retry later".to_string()
    } else if msg.contains("502") || msg.contains("503") {
        "API service temporarily unavailable, please retry later".to_string()
    } else {
        format!("System error: {}", msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_401() {
        let msg = friendly_error("API error (401): invalid key");
        assert!(
            msg.contains("key"),
            "expected key-related msg, got: {}",
            msg
        );
    }

    #[test]
    fn test_api_429() {
        let msg = friendly_error("API error (429): rate limit exceeded");
        assert!(
            msg.contains("rate limited"),
            "expected rate-limit msg, got: {}",
            msg
        );
    }

    #[test]
    fn test_api_500() {
        let msg = friendly_error("API error (500): internal error");
        assert!(
            msg.contains("internal error"),
            "expected internal error msg, got: {}",
            msg
        );
    }

    #[test]
    fn test_connection_refused() {
        let msg = friendly_error("connection refused: tcp connect error");
        assert!(
            msg.contains("connect"),
            "expected connection msg, got: {}",
            msg
        );
    }

    #[test]
    fn test_timeout() {
        let msg = friendly_error("request timed out after 30s");
        assert!(
            msg.contains("timed out"),
            "expected timeout msg, got: {}",
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
            msg.contains("key"),
            "expected key-related msg, got: {}",
            msg
        );
    }

    #[test]
    fn test_service_unavailable() {
        let msg = friendly_error("503 Service Unavailable");
        assert!(
            msg.contains("unavailable"),
            "expected unavailable msg, got: {}",
            msg
        );
    }
}
