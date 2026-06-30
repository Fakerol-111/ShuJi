use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncReadExt;

use crate::tool::path::check_command_safety;
use crate::tool::python_cmd::pytest_cmd;
use crate::tool::ToolOutput;

/// Get the current platform's shell command.
/// Windows -> powershell; others -> bash (fallback to sh)
pub(crate) fn get_shell() -> (&'static str, Vec<&'static str>) {
    if cfg!(windows) {
        ("powershell", vec!["-Command"])
    } else if std::process::Command::new("bash")
        .arg("--version")
        .output()
        .is_ok()
    {
        ("bash", vec!["-l", "-c"])
    } else {
        ("sh", vec!["-c"])
    }
}

// ── execute_command ───────────────────────────────────────────

/// Execute a shell command with timeout and safety checks.
pub async fn tool_execute_command(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let cmd = args["command"].as_str().unwrap_or("");
    if cmd.is_empty() {
        return ToolOutput::error("execute_command", "", "empty_command", "Command is empty");
    }
    log_console!("[{}] executing: {}", dept, cmd);

    if let Err(blocked) = check_safe_command(cmd) {
        log_console!("[{}] BLOCKED command: {} — reason: {}", dept, cmd, blocked);
        return ToolOutput::error(
            "execute_command",
            cmd,
            "command_not_allowed",
            &format!(
                "Command prohibited: {blocked}. Use run_tests/run_lint or an approved build/format command instead."
            ),
        );
    }

    let timeout = std::time::Duration::from_secs(120);
    let (shell, shell_args) = get_shell();
    match execute_with_timeout(shell, &shell_args, cmd, working_dir, timeout).await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code().unwrap_or(-1);
            if exit_code == 0 {
                format!(
                    "Command executed successfully (exit={}):\n{}",
                    exit_code, stdout
                )
            } else {
                format!(
                    "Command execution failed (exit={}):\nstdout:\n{}\nstderr:\n{}",
                    exit_code, stdout, stderr
                )
            }
        }
        Err(timeout_msg) => timeout_msg,
    }
}

/// Execute a process with a timeout, capturing stdout/stderr.
pub async fn execute_with_timeout(
    shell: &str,
    args: &[&str],
    cmd: &str,
    working_dir: &Path,
    timeout: std::time::Duration,
) -> Result<std::process::Output, String> {
    let mut child = tokio::process::Command::new(shell);
    for a in args {
        child.arg(a);
    }
    child.arg(cmd);
    let mut child = child
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start command: {}", e))?;

    let start = tokio::time::Instant::now();
    let poll_interval = tokio::time::Duration::from_millis(500);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(ref mut out) = child.stdout {
                    let _ = out.read_to_end(&mut stdout).await;
                }
                if let Some(ref mut err) = child.stderr {
                    let _ = err.read_to_end(&mut stderr).await;
                }
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    return Err(format!(
                        "Command execution timed out (exceeded {} seconds), process terminated.",
                        timeout.as_secs()
                    ));
                }
                tokio::time::sleep(poll_interval).await;
            }
            Err(e) => return Err(format!("Command execution error: {}", e)),
        }
    }
}

/// Check if a command is safe to execute.
/// Blocks dangerous system commands and path escape patterns.
fn check_safe_command(cmd: &str) -> Result<(), &'static str> {
    check_command_safety(cmd)
}

// ── run_tests ─────────────────────────────────────────────────

/// Runs tests with auto-detected project type and structured output.
/// Reduces LLM command-typing errors that trigger watchdog.
pub async fn tool_run_tests(working_dir: &Path, args: &serde_json::Value) -> String {
    let scope = args["scope"].as_str().unwrap_or("all");
    let path = args["path"].as_str().filter(|s| !s.is_empty());

    // Detect project type
    let project_type = detect_project_type(working_dir);
    let mut cmd = match project_type.as_str() {
        "rust" => match scope {
            "unit" => "cargo test --lib".to_string(),
            "integration" => "cargo test --tests".to_string(),
            _ => "cargo test".to_string(),
        },
        "node" => {
            // Use project's test script or common runners
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
            return ToolOutput::success_raw(
                "run_tests",
                "Unable to detect known project type (Cargo.toml / package.json / pyproject.toml). Cannot determine test command. Use execute_command for custom commands.",
            );
        }
    };

    // Append specific test file path if provided
    if let Some(p) = path {
        // Scope check: reject unit path with integration scope and vice versa
        if scope == "integration" && !p.contains("integration") {
            return ToolOutput::error(
                "run_tests",
                "",
                "scope_mismatch",
                &format!(
                    "scope=integration but path {} does not match integration test directory (tests/integration/)",
                    p
                ),
            );
        }
        cmd.push_str(&format!(" -- {}", p));
    }

    log_console!("[run_tests] executing: {}", cmd);
    let timeout = std::time::Duration::from_secs(300);

    let (shell, shell_args) = get_shell();

    let output = match execute_with_timeout(shell, &shell_args, &cmd, working_dir, timeout).await {
        Ok(o) => o,
        Err(e) => return ToolOutput::error("run_tests", "", "exec_error", &e),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    // Parse test results from output
    let (parsed_pass, parsed_fail, parsed_total) = parse_test_output(&stdout, &stderr);

    let pass_count = parsed_pass.unwrap_or(0);
    let fail_count = parsed_fail.unwrap_or(0);
    let total_count = parsed_total.unwrap_or(0);

    // Extract failed test names from output
    let failed_tests: Vec<&str> = stdout
        .lines()
        .filter(|l| l.contains("FAILED") || l.contains("... FAILED"))
        .map(|l| {
            l.trim()
                .trim_start_matches("test ")
                .trim_end_matches(" ... FAILED")
                .trim_end_matches(" FAILED")
        })
        .collect();

    // Build structured result
    let mut report = String::new();
    report.push_str(&format!(
        "## Test Execution Report\n\nProject type: {} | Scope: {} | Command: `{}`\n",
        project_type, scope, cmd
    ));
    report.push_str(&format!(
        "Exit code: {} | Passed: {} | Failed: {}",
        exit_code, pass_count, fail_count,
    ));
    if total_count > 0 {
        report.push_str(&format!(" | Total: {}", total_count));
    }
    report.push('\n');

    if !failed_tests.is_empty() {
        report.push_str("\n### Failed Tests\n");
        for t in &failed_tests {
            report.push_str(&format!("- {}\n", t));
        }
    }

    if exit_code != 0 {
        // Truncate stderr to avoid context overflow
        let stderr_trimmed = if stderr.len() > 2000 {
            let cutoff = stderr.floor_char_boundary(2000);
            format!(
                "{}...\n[Truncated: showing first {} chars, total {} chars]",
                &stderr[..cutoff],
                cutoff,
                stderr.len()
            )
        } else {
            stderr.to_string()
        };
        if !stderr_trimmed.is_empty() {
            report.push_str(&format!("\n### stderr Summary\n{}", stderr_trimmed));
        }
    }

    if exit_code == 0 && failed_tests.is_empty() {
        report.push_str("\nAll tests passed");
    }

    ToolOutput::success_raw("run_tests", &report)
}

/// Detect project type by checking for key files.
pub(crate) fn detect_project_type(working_dir: &Path) -> String {
    if working_dir.join("Cargo.toml").exists() {
        "rust".to_string()
    } else if working_dir.join("package.json").exists() {
        "node".to_string()
    } else if working_dir.join("pyproject.toml").exists()
        || working_dir.join("setup.py").exists()
        || working_dir.join("requirements.txt").exists()
    {
        "python".to_string()
    } else {
        "unknown".to_string()
    }
}

fn scope_suffix(scope: &str) -> &str {
    match scope {
        "unit" => " tests/",
        "integration" => " tests/integration/",
        _ => "",
    }
}

/// Parse test output to extract pass/fail/total counts.
/// Handles both Rust (cargo test) and Python (pytest) output formats.
pub(crate) fn parse_test_output(
    stdout: &str,
    stderr: &str,
) -> (Option<usize>, Option<usize>, Option<usize>) {
    let combined = format!("{}\n{}", stdout, stderr);

    /// Extract count before a keyword like "passed" or "failed" from token windows.
    /// Handles formats like: `7 passed; 1 failed` and `3 passed, 0 failed`.
    fn extract_count(tokens: &[&str], keyword: &str) -> Option<usize> {
        tokens
            .windows(2)
            .find(|w| w[1] == keyword)
            .and_then(|w| w[0].parse().ok())
    }

    // Rust: "test result: FAILED. 7 passed; 1 failed; 0 ignored; ..."
    if let Some(line) = combined.lines().find(|l| l.contains("test result:")) {
        let tokens: Vec<&str> = line
            .split([' ', ';', '.'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        let passed = extract_count(&tokens, "passed");
        let failed = extract_count(&tokens, "failed");
        let total = combined
            .lines()
            .filter(|l| l.starts_with("test ") && l.contains("..."))
            .count();
        return (passed, failed, Some(total));
    }

    // Python pytest: "= 1 passed, 2 failed in 0.05s ="
    if let Some(line) = combined
        .lines()
        .find(|l| l.contains("passed") || l.contains("failed"))
    {
        let tokens: Vec<&str> = line
            .split([' ', ',', '='])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        let passed = extract_count(&tokens, "passed");
        let failed = extract_count(&tokens, "failed");
        return (passed, failed, None);
    }

    (None, None, None)
}

// ── Tool definitions ──────────────────────────────────────────

pub fn execute_command_tool_def(description: &str) -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "execute_command".into(),
            description: format!("{}. Note: use run_tests for running tests (auto-detects project type). execute_command is only for lint/format/build and other non-test commands.", description),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to execute"
                    }
                },
                "required": ["command"]
            }),
        },
    }
}

pub fn run_tests_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "run_tests".into(),
            description: "Primary test-running tool. Auto-detects project type (Rust/Node/Python), selects appropriate subcommand by scope. Returns structured report: pass count, failed tests, stderr summary. Do NOT manually write cargo test/pytest — use this tool instead.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["unit", "integration", "all"],
                        "description": "unit=unit tests, integration=integration tests, all=all (default all)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Optional: specify a single test file path, e.g. tests/test_user.rs. Must match scope"
                    }
                },
                "required": ["scope"]
            }),
        },
    }
}
