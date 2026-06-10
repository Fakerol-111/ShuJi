use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncReadExt;

use crate::tool::ToolOutput;
use crate::tool::path::{PATH_ESCAPE, SYSTEM_BLOCKS};

// ── execute_command ───────────────────────────────────────────

/// Execute a shell command with timeout and safety checks.
pub async fn tool_execute_command(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let cmd = args["command"].as_str().unwrap_or("");
    if cmd.is_empty() {
        return ToolOutput::error("execute_command", "", "empty_command", "命令为空");
    }
    log_console!("[{}] executing: {}", dept, cmd);

    if let Err(blocked) = check_safe_command(cmd) {
        log_console!("[{}] BLOCKED command: {} — reason: {}", dept, cmd, blocked);
        return format!("[安全拦截] 命令被禁止执行: {}\n原因: {}", cmd, blocked);
    }

    let timeout = std::time::Duration::from_secs(120);
    let (shell, shell_args) = if cfg!(windows) {
        ("powershell", vec!["-Command"])
    } else {
        ("bash", vec!["-l", "-c"])
    };
    match execute_with_timeout(shell, &shell_args, cmd, working_dir, timeout).await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code().unwrap_or(-1);
            if exit_code == 0 {
                format!("命令执行成功 (exit={}):\n{}", exit_code, stdout)
            } else {
                format!(
                    "命令执行失败 (exit={}):\nstdout:\n{}\nstderr:\n{}",
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
        .map_err(|e| format!("无法启动命令: {}", e))?;

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
                        "命令执行超时（超过 {} 秒），进程已终止。",
                        timeout.as_secs()
                    ));
                }
                tokio::time::sleep(poll_interval).await;
            }
            Err(e) => return Err(format!("命令执行出错: {}", e)),
        }
    }
}

/// Check if a command is safe to execute.
/// Blocks dangerous system commands and path escape patterns.
fn check_safe_command(cmd: &str) -> Result<(), &'static str> {
    let c = cmd.trim();
    let tokens: Vec<&str> = c.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(());
    }

    for &(keyword, reason) in SYSTEM_BLOCKS {
        let kw_tokens: Vec<&str> = keyword.split_whitespace().collect();

        let matched = if kw_tokens.len() == 1 {
            // Single-token: match first command token (exact, or prefix for mkfs).
            if keyword == "mkfs" {
                tokens[0].starts_with("mkfs")
            } else {
                tokens[0] == keyword
            }
        } else {
            // Multi-token: match prefix of command tokens.
            tokens.len() >= kw_tokens.len() && tokens[..kw_tokens.len()] == kw_tokens[..]
        };

        if matched {
            return Err(reason);
        }
    }

    for &pattern in PATH_ESCAPE {
        if c.to_lowercase().contains(pattern) {
            return Err("禁止操作项目目录之外的文件");
        }
    }
    Ok(())
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
        "python" => match scope {
            "unit" => "python -m pytest tests/ -v".to_string(),
            "integration" => "python -m pytest tests/integration/ -v".to_string(),
            _ => "python -m pytest -v".to_string(),
        },
        _ => {
            return ToolOutput::success_raw(
                "run_tests",
                "未能检测到已知项目类型（Cargo.toml / package.json / pyproject.toml），无法确定测试命令。请使用 execute_command 自定义。",
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
                    "scope=integration 但路径 {} 不匹配集成测试目录（tests/integration/）",
                    p
                ),
            );
        }
        cmd.push_str(&format!(" -- {}", p));
    }

    log_console!("[run_tests] executing: {}", cmd);
    let timeout = std::time::Duration::from_secs(300);

    let (shell, shell_args) = if cfg!(windows) {
        ("powershell", vec!["-Command"])
    } else {
        ("bash", vec!["-l", "-c"])
    };

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
        "## 测试执行报告\n\n项目类型: {} | 范围: {} | 命令: `{}`\n",
        project_type, scope, cmd
    ));
    report.push_str(&format!(
        "退出码: {} | 通过: {} | 失败: {}",
        exit_code, pass_count, fail_count,
    ));
    if total_count > 0 {
        report.push_str(&format!(" | 总计: {}", total_count));
    }
    report.push('\n');

    if !failed_tests.is_empty() {
        report.push_str("\n### 失败用例\n");
        for t in &failed_tests {
            report.push_str(&format!("- {}\n", t));
        }
    }

    if exit_code != 0 {
        // Truncate stderr to avoid context overflow
        let stderr_trimmed = if stderr.len() > 2000 {
            let cutoff = stderr.floor_char_boundary(2000);
            format!(
                "{}...\n[截断：显示前 {} 字符，共 {} 字符]",
                &stderr[..cutoff],
                cutoff,
                stderr.len()
            )
        } else {
            stderr.to_string()
        };
        if !stderr_trimmed.is_empty() {
            report.push_str(&format!("\n### stderr 摘要\n{}", stderr_trimmed));
        }
    }

    if exit_code == 0 && failed_tests.is_empty() {
        report.push_str("\n✅ 全部通过");
    }

    ToolOutput::success_raw("run_tests", &report)
}

/// Detect project type by checking for key files.
fn detect_project_type(working_dir: &Path) -> String {
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
fn parse_test_output(stdout: &str, stderr: &str) -> (Option<usize>, Option<usize>, Option<usize>) {
    let combined = format!("{}\n{}", stdout, stderr);

    /// Extract count before a keyword like "passed" or "failed" from token windows.
    /// Handles formats like: `7 passed; 1 failed` and `3 passed, 0 failed`.
    fn extract_count<'a>(tokens: &[&'a str], keyword: &str) -> Option<usize> {
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
            description: format!("{}。注意：运行测试请用 run_tests 工具（自动检测项目类型）。execute_command 仅用于 lint/format/构建等非测试命令。", description),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "要执行的命令"
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
            description: "首选跑测试工具。自动检测项目类型（Rust/Node/Python），根据 scope 选择子命令。返回结构化报告：通过数、失败用例、stderr 摘要。禁止手写 cargo test/pytest——请使用本工具。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["unit", "integration", "all"],
                        "description": "unit=单元测试, integration=集成测试, all=全部（默认 all）"
                    },
                    "path": {
                        "type": "string",
                        "description": "可选：指定单个测试文件路径，如 tests/test_user.rs。scope 需匹配"
                    }
                },
                "required": ["scope"]
            }),
        },
    }
}
