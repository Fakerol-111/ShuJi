//! self_execute handler registry.
//!
//! Handlers are registered by name and dispatched from `engine.rs` when
//! executing a `self_execute` step.

use std::path::Path;

/// Outcome of a self_execute handler.
#[derive(Debug, Clone)]
pub enum SelfExecuteOutcome {
    Success {
        message: String,
        artifact: Option<String>,
    },
    Failed {
        reason: String,
    },
}

/// Dispatch to the named handler with the given parameters.
///
/// Phase 1 handlers:
/// - `validate_delivery`: calls `validate::delivery::validate_delivery`
/// - `noop`: test-only, returns success immediately
pub async fn run_self_execute(
    handler: &str,
    params: &serde_json::Value,
    project_dir: &Path,
) -> Result<SelfExecuteOutcome, String> {
    match handler {
        "validate_delivery" => {
            let ctrt_id = params
                .get("ctrt_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let run_lint = params
                .get("run_lint")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let run_contract_diff = params
                .get("run_contract_diff")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let test_scope = params
                .get("test_scope")
                .and_then(|v| v.as_str())
                .unwrap_or("all")
                .to_string();

            let opts = crate::validate::DeliveryOptions {
                ctrt_id,
                run_contract_diff,
                run_lint,
                test_scope,
            };

            match crate::validate::delivery::validate_delivery(project_dir, &opts).await {
                Ok(report) => {
                    let pass_str = if report.overall_pass {
                        "passed"
                    } else {
                        "failed"
                    };
                    let msg = format!(
                        "Validation{}: {} ({} checks)",
                        pass_str,
                        report.project_type,
                        report.checks.len()
                    );
                    if report.overall_pass {
                        Ok(SelfExecuteOutcome::Success {
                            message: msg,
                            artifact: None,
                        })
                    } else {
                        Ok(SelfExecuteOutcome::Failed { reason: msg })
                    }
                }
                Err(e) => Ok(SelfExecuteOutcome::Failed {
                    reason: format!("validation execution error: {}", e),
                }),
            }
        }
        "noop" => Ok(SelfExecuteOutcome::Success {
            message: "noop handler executed successfully".into(),
            artifact: None,
        }),
        _ => Err(format!("unknown self_execute handler: {}", handler)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_handler() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = run_self_execute("noop", &serde_json::json!({}), tmp.path()).await;
        assert!(result.is_ok());
        match result.unwrap() {
            SelfExecuteOutcome::Success { message, .. } => {
                assert!(message.contains("noop"));
            }
            _ => panic!("expected Success"),
        }
    }

    #[tokio::test]
    async fn test_unknown_handler() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = run_self_execute("bogus", &serde_json::json!({}), tmp.path()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown"));
    }

    #[tokio::test]
    async fn test_validate_delivery_handler_without_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No Cargo.toml — should still not panic
        let result = run_self_execute(
            "validate_delivery",
            &serde_json::json!({"ctrt_id": null, "run_lint": false}),
            tmp.path(),
        )
        .await;
        assert!(result.is_ok());
    }
}
