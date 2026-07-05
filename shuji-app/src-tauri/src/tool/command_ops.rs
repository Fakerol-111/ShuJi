use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncReadExt;

use crate::tool::path::check_command_safety;
use crate::tool::python_cmd::pytest_cmd;
use crate::tool::ToolOutput;

/// Get the current platform's shell command.
/// Windows: prefer pwsh 7+ (better UTF-8), fallback to powershell 5.1
/// Others: prefer bash, fallback to sh
pub(crate) fn get_shell() -> (&'static str, Vec<&'static str>) {
    if cfg!(windows) {
        // 优先 pwsh 7+（更好的 UTF-8 支持和跨平台兼容性）
        if std::process::Command::new("pwsh")
            .arg("--version")
            .output()
            .is_ok()
        {
            return ("pwsh", vec!["-NoProfile", "-Command"]);
        }
        // 回退到 Windows PowerShell 5.1
        ("powershell", vec!["-NoProfile", "-Command"])
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

    // Windows: 注入 UTF-8 编码设置以确保中文输出正确
    #[cfg(windows)]
    let cmd = format!(
        "$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {}",
        cmd
    );
    #[cfg(not(windows))]
    let cmd = cmd.to_string();

    match execute_with_timeout(shell, &shell_args, &cmd, working_dir, timeout).await {
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

/// Inject UTF-8 encoding setup for Windows PowerShell.
///
/// On Windows with non-UTF8 system locales (e.g. Chinese GBK), `cargo` output
/// is encoded in the system's ANSI code page. `String::from_utf8_lossy` then
/// garbles the output, causing error parsers like `extract_compile_errors` to
/// fail silently (returning "Errors (0 found)" even when errors exist).
///
/// This function prepends the same UTF-8 encoding setup that `execute_command`
/// uses, ensuring consistent encoding across all command-executing tools.
pub(crate) fn inject_utf8_encoding(cmd: &str) -> String {
    if cfg!(windows) {
        format!(
            "$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {}",
            cmd
        )
    } else {
        cmd.to_string()
    }
}

/// Build a cargo command with RUSTFLAGS to suppress noise warnings.
///
/// During TDD red phase (stubs with `unimplemented!()`) and incremental
/// development, Rust produces many `dead_code` and `unused_variable` warnings.
/// These are not errors but can confuse the agent into thinking compilation
/// failed — leading to destructive "debugging" like deleting source files.
/// This function wraps the cargo command with RUSTFLAGS to suppress them,
/// ensuring the agent only sees actual compilation errors.
fn rust_cargo_cmd(subcommand: &str) -> String {
    let flags =
        "-A dead_code -A unused_variables -A unused_imports -A unused_mut -A unused_assignments";
    if cfg!(windows) {
        format!("$env:RUSTFLAGS=\"{}\"; cargo {}", flags, subcommand)
    } else {
        format!("RUSTFLAGS=\"{}\" cargo {}", flags, subcommand)
    }
}

/// Check if a command is safe to execute.
/// Blocks dangerous system commands and path escape patterns.
fn check_safe_command(cmd: &str) -> Result<(), &'static str> {
    check_command_safety(cmd)
}

// ── run_tests ─────────────────────────────────────────────────

/// Result of classifying compilation output into errors and warnings.
struct CompileResult {
    /// Formatted error strings with file locations (e.g. "- `error[E0xxx]: msg` at src/file.rs:line:col")
    errors: Vec<String>,
    /// Number of warnings detected (content suppressed to avoid confusing the agent).
    warning_count: usize,
}

/// Classify cargo output into errors and warnings.
///
/// Only `error[...]` and `error:` lines are extracted as full error entries.
/// `warning:` lines are counted but their content is NOT included in the output,
/// because warnings are not errors and exposing them can cause the agent to
/// attempt unnecessary "fixes" (e.g. adding `#[allow(dead_code)]`) that waste
/// iterations and sometimes corrupt the codebase.
fn classify_compile_output(output: &str) -> CompileResult {
    let mut errors = Vec::new();
    let mut warning_count = 0usize;
    let mut current_error: Option<String> = None;
    let mut current_location: Option<String> = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("error[") || trimmed.starts_with("error:") {
            if let Some(ref err) = current_error {
                let loc = current_location.as_deref().unwrap_or("unknown location");
                errors.push(format!("- `{}` at {}", err, loc));
            }
            current_error = Some(trimmed.to_string());
            current_location = None;
        } else if trimmed.starts_with("warning:") || trimmed.starts_with("warning[") {
            warning_count += 1;
        } else if trimmed.starts_with("--> ") {
            current_location = Some(trimmed.trim_start_matches("--> ").to_string());
        }
    }
    if let Some(ref err) = current_error {
        let loc = current_location.as_deref().unwrap_or("unknown location");
        errors.push(format!("- `{}` at {}", err, loc));
    }

    if errors.len() > 15 {
        let total = errors.len();
        errors.truncate(15);
        errors.push(format!("... and {} more errors", total - 15));
    }

    CompileResult {
        errors,
        warning_count,
    }
}

/// Append context-aware hints to a tool output report based on detected patterns.
///
/// These hints are **dynamically generated** from the actual command output,
/// not static prompt text. They help the agent understand common situations
/// without requiring language-specific knowledge in the system prompt.
///
/// Design principle (ACI): the tool output should be self-explanatory.
/// The agent should not need external context to interpret what happened.
fn append_context_hints(
    report: &mut String,
    project_type: &str,
    combined_output: &str,
    exit_code: i32,
    fail_count: usize,
) {
    let mut hints: Vec<String> = Vec::new();

    // Detect unimplemented!() / todo!() / NotImplementedError — TDD red phase
    if combined_output.contains("not implemented")
        || combined_output.contains("not yet implemented")
        || combined_output.contains("NotImplementedError")
    {
        hints.push(
            "A `unimplemented!()` / `todo!()` / `NotImplementedError` occurred — this is expected during TDD red phase. Write the real implementation to fix it.".to_string(),
        );
    }

    if project_type == "rust" {
        // Compilation passed but tests failed — the agent should focus on logic, not syntax
        if exit_code != 0
            && fail_count > 0
            && !combined_output.contains("error[")
            && !combined_output.contains("error:")
        {
            hints.push(
                "Compilation passed. Test assertions failed — fix the logic in your implementation, not the syntax.".to_string(),
            );
        }
    }

    if project_type == "python" {
        if combined_output.contains("ModuleNotFoundError")
            || combined_output.contains("ImportError")
        {
            hints.push(
                "An import error occurred — check if the module exists and the project environment is set up (run setup_test_env).".to_string(),
            );
        }
    }

    if !hints.is_empty() {
        report.push_str("\n### Hints\n");
        for hint in &hints {
            report.push_str(&format!("- {}\n", hint));
        }
    }
}

/// Smart truncation of stderr: prioritise error key lines over head truncation.
///
/// Extracts lines matching common error patterns (error[, error:, FAILED,
/// AssertionError, panic, --> location markers, Python exception types).
/// Falls back to head truncation when no key lines are found.
fn smart_truncate_stderr(stderr: &str, max_chars: usize) -> String {
    if stderr.len() <= max_chars {
        return stderr.to_string();
    }

    let key_prefixes = [
        "error[",
        "error:",
        "Error:",
        "FAILED",
        "... FAILED",
        "AssertionError",
        "panic:",
        "thread '",
        "  -->", // Rust location marker
    ];
    let error_keywords = [
        "ImportError",
        "ModuleNotFoundError",
        "TypeError",
        "ValueError",
        "KeyError",
        "AttributeError",
    ];

    let mut key_lines: Vec<String> = Vec::new();
    for line in stderr.lines() {
        let trimmed = line.trim();
        let is_key = key_prefixes.iter().any(|p| trimmed.starts_with(p))
            || error_keywords.iter().any(|k| trimmed.contains(k));
        if is_key {
            key_lines.push(line.to_string());
        }
    }

    if key_lines.is_empty() {
        // No key lines — fall back to head truncation
        let cutoff = stderr.floor_char_boundary(max_chars);
        return format!(
            "{}...\n[Truncated: showing first {} chars, total {} chars]",
            &stderr[..cutoff],
            cutoff,
            stderr.len()
        );
    }

    let mut result = String::new();
    result.push_str(&format!("### Key errors ({} found)\n\n", key_lines.len()));

    let max_key_lines = 10;
    for (idx, line) in key_lines.iter().take(max_key_lines).enumerate() {
        if line.len() > 300 {
            let cutoff = line.floor_char_boundary(300);
            result.push_str(&format!("{}. {}...\n", idx + 1, &line[..cutoff]));
        } else {
            result.push_str(&format!("{}. {}\n", idx + 1, line));
        }
    }

    if key_lines.len() > max_key_lines {
        result.push_str(&format!(
            "\n... and {} more errors omitted\n",
            key_lines.len() - max_key_lines
        ));
    }

    result.push_str(&format!(
        "\n[Full stderr: {} chars, extracted {} key lines]",
        stderr.len(),
        key_lines.len()
    ));

    // Final safety truncation
    if result.len() > max_chars {
        let cutoff = result.floor_char_boundary(max_chars);
        result.truncate(cutoff);
        result.push_str("...");
    }

    result
}

/// Runs tests with auto-detected project type and structured output.
/// Reduces LLM command-typing errors that trigger watchdog.
pub async fn tool_run_tests(working_dir: &Path, args: &serde_json::Value) -> String {
    let scope = args["scope"].as_str().unwrap_or("all");
    let path = args["path"].as_str().filter(|s| !s.is_empty());
    let test_name = args["test_name"].as_str().filter(|s| !s.is_empty());

    // Detect project type
    let project_type = detect_project_type(working_dir);
    let (shell, shell_args) = get_shell();

    // ── Rust: cargo check pre-flight ──
    // Compilation errors and test failures require different fixes.
    // Running cargo check first prevents wasting test iterations on syntax errors.
    if project_type == "rust" {
        let check_cmd = inject_utf8_encoding(&rust_cargo_cmd("check 2>&1"));
        let check_timeout = std::time::Duration::from_secs(180);
        match execute_with_timeout(shell, &shell_args, &check_cmd, working_dir, check_timeout).await
        {
            Ok(o) => {
                let check_stdout = String::from_utf8_lossy(&o.stdout);
                let check_stderr = String::from_utf8_lossy(&o.stderr);
                let combined = format!("{}\n{}", check_stdout, check_stderr);
                if !o.status.success() {
                    let compiled = classify_compile_output(&combined);
                    let warning_note = if compiled.warning_count > 0 {
                        format!(
                            "\n\n({} warnings suppressed — warnings are not errors, focus on fixing the errors above.)",
                            compiled.warning_count
                        )
                    } else {
                        String::new()
                    };
                    return ToolOutput::error(
                        "run_tests",
                        "",
                        "compile_error",
                        &format!(
                            "## Compilation Failed\n\n\
                            cargo check failed (exit={}). Fix compilation errors before running tests.\n\n\
                            ### Errors ({} found)\n{}{}\n\n\
                            **No tests were executed.** Fix the errors above, then call run_tests again.",
                            o.status.code().unwrap_or(-1),
                            compiled.errors.len(),
                            compiled.errors.join("\n"),
                            warning_note,
                        ),
                    );
                }
            }
            Err(e) => {
                // cargo check timed out or failed to execute — skip pre-check, continue to tests
                log_console!("[run_tests] cargo check skipped: {}", e);
            }
        }
    }

    let mut cmd = match project_type.as_str() {
        "rust" => match scope {
            "unit" => rust_cargo_cmd("test --lib"),
            "integration" => rust_cargo_cmd("test --tests"),
            _ => rust_cargo_cmd("test"),
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
        "python" => pytest_cmd(scope, working_dir),
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

    // Append test name filter if provided
    if let Some(tn) = test_name {
        // Validate test_name: only allow alphanumeric, underscore, ::, ., -
        if !tn
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':' || c == '.' || c == '-')
        {
            return ToolOutput::error(
                "run_tests",
                "",
                "invalid_test_name",
                "test_name contains invalid characters. Only alphanumeric, _, ::, ., - are allowed.",
            );
        }
        match project_type.as_str() {
            "rust" => cmd.push_str(&format!(" {}", tn)),
            "python" => cmd.push_str(&format!(" -k \"{}\"", tn)),
            "node" => cmd.push_str(&format!(" -t \"{}\"", tn)),
            _ => {}
        }
    }

    log_console!("[run_tests] executing: {}", cmd);
    let cmd = inject_utf8_encoding(&cmd);
    let timeout = std::time::Duration::from_secs(300);

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
    // Supports both Rust ("test test_name ... FAILED") and pytest ("FAILED tests/test_xxx.py::test_name")
    let failed_tests: Vec<String> = stdout
        .lines()
        .filter_map(|l| {
            let trimmed = l.trim();
            // Rust format: "test test_name ... FAILED"
            if trimmed.contains("... FAILED") {
                Some(
                    trimmed
                        .trim_start_matches("test ")
                        .trim_end_matches(" ... FAILED")
                        .to_string(),
                )
            // Pytest format: "FAILED tests/test_xxx.py::test_name"
            } else if trimmed.starts_with("FAILED ") {
                Some(trimmed.trim_start_matches("FAILED ").to_string())
            // Generic "test_name FAILED"
            } else if trimmed.ends_with(" FAILED") && trimmed.contains(" ") {
                Some(trimmed.trim_end_matches(" FAILED").to_string())
            } else {
                None
            }
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
        // Smart truncation: extract key error lines instead of simple head truncation
        let stderr_trimmed = smart_truncate_stderr(&stderr, 3000);
        if !stderr_trimmed.is_empty() {
            report.push_str(&format!("\n### stderr Summary\n{}", stderr_trimmed));
        }
    }

    if exit_code == 0 && failed_tests.is_empty() {
        report.push_str("\nAll tests passed");
    }

    // ── Context-aware hints (dynamically generated, not static prompt) ──
    let combined_output = format!("{}\n{}", stdout, stderr);
    append_context_hints(
        &mut report,
        &project_type,
        &combined_output,
        exit_code,
        fail_count,
    );

    // ── Machine-parseable JSON summary (for LLM structured reading) ──
    let json_summary = serde_json::json!({
        "exit_code": exit_code,
        "passed": pass_count,
        "failed": fail_count,
        "total": total_count,
        "failed_tests": failed_tests,
        "project_type": project_type,
        "scope": scope,
        "command": cmd,
        "compilation_check": if project_type == "rust" { "passed" } else { "n/a" },
    });
    report.push_str(&format!(
        "\n\n```json\n{}\n```",
        serde_json::to_string_pretty(&json_summary).unwrap_or_default()
    ));

    // Return ok:false when tests fail so the watchdog can detect repeated
    // test failures and trigger the test-stalemate playbook / force-stop.
    if exit_code != 0 || fail_count > 0 {
        ToolOutput::error("run_tests", "", "test_failure", &report)
    } else {
        ToolOutput::success_raw("run_tests", &report)
    }
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
                    },
                    "test_name": {
                        "type": "string",
                        "description": "Optional: run a specific test by name (e.g. test_create_user). Much faster than running all tests. Rust: filter by test name; Python: -k pattern; Node: -t pattern"
                    }
                },
                "required": ["scope"]
            }),
        },
    }
}

// ── check_compile ─────────────────────────────────────────────

/// Check if the project compiles without running tests.
/// Returns structured result: compilation passed/failed + error list.
pub async fn tool_check_compile(working_dir: &Path, _args: &serde_json::Value) -> String {
    let project_type = detect_project_type(working_dir);
    let (shell, shell_args) = get_shell();

    let (cmd, timeout) = match project_type.as_str() {
        "rust" => (
            rust_cargo_cmd("check 2>&1"),
            std::time::Duration::from_secs(180),
        ),
        "node" => {
            if working_dir.join("node_modules/.bin/tsc").exists() {
                (
                    "npx tsc --noEmit 2>&1".to_string(),
                    std::time::Duration::from_secs(120),
                )
            } else {
                return ToolOutput::success_raw(
                    "check_compile",
                    "{\"skipped\": true, \"reason\": \"no TypeScript compiler found\"}",
                );
            }
        }
        "python" => {
            let py = crate::tool::python_cmd::venv_python_or_system(working_dir);
            if cfg!(windows) {
                (format!("{} -m py_compile $(Get-ChildItem -Recurse -Filter *.py -Exclude .venv | ForEach-Object {{$_.FullName}}) 2>&1", py),
                 std::time::Duration::from_secs(60))
            } else {
                (
                    format!(
                        "{} -m py_compile $(find . -name '*.py' -not -path './.venv/*') 2>&1",
                        py
                    ),
                    std::time::Duration::from_secs(60),
                )
            }
        }
        _ => {
            return ToolOutput::success_raw(
                "check_compile",
                &format!(
                    "{{\"skipped\": true, \"reason\": \"unsupported project type: {}\"}}",
                    project_type
                ),
            );
        }
    };

    let cmd = inject_utf8_encoding(&cmd);
    log_console!("[check_compile] executing: {}", cmd);
    match execute_with_timeout(shell, &shell_args, &cmd, working_dir, timeout).await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}\n{}", stdout, stderr);
            let exit_code = output.status.code().unwrap_or(-1);

            if exit_code == 0 {
                ToolOutput::success_raw(
                    "check_compile",
                    &format!(
                        "## Compilation Check\n\nProject type: {} | Command: `{}`\nExit code: {}\n\n✅ Compilation successful. You can now run tests.",
                        project_type, cmd, exit_code,
                    ),
                )
            } else {
                let compiled = classify_compile_output(&combined);
                let warning_note = if compiled.warning_count > 0 {
                    format!(
                        "\n\n({} warnings suppressed — warnings are not errors, focus on fixing the errors above.)",
                        compiled.warning_count
                    )
                } else {
                    String::new()
                };
                ToolOutput::error(
                    "check_compile",
                    "",
                    "compile_error",
                    &format!(
                        "## Compilation Check\n\nProject type: {} | Command: `{}`\nExit code: {}\n\n❌ Compilation failed. Fix the errors below before running tests.\n\n### Errors ({} found)\n{}{}",
                        project_type, cmd, exit_code, compiled.errors.len(), compiled.errors.join("\n"), warning_note,
                    ),
                )
            }
        }
        Err(e) => ToolOutput::error("check_compile", "", "exec_error", &e),
    }
}

/// Tool definition for check_compile.
pub fn check_compile_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "check_compile".into(),
            description: "Check if the project compiles without running tests. Rust: cargo check, Node: tsc --noEmit, Python: py_compile. Use this BEFORE run_tests to catch compilation errors early. Returns structured error list with file locations.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    }
}
