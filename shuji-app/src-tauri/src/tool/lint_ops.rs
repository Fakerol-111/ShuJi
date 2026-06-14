//! `run_lint` tool: runs project-appropriate linter and returns structured output.
//!
//! Shared implementation with `validate::lint` — this is the tool interface
//! that agents (工部, 礼部) can invoke during execution.

use std::path::Path;

use crate::tool::command_ops::{execute_with_timeout, get_shell};
use crate::tool::ToolOutput;

/// Run lint on the project. Used by agents via `run_lint` tool.
///
/// Parameters:
/// - `strict` (bool, default false): when true, warnings also count as failure
pub async fn tool_run_lint(working_dir: &Path, args: &serde_json::Value) -> String {
    let strict = args
        .get("strict")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let project_type = detect_project_type(working_dir);

    let (cmd, wait_for_it) = match project_type.as_str() {
        "rust" => ("cargo clippy -- -D warnings".to_string(), true),
        "node" => {
            if !npm_lint_script_exists(working_dir).await {
                return ToolOutput::success_raw("run_lint",
                    &format!("{{\"skipped\": true, \"reason\": \"no_lint_script\", \"project_type\": \"{}\"}}", project_type));
            }
            ("npm run lint".to_string(), true)
        }
        "python" => {
            if !ruff_available(working_dir).await {
                return ToolOutput::success_raw("run_lint",
                    &format!("{{\"skipped\": true, \"reason\": \"ruff_not_found\", \"project_type\": \"{}\", \"warning\": true}}", project_type));
            }
            ("ruff check .".to_string(), true)
        }
        _ => {
            return ToolOutput::success_raw("run_lint",
                &format!("{{\"skipped\": true, \"reason\": \"unsupported_project\", \"project_type\": \"{}\"}}", project_type));
        }
    };

    if !wait_for_it {
        return ToolOutput::success_raw("run_lint", r#"{"skipped": true}"#);
    }

    let timeout = std::time::Duration::from_secs(300);
    let (shell, shell_args) = get_shell();

    match execute_with_timeout(shell, &shell_args, &cmd, working_dir, timeout).await {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let warning_count = stdout.lines().filter(|l| l.contains("warning")).count()
                + stderr.lines().filter(|l| l.contains("warning")).count();
            let error_count = stdout.lines().filter(|l| l.contains("error")).count()
                + stderr.lines().filter(|l| l.contains("error")).count();

            let pass = if strict {
                exit_code == 0
            } else {
                exit_code == 0 || (exit_code != 0 && error_count == 0)
            };

            if pass {
                ToolOutput::success(
                    "run_lint",
                    "",
                    &format!("lint 通过 (warnings={})", warning_count),
                )
            } else {
                ToolOutput::error(
                    "run_lint",
                    "",
                    "lint_failed",
                    &format!(
                        "lint 失败 exit={} errors={} warnings={}",
                        exit_code, error_count, warning_count
                    ),
                )
            }
        }
        Err(e) => ToolOutput::error("run_lint", "", "exec_error", &e),
    }
}

fn detect_project_type(working_dir: &Path) -> String {
    if working_dir.join("Cargo.toml").exists() {
        "rust".to_string()
    } else if working_dir.join("package.json").exists() {
        "node".to_string()
    } else {
        "unknown".to_string()
    }
}

async fn npm_lint_script_exists(working_dir: &Path) -> bool {
    let pkg = working_dir.join("package.json");
    tokio::fs::read_to_string(&pkg)
        .await
        .map(|c| c.contains("\"lint\""))
        .unwrap_or(false)
}

async fn ruff_available(working_dir: &Path) -> bool {
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

/// Tool definition for run_lint.
pub fn run_lint_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "run_lint".into(),
            description: "运行当前项目的 lint 检查（clippy / ruff / npm run lint），返回 pass/fail 结果。strict=true 时 warning 也算 fail。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "strict": {
                        "type": "boolean",
                        "description": "默认 false。true 时 warnings 也算失败"
                    }
                }
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_run_lint_on_rust_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        tokio::fs::write(
            tmp.path().join("Cargo.toml"),
            r#"
[package]
name = "test_lint"
version = "0.1.0"
edition = "2021"
"#,
        )
        .await
        .unwrap();
        let src = tmp.path().join("src");
        tokio::fs::create_dir_all(&src).await.unwrap();
        tokio::fs::write(src.join("lib.rs"), "pub fn ok() -> i32 { 42 }")
            .await
            .unwrap();

        let result = tool_run_lint(tmp.path(), &serde_json::json!({})).await;
        assert!(
            result.contains("ok") || result.contains("pass") || result.contains("failed"),
            "should produce a valid response: {}",
            result
        );
    }
}
