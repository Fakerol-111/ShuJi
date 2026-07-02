//! Error retry decision helpers for the Session module.
//!
//! Extracted from [`Session::step()`] in `mod.rs`. Consolidates the reasoning
//! unsupported-error detection so `step()` does not inline the parsing logic.

/// Check whether an API error indicates the provider does not support
/// reasoning/thinking parameters. If so, the caller should strip the
/// reasoning fields from the request body and retry once.
pub(super) fn should_retry_without_reasoning(
    reasoning_enabled: bool,
    already_stripped: bool,
    err: &anyhow::Error,
) -> bool {
    if !reasoning_enabled || already_stripped {
        return false;
    }
    let err_str = err.to_string();
    if !err_str.contains("400") && !err_str.contains("422") {
        return false;
    }
    let status_code = err_str
        .split("API error (")
        .nth(1)
        .and_then(|s| s.split(')').next())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<u16>().ok())
        .unwrap_or(0);
    let error_body = err_str
        .split("API error (")
        .nth(1)
        .and_then(|s| s.split("): ").nth(1))
        .unwrap_or("");
    crate::api::reasoning::looks_like_unsupported_reasoning_error(status_code, error_body)
}
