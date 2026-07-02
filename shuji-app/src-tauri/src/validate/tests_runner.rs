//! Test gate runner. Reuses logic from `command_ops.rs` for execution but
//! returns structured `CheckResult` for the validation pipeline.

use std::path::Path;

use crate::tool::command_ops::{
    detect_project_type, execute_with_timeout, get_shell, parse_test_output,
};
use crate::tool::python_cmd::pytest_cmd;
use crate::validate::report::{CheckResult, ValidateConfig};

/// Run test gate: execute tests and produce a structured CheckResult.
///
/// Rules:
/// - exit_code != 0 → fail
/// - fail_count > 0 → fail
/// - stdout contains "skipped" and config.forbid_unexplained_skip → fail
pub async fn run_test_gate(working_dir: &Path, config: &ValidateConfig) -> CheckResult {
    let project_type = detect_project_type(working_dir);
    let scope = &config.tests.scope;

    let cmd = build_test_cmd(&project_type, scope, working_dir);

    let timeout = std::time::Duration::from_secs(300);
    let (shell, shell_args) = get_shell();

    match execute_with_timeout(shell, &shell_args, &cmd, working_dir, timeout).await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code().unwrap_or(-1);

            let (parsed_pass, parsed_fail, parsed_total) = parse_test_output(&stdout, &stderr);
            let fail_count = parsed_fail.unwrap_or(0);

            let mut pass = true;
            let mut details = serde_json::json!({
                "exit_code": exit_code,
                "pass_count": parsed_pass.unwrap_or(0),
                "fail_count": fail_count,
                "total_count": parsed_total.unwrap_or(0),
                "project_type": project_type,
                "command": cmd,
            });

            let mut summary = String::new();

            if exit_code != 0 {
                pass = false;
                summary.push_str(&format!("exit_code={}", exit_code));
            }

            if fail_count > 0 {
                pass = false;
                if !summary.is_empty() {
                    summary.push_str("; ");
                }
                summary.push_str(&format!("{} test(s) failed", fail_count));

                // Extract failed test names
                let failed: Vec<&str> = stdout
                    .lines()
                    .filter(|l| l.contains("FAILED") || l.contains("... FAILED"))
                    .collect();
                if let Some(obj) = details.as_object_mut() {
                    obj.insert(
                        "failed_tests".into(),
                        serde_json::Value::Array(
                            failed
                                .iter()
                                .map(|f| serde_json::Value::String(f.trim().to_string()))
                                .collect(),
                        ),
                    );
                }
            }

            // Check for unexplained skips
            if config.tests.forbid_unexplained_skip && stdout.contains("skipped") {
                // Simple heuristic: check if "skipped" appears in test result line
                let skip_line = stdout
                    .lines()
                    .find(|l| l.contains("skipped") && l.contains("test result"))
                    .unwrap_or("");
                if !skip_line.contains("0 skipped") {
                    pass = false;
                    if !summary.is_empty() {
                        summary.push_str("; ");
                    }
                    summary.push_str("has unexplained skip cases");
                }
            }

            if pass {
                summary = "all tests passed".to_string();
            }

            CheckResult {
                name: "tests".into(),
                pass,
                summary,
                details,
            }
        }
        Err(e) => CheckResult {
            name: "tests".into(),
            pass: false,
            summary: format!("test execution failed: {}", e),
            details: serde_json::json!({"error": e}),
        },
    }
}

/// Build the test command based on project type and scope.
fn build_test_cmd(project_type: &str, scope: &str, working_dir: &Path) -> String {
    match project_type {
        "rust" => match scope {
            "unit" => "cargo test --lib".to_string(),
            "integration" => "cargo test --tests".to_string(),
            _ => "cargo test".to_string(),
        },
        "node" => {
            let has_vitest = working_dir.join("node_modules/.bin/vitest").exists();
            let has_jest = working_dir.join("node_modules/.bin/jest").exists();
            if has_vitest {
                format!("npx vitest run{}", scope_suffix(scope))
            } else if has_jest {
                format!("npx jest --verbose{}", scope_suffix(scope))
            } else {
                "npm test".to_string()
            }
        }
        "python" => pytest_cmd(scope),
        _ => {
            // unknown project type — try common commands
            "cargo test".to_string()
        }
    }
}

fn scope_suffix(scope: &str) -> &str {
    match scope {
        "unit" => " tests/",
        "integration" => " tests/integration/",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::report::ValidateConfig;

    #[tokio::test]
    async fn test_run_test_gate_pass_on_valid_cargo_project() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        let dir = tmp.path();

        tokio::fs::write(
            dir.join("Cargo.toml"),
            r#"
[package]
name = "test_crate"
version = "0.1.0"
edition = "2021"
"#,
        )
        .await?;

        let src = dir.join("src");
        tokio::fs::create_dir_all(&src).await?;
        tokio::fs::write(
            src.join("lib.rs"),
            r#"
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
"#,
        )
        .await?;

        let config = ValidateConfig::default();
        let result = run_test_gate(dir, &config).await;
        assert!(result.pass, "test should pass: {}", result.summary);
        Ok(())
    }

    #[tokio::test]
    async fn test_run_test_gate_fail_on_broken_test() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        let dir = tmp.path();

        tokio::fs::write(
            dir.join("Cargo.toml"),
            r#"
[package]
name = "test_crate"
version = "0.1.0"
edition = "2021"
"#,
        )
        .await?;

        let src = dir.join("src");
        tokio::fs::create_dir_all(&src).await?;
        tokio::fs::write(
            src.join("lib.rs"),
            r#"
#[cfg(test)]
mod tests {
    #[test]
    fn it_fails() {
        assert_eq!(2 + 2, 5);
    }
}
"#,
        )
        .await?;

        let config = ValidateConfig::default();
        let result = run_test_gate(dir, &config).await;
        assert!(!result.pass, "test should fail");
        assert!(
            result.details["failed_tests"]
                .as_array()
                .is_some_and(|a| !a.is_empty()),
            "should list failed test names"
        );
        Ok(())
    }
}
