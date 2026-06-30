//! Tauri command for validate_delivery.
//!
//! Provides a command that UI or CLI can invoke to run delivery validation.

use crate::validate::{validate_delivery, DeliveryOptions, ValidationReport};

#[tauri::command]
pub async fn validate_delivery_cmd(
    project_dir: String,
    ctrt_id: Option<String>,
    run_lint: Option<bool>,
    run_contract_diff: Option<bool>,
    test_scope: Option<String>,
) -> Result<ValidationReport, String> {
    let opts = DeliveryOptions {
        ctrt_id,
        run_contract_diff: run_contract_diff.unwrap_or(false),
        run_lint: run_lint.unwrap_or(false),
        test_scope: test_scope.unwrap_or_else(|| "all".to_string()),
    };

    validate_delivery(std::path::Path::new(&project_dir), &opts).await
}
