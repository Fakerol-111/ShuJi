//! ValidationReport and CheckResult types for mechanical validation.
//!
//! These types are serialized to `.shuji/validate/latest.json` and consumed
//! by Pipeline's `self_execute(handler="validate_delivery")`, 礼部 audit,
//! and frontend Dashboard (PART-05).

use serde::{Deserialize, Serialize};

/// Overall validation report for a delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub ts: String,
    pub project_type: String,
    pub overall_pass: bool,
    pub checks: Vec<CheckResult>,
    pub ctrt_id: Option<String>,
}

/// A single check item within a validation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub pass: bool,
    pub summary: String,
    pub details: serde_json::Value,
}

/// Options controlling what validation gates to run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryOptions {
    pub ctrt_id: Option<String>,
    pub run_contract_diff: bool,
    pub run_lint: bool,
    pub test_scope: String,
}

impl Default for DeliveryOptions {
    fn default() -> Self {
        Self {
            ctrt_id: None,
            run_contract_diff: false,
            run_lint: false,
            test_scope: "all".to_string(),
        }
    }
}

///Project-level validate config loaded from `.shuji/validate_config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfig {
    pub enabled: bool,
    pub tests: TestConfig,
    pub contract_diff: ContractDiffConfig,
    pub lint: LintConfig,
}

impl Default for ValidateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tests: TestConfig {
                required: true,
                scope: "all".to_string(),
                forbid_unexplained_skip: false,
            },
            contract_diff: ContractDiffConfig {
                enabled: false,
                ctrt_id: None,
            },
            lint: LintConfig {
                enabled: false,
                fail_on_warning: true,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub required: bool,
    pub scope: String,
    pub forbid_unexplained_skip: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDiffConfig {
    pub enabled: bool,
    pub ctrt_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintConfig {
    pub enabled: bool,
    pub fail_on_warning: bool,
}

// ── Test helpers ─────────────────────────────────────────────

pub fn default_validate_config() -> ValidateConfig {
    ValidateConfig::default()
}

pub fn load_validate_config_json(config_json: &str) -> ValidateConfig {
    serde_json::from_str(config_json).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_report_roundtrip() -> anyhow::Result<()> {
        let report = ValidationReport {
            ts: "2026-06-13T12:00:00".into(),
            project_type: "rust".into(),
            overall_pass: true,
            checks: vec![CheckResult {
                name: "tests".into(),
                pass: true,
                summary: "all tests passed".into(),
                details: serde_json::json!({"passed": 10, "failed": 0}),
            }],
            ctrt_id: None,
        };

        let json = serde_json::to_string(&report)?;
        let deserialized: ValidationReport = serde_json::from_str(&json)?;
        assert!(deserialized.overall_pass);
        assert_eq!(deserialized.checks.len(), 1);
        assert_eq!(deserialized.project_type, "rust");
        Ok(())
    }

    #[test]
    fn test_overall_pass_all_checks_pass() {
        let report = ValidationReport {
            ts: "".into(),
            project_type: "rust".into(),
            overall_pass: true,
            checks: vec![
                CheckResult {
                    name: "tests".into(),
                    pass: true,
                    summary: "".into(),
                    details: serde_json::json!({}),
                },
                CheckResult {
                    name: "lint".into(),
                    pass: true,
                    summary: "".into(),
                    details: serde_json::json!({}),
                },
            ],
            ctrt_id: None,
        };
        assert!(report.checks.iter().all(|c| c.pass));
    }

    #[test]
    fn test_overall_pass_any_check_fails() {
        let report = ValidationReport {
            ts: "".into(),
            project_type: "rust".into(),
            overall_pass: true, // stored field; logic is in delivery
            checks: vec![
                CheckResult {
                    name: "tests".into(),
                    pass: true,
                    summary: "".into(),
                    details: serde_json::json!({}),
                },
                CheckResult {
                    name: "lint".into(),
                    pass: false,
                    summary: "clippy error".into(),
                    details: serde_json::json!({}),
                },
            ],
            ctrt_id: None,
        };
        // overall_pass must be false if any check fails
        assert!(!report.checks.iter().all(|c| c.pass));
    }

    #[test]
    fn test_validate_config_default() {
        let config = ValidateConfig::default();
        assert!(config.enabled);
        assert!(config.tests.required);
        assert!(!config.contract_diff.enabled);
        assert!(!config.lint.enabled);
    }

    #[test]
    fn test_validate_config_deserialize() -> anyhow::Result<()> {
        let json = r#"{
            "enabled": true,
            "tests": { "required": true, "scope": "unit", "forbid_unexplained_skip": true },
            "contract_diff": { "enabled": true, "ctrt_id": "ctrt_001" },
            "lint": { "enabled": true, "fail_on_warning": false }
        }"#;
        let config: ValidateConfig = serde_json::from_str(json)?;
        assert!(config.tests.forbid_unexplained_skip);
        assert_eq!(config.contract_diff.ctrt_id, Some("ctrt_001".into()));
        assert!(config.lint.enabled);
        Ok(())
    }

    #[test]
    fn test_delivery_options_default() {
        let opts = DeliveryOptions::default();
        assert_eq!(opts.test_scope, "all");
        assert!(!opts.run_lint);
        assert!(!opts.run_contract_diff);
        assert!(opts.ctrt_id.is_none());
    }
}
