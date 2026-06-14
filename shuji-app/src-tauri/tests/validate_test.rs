//! Integration tests for validate_delivery.
//!
//! Tests the end-to-end validation pipeline: test gate, contract diff,
//! lint, persistence, and audit logging.

use std::path::Path;

use shuji_app_lib::validate::delivery::validate_delivery;
use shuji_app_lib::validate::report::{DeliveryOptions, ValidationReport};

/// Helper: create a minimal Rust crate at the given path.
async fn create_mini_crate(dir: &Path) {
    tokio::fs::write(
        dir.join("Cargo.toml"),
        r#"
[package]
name = "test_crate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .await
    .unwrap();
    let src = dir.join("src");
    tokio::fs::create_dir_all(&src).await.unwrap();
    tokio::fs::write(
        src.join("lib.rs"),
        r#"
pub fn greet(name: &str) -> String {
    format!("Hello, {}", name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_greet() {
        assert_eq!(greet("world"), "Hello, world");
    }
}
"#,
    )
    .await
    .unwrap();
}

/// Test that validate_delivery passes on a valid Rust project with all tests.
#[tokio::test]
async fn test_validate_delivery_pass() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_mini_crate(tmp.path()).await;

    let opts = DeliveryOptions::default();
    let report = validate_delivery(tmp.path(), &opts).await.unwrap();
    assert!(
        report.overall_pass,
        "delivery should pass: {:?}",
        report.checks
    );
    assert_eq!(report.project_type, "rust");
    assert!(report.checks.iter().any(|c| c.name == "tests" && c.pass));
}

/// Test that validate_delivery detects test failures.
#[tokio::test]
async fn test_validate_delivery_fail_on_broken_tests() {
    let tmp = tempfile::TempDir::new().unwrap();

    tokio::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"
[package]
name = "broken"
version = "0.1.0"
edition = "2021"
"#,
    )
    .await
    .unwrap();
    let src = tmp.path().join("src");
    tokio::fs::create_dir_all(&src).await.unwrap();
    tokio::fs::write(
        src.join("lib.rs"),
        r#"
#[test]
fn broken() {
    assert_eq!(1, 2);
}
"#,
    )
    .await
    .unwrap();

    let opts = DeliveryOptions::default();
    let report = validate_delivery(tmp.path(), &opts).await.unwrap();
    assert!(!report.overall_pass, "broken tests should fail validation");
}

/// Test that report is persisted to `.shuji/validate/latest.json`.
#[tokio::test]
async fn test_validate_persists_report() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_mini_crate(tmp.path()).await;

    let opts = DeliveryOptions::default();
    validate_delivery(tmp.path(), &opts).await.unwrap();

    let report_path = tmp
        .path()
        .join(".shuji")
        .join("validate")
        .join("latest.json");
    assert!(report_path.exists(), "report should be persisted");

    let content = tokio::fs::read_to_string(&report_path).await.unwrap();
    let report: ValidationReport = serde_json::from_str(&content).unwrap();
    assert_eq!(report.project_type, "rust");
}

/// Test that audit event is created.
#[tokio::test]
async fn test_validate_creates_audit_event() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_mini_crate(tmp.path()).await;

    let opts = DeliveryOptions::default();
    validate_delivery(tmp.path(), &opts).await.unwrap();

    let audit_path = tmp.path().join(".shuji").join("audit.jsonl");
    assert!(audit_path.exists(), "audit log should exist");
    let content = tokio::fs::read_to_string(&audit_path).await.unwrap();
    assert!(
        content.contains("validate_delivery"),
        "audit should contain validate_delivery event"
    );
}

/// Test that validation works on empty/unknown project type.
#[tokio::test]
async fn test_validate_unknown_project_type() {
    let tmp = tempfile::TempDir::new().unwrap();
    // No Cargo.toml or package.json — unknown type
    let opts = DeliveryOptions::default();
    let report = validate_delivery(tmp.path(), &opts).await.unwrap();
    assert!(
        !report.overall_pass,
        "unknown project without tests should fail"
    );
    assert_eq!(report.project_type, "unknown");
}
