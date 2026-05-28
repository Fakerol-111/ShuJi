/// Translate raw technical errors into user-friendly Chinese messages.
#[allow(dead_code)]
pub fn friendly_error(e: impl std::fmt::Display) -> String {
    let msg = e.to_string().to_lowercase();
    if msg.contains("connection refused") || msg.contains("connect error") || msg.contains("tcp connect") {
        "无法连接 API 服务器，请检查网络或 API URL 配置".to_string()
    } else if msg.contains("401") || msg.contains("unauthorized") || msg.contains("invalid api key") {
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
