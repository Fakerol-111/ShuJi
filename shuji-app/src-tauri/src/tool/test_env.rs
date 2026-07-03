//! `setup_test_env` tool: prepares test environment for different project types.
//!
//! Supports:
//! - python: `python3 -m venv .venv` (or `python`) + pip install
//! - node: `npm ci` or `npm install`
//! - rust: `cargo fetch` (optional, skip by default)

use std::path::Path;

use crate::tool::command_ops::{execute_with_timeout, get_shell};
use crate::tool::python_cmd::{python_command, venv_create_cmd, venv_python_or_system};
use crate::tool::ToolOutput;

/// Set up test environment for the project.
///
/// Parameters:
/// - `force` (bool, default false): re-create environment even if it exists
pub async fn tool_setup_test_env(working_dir: &Path, args: &serde_json::Value) -> String {
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    let project_type = detect_project_type(working_dir);
    let (shell, shell_args) = get_shell();

    match project_type.as_str() {
        "rust" => {
            // Rust: cargo fetch (optional, only if Cargo.lock absent)
            if !working_dir.join("Cargo.lock").exists() || force {
                let cmd = "cargo fetch";
                let timeout = std::time::Duration::from_secs(600);
                match execute_with_timeout(shell, &shell_args, cmd, working_dir, timeout).await {
                    Ok(o) if o.status.success() => {
                        ToolOutput::success("setup_test_env", "", "cargo fetch completed")
                    }
                    Ok(o) => ToolOutput::error(
                        "setup_test_env",
                        "",
                        "fetch_failed",
                        &format!("cargo fetch failed exit={}", o.status.code().unwrap_or(-1)),
                    ),
                    Err(e) => ToolOutput::error("setup_test_env", "", "exec_error", &e),
                }
            } else {
                ToolOutput::success(
                    "setup_test_env",
                    "",
                    "rust environment ready (Cargo.lock exists)",
                )
            }
        }
        "node" => {
            let has_lock = working_dir.join("package-lock.json").exists();
            let has_node_modules = working_dir.join("node_modules").exists();

            if has_node_modules && !force {
                return ToolOutput::success("setup_test_env", "", "node_modules already exists");
            }

            let cmd = if has_lock { "npm ci" } else { "npm install" };
            let timeout = std::time::Duration::from_secs(600);
            match execute_with_timeout(shell, &shell_args, cmd, working_dir, timeout).await {
                Ok(o) if o.status.success() => {
                    ToolOutput::success("setup_test_env", "", &format!("{} completed", cmd))
                }
                Ok(o) => ToolOutput::error(
                    "setup_test_env",
                    "",
                    "install_failed",
                    &format!("{} failed exit={}", cmd, o.status.code().unwrap_or(-1)),
                ),
                Err(e) => ToolOutput::error("setup_test_env", "", "exec_error", &e),
            }
        }
        "python" => {
            let venv_dir = working_dir.join(".venv");
            let venv_bin = if cfg!(windows) {
                venv_dir.join("Scripts").join("python")
            } else {
                venv_dir.join("bin").join("python")
            };

            if venv_bin.exists() && !force {
                return ToolOutput::success("setup_test_env", "", ".venv already exists");
            }

            // Create venv
            let venv_cmd = venv_create_cmd();
            let py = python_command();
            let timeout = std::time::Duration::from_secs(300);
            match execute_with_timeout(shell, &shell_args, &venv_cmd, working_dir, timeout).await {
                Ok(o) if !o.status.success() => {
                    return ToolOutput::error(
                        "setup_test_env",
                        "",
                        "venv_failed",
                        &format!("{py} -m venv failed exit={}", o.status.code().unwrap_or(-1)),
                    );
                }
                Err(e) => {
                    return ToolOutput::error("setup_test_env", "", "exec_error", &e);
                }
                _ => {}
            }

            // Try pip install editable
            let pip_install = if cfg!(windows) {
                ".venv/Scripts/pip install -e ."
            } else {
                ".venv/bin/pip install -e ."
            };
            let timeout = std::time::Duration::from_secs(300);
            match execute_with_timeout(shell, &shell_args, pip_install, working_dir, timeout).await
            {
                Ok(o) if o.status.success() => ToolOutput::success(
                    "setup_test_env",
                    "",
                    &format!(
                        "python venv created + pip install. Tests will use: {}",
                        venv_python_or_system(working_dir)
                    ),
                ),
                Ok(_) => {
                    // Try with [dev] extra
                    let pip_install_dev = if cfg!(windows) {
                        ".venv/Scripts/pip install -e \".[dev]\""
                    } else {
                        ".venv/bin/pip install -e '.[dev]'"
                    };
                    match execute_with_timeout(
                        shell,
                        &shell_args,
                        pip_install_dev,
                        working_dir,
                        timeout,
                    )
                    .await
                    {
                        Ok(o) if o.status.success() => ToolOutput::success(
                            "setup_test_env",
                            "",
                            &format!("python venv created + pip install -e .[dev]. Tests will use: {}", venv_python_or_system(working_dir)),
                        ),
                        Ok(_) => ToolOutput::success(
                            "setup_test_env",
                            "",
                            "python venv created (pip install skipped or failed, environment unaffected)",
                        ),
                        Err(e) => ToolOutput::error("setup_test_env", "", "exec_error", &e),
                    }
                }
                Err(e) => ToolOutput::error("setup_test_env", "", "exec_error", &e),
            }
        }
        _ => ToolOutput::success_raw(
            "setup_test_env",
            &format!(
                "{{\"skipped\": true, \"reason\": \"unknown_project_type: {}\"}}",
                project_type
            ),
        ),
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

/// Tool definition for setup_test_env.
pub fn setup_test_env_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "setup_test_env".into(),
            description:
                "Set up the project test environment (create venv / npm install / cargo fetch). force=true forces recreation even if it already exists."
                    .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "force": {
                        "type": "boolean",
                        "description": "Default false. When true, recreate the environment even if it already exists"
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
    async fn test_setup_test_env_no_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = tool_setup_test_env(tmp.path(), &serde_json::json!({})).await;
        assert!(
            result.contains("unknown"),
            "unknown project should be skipped: {}",
            result
        );
    }
}
