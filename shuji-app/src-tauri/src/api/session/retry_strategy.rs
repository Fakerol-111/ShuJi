//! Exponential backoff retry strategy for API errors.
//!
//! Replaces the previous fixed 2-second delay with configurable exponential
//! backoff and optional jitter. Also provides retryable-error classification
//! so only transient failures (5xx, 429, network) trigger retries.

use std::time::Duration;

use crate::config::RetryConfig;

/// HTTP status codes that are retryable (server errors + rate limit).
const RETRYABLE_STATUS_CODES: &[u16] = &[429, 500, 502, 503, 504];

/// Calculate the backoff delay for a given retry attempt (0-indexed).
///
/// Formula: `min(initial * multiplier^attempt, max_backoff)`
/// With jitter: add up to 25% random variance to prevent thundering herd.
pub fn calculate_backoff(retry_count: u32, config: &RetryConfig) -> Duration {
    let base_ms = config.initial_backoff_ms as f64;
    let multiplier = config.backoff_multiplier;
    let max_ms = config.max_backoff_ms as f64;

    let mut delay_ms = base_ms * multiplier.powi(retry_count as i32);
    delay_ms = delay_ms.min(max_ms);

    if config.jitter {
        // Add 0-25% jitter using a simple thread-local pseudo-random source.
        // We use std::time for deterministic-enough jitter without pulling in rand.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let jitter_factor = 1.0 + ((nanos % 25) as f64 / 100.0);
        delay_ms *= jitter_factor;
        delay_ms = delay_ms.min(max_ms);
    }

    Duration::from_millis(delay_ms as u64)
}

/// Extract HTTP status code from an anyhow::Error's string representation.
/// The error message format from the API client is: "API error (429): ..."
pub fn extract_status_code(err: &anyhow::Error) -> Option<u16> {
    let err_str = err.to_string();
    err_str
        .split("API error (")
        .nth(1)
        .and_then(|s| s.split(')').next())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<u16>().ok())
}

/// Check if an error is retryable based on its content.
///
/// Retryable conditions:
/// - HTTP 429 (rate limit)
/// - HTTP 5xx (server errors)
/// - Network/timeout errors (reqwest errors without a status code)
/// - Connection reset / broken pipe
pub fn is_retryable_error(err: &anyhow::Error) -> bool {
    let err_str = err.to_string();

    // If we can extract a status code, check against retryable codes
    if let Some(code) = extract_status_code(err) {
        return RETRYABLE_STATUS_CODES.contains(&code);
    }

    // Network-level errors (no HTTP status) are always retryable
    // These typically show up as reqwest::Error messages
    let network_error_indicators = [
        "error sending request",
        "connection",
        "timeout",
        "timed out",
        "broken pipe",
        "connection reset",
        "connect error",
        "dns error",
        "tcp connect",
        "request was cancelled",
        "pool",
        "broken",
    ];

    let err_lower = err_str.to_lowercase();
    network_error_indicators
        .iter()
        .any(|indicator| err_lower.contains(indicator))
}

/// Classify an error and decide whether to retry, along with the backoff duration.
///
/// Returns `Some(duration)` if the error is retryable and retries remain,
/// `None` if the error is not retryable or no retries remain.
pub fn should_retry_with_backoff(
    err: &anyhow::Error,
    retry_count: u32,
    max_retries: u32,
    config: &RetryConfig,
) -> Option<Duration> {
    if retry_count >= max_retries {
        return None;
    }

    if !is_retryable_error(err) {
        return None;
    }

    // For 429 rate limit, the server may provide a Retry-After hint.
    // We check for it in the error message and use it if available.
    let err_str = err.to_string();
    if let Some(retry_after_secs) = extract_retry_after(&err_str) {
        return Some(Duration::from_secs(retry_after_secs));
    }

    Some(calculate_backoff(retry_count, config))
}

/// Extract Retry-After value from error message (if present).
/// Looks for "Retry-After: N" or "retry_after: N" patterns.
fn extract_retry_after(err_str: &str) -> Option<u64> {
    // Common patterns in error bodies
    for prefix in &["Retry-After: ", "retry_after: ", "\"retry_after\":"] {
        if let Some(pos) = err_str.find(prefix) {
            let after = &err_str[pos + prefix.len()..];
            // Skip whitespace after the prefix
            let after = after.trim_start();
            // Try to parse the first number after the prefix
            let num: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = num.parse::<u64>() {
                // Cap at max_backoff to respect configuration
                return Some(n.min(120)); // Cap at 2 minutes max
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> RetryConfig {
        RetryConfig {
            initial_backoff_ms: 1000,
            max_backoff_ms: 30000,
            backoff_multiplier: 2.0,
            jitter: false,
        }
    }

    #[test]
    fn test_backoff_without_jitter() {
        let config = default_config();
        assert_eq!(calculate_backoff(0, &config), Duration::from_millis(1000));
        assert_eq!(calculate_backoff(1, &config), Duration::from_millis(2000));
        assert_eq!(calculate_backoff(2, &config), Duration::from_millis(4000));
        assert_eq!(calculate_backoff(3, &config), Duration::from_millis(8000));
    }

    #[test]
    fn test_backoff_capped_at_max() {
        let config = default_config();
        // 2^5 * 1000 = 32000 > 30000 max
        assert_eq!(calculate_backoff(5, &config), Duration::from_millis(30000));
    }

    #[test]
    fn test_backoff_with_jitter_stays_in_bounds() {
        let config = RetryConfig {
            initial_backoff_ms: 1000,
            max_backoff_ms: 30000,
            backoff_multiplier: 2.0,
            jitter: true,
        };
        for i in 0..10 {
            let delay = calculate_backoff(i, &config);
            // Base delay for attempt i: 1000 * 2^i, capped at 30000
            // With jitter: up to 1.25x, but also capped at 30000
            assert!(
                delay <= Duration::from_millis(30000),
                "delay {} for attempt {} exceeds max",
                delay.as_millis(),
                i
            );
            assert!(
                delay >= Duration::from_millis(1000),
                "delay {} for attempt {} below initial",
                delay.as_millis(),
                i
            );
        }
    }

    #[test]
    fn test_is_retryable_429() {
        let err = anyhow::anyhow!("API error (429): Rate limit exceeded");
        assert!(is_retryable_error(&err));
    }

    #[test]
    fn test_is_retryable_500() {
        let err = anyhow::anyhow!("API error (500): Internal server error");
        assert!(is_retryable_error(&err));
    }

    #[test]
    fn test_is_retryable_503() {
        let err = anyhow::anyhow!("API error (503): Service unavailable");
        assert!(is_retryable_error(&err));
    }

    #[test]
    fn test_not_retryable_400() {
        let err = anyhow::anyhow!("API error (400): Bad request");
        assert!(!is_retryable_error(&err));
    }

    #[test]
    fn test_not_retryable_401() {
        let err = anyhow::anyhow!("API error (401): Unauthorized");
        assert!(!is_retryable_error(&err));
    }

    #[test]
    fn test_retryable_network_error() {
        let err = anyhow::anyhow!("error sending request: connection timeout");
        assert!(is_retryable_error(&err));
    }

    #[test]
    fn test_retryable_connection_reset() {
        let err = anyhow::anyhow!("connection reset by peer");
        assert!(is_retryable_error(&err));
    }

    #[test]
    fn test_should_retry_with_backoff_retryable() {
        let config = default_config();
        let err = anyhow::anyhow!("API error (503): Service unavailable");
        let result = should_retry_with_backoff(&err, 0, 3, &config);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Duration::from_millis(1000));
    }

    #[test]
    fn test_should_retry_with_backoff_max_retries_exceeded() {
        let config = default_config();
        let err = anyhow::anyhow!("API error (503): Service unavailable");
        let result = should_retry_with_backoff(&err, 3, 3, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_should_retry_with_backoff_non_retryable() {
        let config = default_config();
        let err = anyhow::anyhow!("API error (400): Bad request");
        let result = should_retry_with_backoff(&err, 0, 3, &config);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_retry_after() {
        let err_str = "API error (429): Rate limit. Retry-After: 30";
        assert_eq!(extract_retry_after(err_str), Some(30));
    }

    #[test]
    fn test_extract_retry_after_json_format() {
        let err_str = r#"API error (429): {"retry_after": 45}"#;
        assert_eq!(extract_retry_after(err_str), Some(45));
    }

    #[test]
    fn test_extract_retry_after_not_present() {
        let err_str = "API error (500): Internal server error";
        assert_eq!(extract_retry_after(err_str), None);
    }

    #[test]
    fn test_extract_status_code() {
        let err = anyhow::anyhow!("API error (502): Bad gateway");
        assert_eq!(extract_status_code(&err), Some(502));
    }

    #[test]
    fn test_extract_status_code_none() {
        let err = anyhow::anyhow!("connection timeout");
        assert_eq!(extract_status_code(&err), None);
    }
}
