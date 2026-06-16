//! Lint gate: run project-appropriate linter and produce CheckResult.
//!
//! Phase 1:
//! | project_type | command |
//! |--------------|---------|
//! | rust         | `cargo clippy -- -D warnings` (timeout 300s) |
//! | python       | `ruff check .` (if missing → skip with warning) |
//! | node         | `npm run lint` (if no script → skip with warning) |

use std::path::Path;

use crate::tool::command_ops::{execute_with_timeout, get_shell};
use crate::validate::report::CheckResult;

/// Run the appropriate linter for the project type.
pub async fn run_lint(working_dir: &Path) -> CheckResult {
    let project_type = detect_project_type(working_dir);

    let (cmd, wait_for_it) = match project_type.as_str() {
        "rust" => ("cargo clippy -- -D warnings".to_string(), true),
        "node" => {
            if !npm_lint_script_exists(working_dir).await {
                return CheckResult {
                    name: "lint".into(),
                    pass: true,
                    summary: "package.json has no lint script, skipping lint".to_string(),
                    details: serde_json::json!({"skipped": true, "reason": "no_lint_script"}),
                };
            }
            ("npm run lint".to_string(), true)
        }
        "python" => {
            if !ruff_available(working_dir).await {
                return CheckResult {
                    name: "lint".into(),
                    pass: true,
                    summary: "ruff not installed, skipping lint".to_string(),
                    details: serde_json::json!({"skipped": true, "reason": "ruff_not_found", "warning": true}),
                };
            }
            ("ruff check .".to_string(), true)
        }
        _ => {
            return CheckResult {
                name: "lint".into(),
                pass: true,
                summary: format!("Unsupported project type {}, skipping lint", project_type),
                details: serde_json::json!({"skipped": true}),
            };
        }
    };

    if !wait_for_it {
        return CheckResult {
            name: "lint".into(),
            pass: true,
            summary: "lint skipped".to_string(),
            details: serde_json::json!({"skipped": true}),
        };
    }

    let timeout = std::time::Duration::from_secs(300);
    let (shell, shell_args) = get_shell();

    match execute_with_timeout(shell, &shell_args, &cmd, working_dir, timeout).await {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let pass = exit_code == 0;

            // Extract warning/error counts
            let warning_count = stdout.lines().filter(|l| l.contains("warning")).count()
                + stderr.lines().filter(|l| l.contains("warning")).count();
            let error_count = stdout.lines().filter(|l| l.contains("error")).count()
                + stderr.lines().filter(|l| l.contains("error")).count();

            CheckResult {
                name: "lint".into(),
                pass,
                summary: if pass {
                    "lint passed".to_string()
                } else {
                    format!("lint failed (exit={}, errors={})", exit_code, error_count)
                },
                details: serde_json::json!({
                    "exit_code": exit_code,
                    "warning_count": warning_count,
                    "error_count": error_count,
                    "stdout_truncated": stdout.chars().take(2000).collect::<String>(),
                    "stderr_truncated": stderr.chars().take(2000).collect::<String>(),
                }),
            }
        }
        Err(e) => CheckResult {
            name: "lint".into(),
            pass: false,
            summary: format!("lint execution failed: {}", e),
            details: serde_json::json!({"error": e}),
        },
    }
}

fn detect_project_type(working_dir: &Path) -> String {
    if working_dir.join("Cargo.toml").exists() {
        "rust".to_string()
    } else if working_dir.join("package.json").exists() {
        "node".to_string()
    } else if working_dir.join("pyproject.toml").exists() || working_dir.join("setup.py").exists() {
        "python".to_string()
    } else {
        "unknown".to_string()
    }
}

async fn npm_lint_script_exists(working_dir: &Path) -> bool {
    let pkg_path = working_dir.join("package.json");
    let content = match tokio::fs::read_to_string(&pkg_path).await {
        Ok(c) => c,
        Err(_) => return false,
    };
    // Simple check: does the JSON contain "lint" in the scripts section?
    content.contains("\"lint\"")
}

async fn ruff_available(working_dir: &Path) -> bool {
    // Check if ruff is installed globally or in project venv
    let ruff_check = working_dir.join(".venv").join("bin").join("ruff");
    let ruff_check_win = working_dir.join(".venv").join("Scripts").join("ruff.exe");

    if ruff_check.exists() || ruff_check_win.exists() {
        return true;
    }

    // Try `ruff --version` as fallback
    let (shell, shell_args) = get_shell();
    execute_with_timeout(
        shell,
        &shell_args,
        "ruff --version",
        working_dir,
        std::time::Duration::from_secs(10),
    )
    .await
    .map(|o| o.status.success())
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rust_lint_on_empty_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();

        tokio::fs::write(
            dir.join("Cargo.toml"),
            r#"
[package]
name = "test_lint"
version = "0.1.0"
edition = "2021"
"#,
        )
        .await
        .unwrap();

        let src = dir.join("src");
        tokio::fs::create_dir_all(&src).await.unwrap();
        tokio::fs::write(src.join("lib.rs"), "pub fn fine() -> i32 { 42 }")
            .await
            .unwrap();

        let result = run_lint(dir).await;
        // May pass or fail depending on clippy availability, but should not panic
        assert_eq!(result.name, "lint");
    }

    #[test]
    fn test_detect_project_type_rust() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_project_type(tmp.path()), "rust");
    }

    #[test]
    fn test_detect_project_type_node() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_project_type(tmp.path()), "node");
    }
}
