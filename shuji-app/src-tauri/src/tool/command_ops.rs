use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncReadExt;

use crate::tool::path::check_command_safety;
use crate::tool::python_cmd::pytest_cmd;
use crate::tool::ToolOutput;

pub(crate) fn get_shell() -> (&'static str, Vec<&'static str>) {
    if cfg!(windows) {
        if std::process::Command::new("pwsh")
            .arg("--version")
            .output()
            .is_ok()
        {
            return ("pwsh", vec!["-NoProfile", "-Command"]);
        }
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

            ToolOutput::command(
                "execute_command",
                exit_code,
                &stdout,
                &stderr,
                false,
                10_000,
                5_000,
            )
        }
        Err(timeout_msg) => {
            ToolOutput::command("execute_command", -1, "", &timeout_msg, true, 10_000, 5_000)
        }
    }
}

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

fn rust_cargo_cmd(subcommand: &str) -> String {
    let flags =
        "-A dead_code -A unused_variables -A unused_imports -A unused_mut -A unused_assignments";
    if cfg!(windows) {
        format!("$env:RUSTFLAGS=\"{}\"; cargo {}", flags, subcommand)
    } else {
        format!("RUSTFLAGS=\"{}\" cargo {}", flags, subcommand)
    }
}

fn rust_test_compile_cmd(scope: &str) -> String {
    match scope {
        "unit" => rust_cargo_cmd("test --lib --no-run"),
        "integration" => rust_cargo_cmd("test --tests --no-run"),
        _ => rust_cargo_cmd("test --no-run"),
    }
}

fn check_safe_command(cmd: &str) -> Result<(), &'static str> {
    check_command_safety(cmd)
}

// ── run_tests ─────────────────────────────────────────────────

struct CompileResult {
    errors: Vec<String>,
    warning_count: usize,
    diagnostic_excerpt: Option<String>,
}

fn extract_diagnostic_excerpt(output: &str, max_chars: usize) -> String {
    let keywords = [
        "error",
        "failed",
        "Caused by",
        "Permission denied",
        "拒绝访问",
        "Access is denied",
        ".cargo-lock",
        "could not compile",
        "type annotations needed",
    ];
    let mut key_lines: Vec<&str> = Vec::new();
    let mut other_lines: Vec<&str> = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if keywords.iter().any(|k| trimmed.contains(k)) {
            key_lines.push(trimmed);
        } else if key_lines.len() + other_lines.len() < 60 {
            other_lines.push(trimmed);
        }
    }

    let mut result = String::new();
    for line in key_lines.iter().take(20) {
        if result.len() + line.len() + 1 > max_chars {
            break;
        }
        result.push_str(line);
        result.push('\n');
    }
    if result.len() < max_chars {
        for line in other_lines.iter().take(15) {
            if result.len() + line.len() + 1 > max_chars {
                break;
            }
            result.push_str(line);
            result.push('\n');
        }
    }
    if result.len() > max_chars {
        let cutoff = result.floor_char_boundary(max_chars);
        result.truncate(cutoff);
    }
    result
}

fn classify_execution_failure(output: &str) -> Option<&'static str> {
    let lower = output.to_lowercase();
    if lower.contains(".cargo-lock")
        || lower.contains("permission denied")
        || output.contains("拒绝访问")
        || lower.contains("access is denied")
        || lower.contains("failed to open")
    {
        return Some("environment_error");
    }
    None
}

/// Compute a stable fingerprint for a failure output.
///
/// Priority:
/// 1. error_code + first rustc error code + file:line
/// 2. error_code + first assertion failure text
/// 3. error_code + first key line from raw diagnostic excerpt
/// 4. tool_name + error_code as last resort
///
/// Used by watchdog to detect repeated same-root-cause failures.
pub fn compute_failure_fingerprint(output: &str, error_code: &str) -> String {
    // Try rustc-style error with file:line
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("error[") {
            if let Some(file_line) = output
                .lines()
                .skip_while(|l| !l.trim().starts_with("--> "))
                .nth(0)
                .and_then(|l| {
                    l.trim()
                        .trim_start_matches("--> ")
                        .split_once(':')
                        .map(|(f, _)| f.to_string())
                })
            {
                let code = trimmed.split(|c| c == '[' || c == ']').nth(1).unwrap_or("");
                return format!("{}|rustc|{}|{}", error_code, code, file_line);
            }
            let code = trimmed.split(|c| c == '[' || c == ']').nth(1).unwrap_or("");
            return format!("{}|rustc|{}", error_code, code);
        }
        if trimmed.starts_with("error:") && !trimmed.contains("failed to open") {
            let text = trimmed.trim_start_matches("error:").trim();
            let short: String = text.chars().take(60).collect();
            return format!("{}|rustc_msg|{}", error_code, short);
        }
    }

    // Try assertion failure from test output
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("FAILED") && trimmed.contains("...") {
            let test_name = trimmed
                .trim_start_matches("test ")
                .trim_end_matches(" ... FAILED");
            return format!("{}|test_fail|{}", error_code, test_name);
        }
        if trimmed.starts_with("FAILED ") {
            let name = trimmed.trim_start_matches("FAILED ");
            return format!("{}|test_fail|{}", error_code, name);
        }
    }

    // Environment error: use first keyword line
    if error_code == "environment_error" {
        let env_keywords = [
            ".cargo-lock",
            "permission denied",
            "拒绝访问",
            "access is denied",
            "failed to open",
        ];
        for line in output.lines() {
            let lower = line.to_lowercase();
            for kw in &env_keywords {
                if lower.contains(kw) {
                    return format!("{}|env|{}", error_code, kw);
                }
            }
        }
    }

    // Fallback: hash first 80 chars of relevant output
    let relevant: String = output
        .lines()
        .filter(|l| {
            let lower = l.to_lowercase();
            lower.contains("error")
                || lower.contains("failed")
                || lower.contains("caused by")
                || lower.contains("panic")
        })
        .take(3)
        .collect::<Vec<_>>()
        .join("|");
    if !relevant.is_empty() {
        let short: String = relevant.chars().take(80).collect();
        return format!("{}|raw|{}", error_code, short);
    }

    format!("{}|unknown", error_code)
}

/// Generate suggested next diagnostic commands for Rust failures.
pub fn suggest_rust_diagnostic(error_code: &str, output: &str, scope: &str) -> Vec<String> {
    let mut suggestions = Vec::new();

    match error_code {
        "compile_error" => {
            // Try to extract error code for rustc --explain
            for line in output.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("error[") {
                    if let Some(code) = trimmed.split(|c| c == '[' || c == ']').nth(1) {
                        suggestions.push(format!("rustc --explain {}", code));
                        break;
                    }
                }
            }
            // Suggest targeted compile
            match scope {
                "unit" => suggestions.push("cargo test --lib --no-run".to_string()),
                "integration" => suggestions.push("cargo test --tests --no-run".to_string()),
                _ => suggestions.push("cargo test --no-run".to_string()),
            }
        }
        "test_failure" => {
            suggestions.push("cargo test --lib <test_name> -- --nocapture".to_string());
            if scope != "unit" {
                suggestions.push("cargo test --lib".to_string());
            }
        }
        "environment_error" => {
            suggestions.push("Check file permissions and disk space".to_string());
            if output.contains(".cargo-lock") {
                suggestions.push("Remove target/debug/.cargo-lock and retry".to_string());
            }
        }
        _ => {}
    }

    suggestions
}

fn classify_compile_output(output: &str, exit_code: i32) -> CompileResult {
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

    let diagnostic_excerpt = if exit_code != 0 && errors.is_empty() {
        Some(extract_diagnostic_excerpt(output, 3000))
    } else {
        None
    };

    CompileResult {
        errors,
        warning_count,
        diagnostic_excerpt,
    }
}

// ── P2.1: Subproject resolution for workspace/monorepo support ─────

fn resolve_subproject(
    working_dir: &Path,
    subproject: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    match subproject {
        None | Some("") => Ok(None),
        Some(sub) => {
            let path = working_dir.join(sub);
            let canonical = path
                .canonicalize()
                .map_err(|e| format!("subproject '{}' does not exist: {}", sub, e))?;
            let root = working_dir
                .canonicalize()
                .map_err(|_| "project root does not exist".to_string())?;
            if !canonical.starts_with(&root) {
                return Err(format!(
                    "subproject '{}' escapes the project root directory",
                    sub
                ));
            }
            Ok(Some(canonical))
        }
    }
}

struct ProjectCandidate {
    path: PathBuf,
    project_type: String,
    relative: String,
}

fn detect_subproject_candidates(working_dir: &Path) -> Vec<ProjectCandidate> {
    let mut candidates = Vec::new();

    let pt = detect_project_type(working_dir);
    if pt != "unknown" {
        let rel = ".".to_string();
        candidates.push(ProjectCandidate {
            path: working_dir.to_path_buf(),
            project_type: pt,
            relative: rel,
        });
    }

    let common_subdirs = [
        "shuji-app",
        "shuji-app/src-tauri",
        "src",
        "src-tauri",
        "frontend",
        "backend",
        "server",
        "client",
        "web",
        "api",
    ];

    for sub in &common_subdirs {
        let candidate = working_dir.join(sub);
        if candidate.exists() && candidate.is_dir() {
            let pt = detect_project_type(&candidate);
            if pt != "unknown" {
                let canon = candidate.canonicalize().ok();
                if candidates
                    .iter()
                    .any(|c| c.path.canonicalize().ok() == canon)
                {
                    continue;
                }
                candidates.push(ProjectCandidate {
                    path: candidate,
                    project_type: pt,
                    relative: sub.to_string(),
                });
            }
        }
    }

    candidates.sort_by_key(|a| a.relative.matches('/').count());
    candidates.truncate(6);
    candidates
}

fn ambiguous_project_message(candidates: &[ProjectCandidate]) -> String {
    let mut msg = String::from(
        "## Ambiguous project: multiple project types detected.\n\n\
         Specify `subproject` in your run_tests call to choose one:\n\n",
    );
    for (i, c) in candidates.iter().enumerate() {
        msg.push_str(&format!(
            "{}. `{}` — `{}` project\n",
            i + 1,
            c.relative,
            c.project_type
        ));
    }
    msg
}

fn append_context_hints(
    report: &mut String,
    project_type: &str,
    combined_output: &str,
    exit_code: i32,
    fail_count: usize,
) {
    let mut hints: Vec<String> = Vec::new();

    if combined_output.contains("not implemented")
        || combined_output.contains("not yet implemented")
        || combined_output.contains("NotImplementedError")
    {
        hints.push(
            "A `unimplemented!()` / `todo!()` / `NotImplementedError` occurred — this is expected during TDD red phase. Write the real implementation to fix it.".to_string(),
        );
    }

    if project_type == "rust"
        && exit_code != 0
        && fail_count > 0
        && !combined_output.contains("error[")
        && !combined_output.contains("error:")
    {
        hints.push(
            "Compilation passed. Test assertions failed — fix the logic in your implementation, not the syntax.".to_string(),
        );
    }

    if project_type == "python"
        && (combined_output.contains("ModuleNotFoundError")
            || combined_output.contains("ImportError"))
    {
        hints.push(
            "An import error occurred — check if the module exists and the project environment is set up (run setup_test_env).".to_string(),
        );
    }

    if !hints.is_empty() {
        report.push_str("\n### Hints\n");
        for hint in &hints {
            report.push_str(&format!("- {}\n", hint));
        }
    }
}

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
        "  -->",
        "Caused by:",
        "could not compile",
    ];
    let error_keywords = [
        "ImportError",
        "ModuleNotFoundError",
        "TypeError",
        "ValueError",
        "KeyError",
        "AttributeError",
        "Permission denied",
        "拒绝访问",
        "Access is denied",
        ".cargo-lock",
        "failed to open",
        "type annotations needed",
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

    if result.len() > max_chars {
        let cutoff = result.floor_char_boundary(max_chars);
        result.truncate(cutoff);
        result.push_str("...");
    }

    result
}

pub async fn tool_run_tests(working_dir: &Path, args: &serde_json::Value) -> String {
    let scope = args["scope"].as_str().unwrap_or("all");
    let path = args["path"].as_str().filter(|s| !s.is_empty());
    let test_name = args["test_name"].as_str().filter(|s| !s.is_empty());
    let subproject = args["subproject"].as_str().filter(|s| !s.is_empty());
    let project_type_hint = args["project_type"].as_str().filter(|s| !s.is_empty());

    let target_dir = match resolve_subproject(working_dir, subproject) {
        Ok(Some(path)) => path,
        Ok(None) => working_dir.to_path_buf(),
        Err(e) => {
            return ToolOutput::error("run_tests", "", "invalid_subproject", &e);
        }
    };

    let project_type = match project_type_hint {
        Some(hint) => hint.to_string(),
        None => detect_project_type(&target_dir),
    };

    if subproject.is_none() && project_type_hint.is_none() {
        let candidates = detect_subproject_candidates(working_dir);
        if candidates.len() > 1 {
            let root_pt = detect_project_type(working_dir);
            if root_pt == "unknown" || candidates.len() > 2 {
                return ToolOutput::error(
                    "run_tests",
                    "",
                    "ambiguous_project",
                    &ambiguous_project_message(&candidates),
                );
            }
        }
    }

    if project_type == "unknown" {
        let hint = if subproject.is_some() {
            format!(
                "No known project type found at '{}' (Cargo.toml / package.json / pyproject.toml).",
                subproject.unwrap_or("")
            )
        } else {
            "Unable to detect known project type (Cargo.toml / package.json / pyproject.toml)."
                .to_string()
        };
        return ToolOutput::success_raw(
            "run_tests",
            &format!(
                "{}. Cannot determine test command. Use execute_command for custom commands.",
                hint
            ),
        );
    }

    if let Some(hint) = project_type_hint {
        let actual = detect_project_type(&target_dir);
        if actual != "unknown" && actual != hint {
            let msg = format!(
                "project_type='{}' but detected '{}' at '{}'. \
                 Fix the project_type parameter or omit it for auto-detection.",
                hint,
                actual,
                subproject.unwrap_or(".")
            );
            return ToolOutput::error("run_tests", "", "project_type_mismatch", &msg);
        }
    }

    let (shell, shell_args) = get_shell();

    // ── Rust: test-target compilation pre-flight ──
    // Unlike `cargo check` (which skips #[cfg(test)] code), we compile test targets
    // via `cargo test --no-run` so that test-code compilation errors (e.g. type
    // inference failures only triggered by test code) are caught before test execution.
    if project_type == "rust" {
        let check_cmd = inject_utf8_encoding(&rust_test_compile_cmd(scope));
        let check_timeout = std::time::Duration::from_secs(180);
        match execute_with_timeout(shell, &shell_args, &check_cmd, &target_dir, check_timeout).await
        {
            Ok(o) => {
                let check_stdout = String::from_utf8_lossy(&o.stdout);
                let check_stderr = String::from_utf8_lossy(&o.stderr);
                let combined = format!("{}\n{}", check_stdout, check_stderr);
                let exit_code_pre = o.status.code().unwrap_or(-1);
                if !o.status.success() {
                    if let Some(env_err) = classify_execution_failure(&combined) {
                        let message = format!(
                            "## Environment Error\n\n\
                            Test compilation failed with an environment issue (exit={}).\n\n\
                            ### Raw Diagnostic Excerpt\n{}\n\n\
                            This is an environment or permission issue, not a code error. \
                            Do NOT modify source code. Report the environment issue instead.",
                            exit_code_pre,
                            extract_diagnostic_excerpt(&combined, 2000),
                        );
                        let fingerprint = compute_failure_fingerprint(&combined, env_err);
                        let suggested = suggest_rust_diagnostic(env_err, &combined, scope);
                        return ToolOutput::diagnostic_error(
                            "run_tests",
                            "",
                            env_err,
                            &message,
                            "raw_excerpt",
                            &fingerprint,
                            false,
                            suggested,
                        );
                    }
                    let compiled = classify_compile_output(&combined, exit_code_pre);
                    let parsed_count = compiled.errors.len();
                    let mut error_section = if parsed_count == 0 {
                        String::from("### Parsed Errors\nNo rustc-style errors were parsed.\n")
                    } else {
                        format!(
                            "### Parsed Errors ({} found)\n{}\n",
                            parsed_count,
                            compiled.errors.join("\n"),
                        )
                    };
                    if let Some(ref excerpt) = compiled.diagnostic_excerpt {
                        error_section
                            .push_str(&format!("\n### Raw Diagnostic Excerpt\n{}\n", excerpt,));
                    }
                    let warning_note = if compiled.warning_count > 0 {
                        format!(
                            "\n\n({} warnings suppressed — warnings are not errors, focus on fixing the errors above.)",
                            compiled.warning_count
                        )
                    } else {
                        String::new()
                    };
                    let compile_msg = format!(
                        "## Compilation Failed\n\n\
                        Test compilation failed (exit={}). Fix errors before running tests.\n\n\
                        {}{}\n\n\
                        **No tests were executed.** Fix the errors above, then call run_tests again.",
                        exit_code_pre,
                        error_section,
                        warning_note,
                    );
                    let fingerprint = compute_failure_fingerprint(&combined, "compile_error");
                    let suggested = suggest_rust_diagnostic("compile_error", &combined, scope);
                    return ToolOutput::diagnostic_error(
                        "run_tests",
                        "",
                        "compile_error",
                        &compile_msg,
                        if compiled.diagnostic_excerpt.is_some() {
                            "raw_excerpt"
                        } else {
                            "structured"
                        },
                        &fingerprint,
                        true,
                        suggested,
                    );
                }
            }
            Err(e) => {
                log_console!("[run_tests] test compile pre-flight skipped: {}", e);
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
            let has_vitest = target_dir.join("node_modules/.bin/vitest").exists();
            let has_jest = target_dir.join("node_modules/.bin/jest").exists();
            if has_vitest {
                format!("npx vitest run{}", scope_suffix(scope))
            } else if has_jest {
                format!("npx jest --verbose{}", scope_suffix(scope))
            } else {
                "npm test".to_string()
            }
        }
        "python" => pytest_cmd(scope, &target_dir),
        _ => {
            return ToolOutput::success_raw(
                "run_tests",
                "Unable to detect known project type (Cargo.toml / package.json / pyproject.toml). Cannot determine test command. Use execute_command for custom commands.",
            );
        }
    };

    if let Some(p) = path {
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

    if let Some(tn) = test_name {
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

    let output = match execute_with_timeout(shell, &shell_args, &cmd, &target_dir, timeout).await {
        Ok(o) => o,
        Err(e) => return ToolOutput::error("run_tests", "", "exec_error", &e),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    let (parsed_pass, parsed_fail, parsed_total) = parse_test_output(&stdout, &stderr);

    let pass_count = parsed_pass.unwrap_or(0);
    let fail_count = parsed_fail.unwrap_or(0);
    let total_count = parsed_total.unwrap_or(0);

    let failed_tests: Vec<String> = stdout
        .lines()
        .filter_map(|l| {
            let trimmed = l.trim();
            if trimmed.contains("... FAILED") {
                Some(
                    trimmed
                        .trim_start_matches("test ")
                        .trim_end_matches(" ... FAILED")
                        .to_string(),
                )
            } else if trimmed.starts_with("FAILED ") {
                Some(trimmed.trim_start_matches("FAILED ").to_string())
            } else if trimmed.ends_with(" FAILED") && trimmed.contains(" ") {
                Some(trimmed.trim_end_matches(" FAILED").to_string())
            } else {
                None
            }
        })
        .collect();

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
        let stderr_trimmed = smart_truncate_stderr(&stderr, 3000);
        if !stderr_trimmed.is_empty() {
            report.push_str(&format!("\n### stderr Summary\n{}", stderr_trimmed));
        }
    }

    if exit_code == 0 && failed_tests.is_empty() {
        report.push_str("\nAll tests passed");
    }

    let combined_output = format!("{}\n{}", stdout, stderr);
    append_context_hints(
        &mut report,
        &project_type,
        &combined_output,
        exit_code,
        fail_count,
    );

    let json_summary = serde_json::json!({
        "exit_code": exit_code,
        "passed": pass_count,
        "failed": fail_count,
        "total": total_count,
        "failed_tests": failed_tests,
        "project_type": project_type,
        "scope": scope,
        "command": cmd,
        "compilation_check": if project_type == "rust" { "test_compile_pre_flight_ok" } else { "n/a" },
    });
    report.push_str(&format!(
        "\n\n```json\n{}\n```",
        serde_json::to_string_pretty(&json_summary).unwrap_or_default()
    ));

    if exit_code != 0 || fail_count > 0 {
        let combined_output = format!("{}\n{}", stdout, stderr);
        if let Some(env_err) = classify_execution_failure(&combined_output) {
            let fingerprint = compute_failure_fingerprint(&combined_output, env_err);
            let suggested = suggest_rust_diagnostic(env_err, &combined_output, scope);
            ToolOutput::diagnostic_error(
                "run_tests",
                "",
                env_err,
                &report,
                "raw_excerpt",
                &fingerprint,
                false,
                suggested,
            )
        } else {
            let fingerprint = compute_failure_fingerprint(&combined_output, "test_failure");
            let suggested = suggest_rust_diagnostic("test_failure", &combined_output, scope);
            let diag_grade = if !failed_tests.is_empty() {
                "structured"
            } else {
                "raw_excerpt"
            };
            ToolOutput::diagnostic_error(
                "run_tests",
                "",
                "test_failure",
                &report,
                diag_grade,
                &fingerprint,
                true,
                suggested,
            )
        }
    } else {
        ToolOutput::success_raw("run_tests", &report)
    }
}

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

pub(crate) fn parse_test_output(
    stdout: &str,
    stderr: &str,
) -> (Option<usize>, Option<usize>, Option<usize>) {
    let combined = format!("{}\n{}", stdout, stderr);

    fn extract_count(tokens: &[&str], keyword: &str) -> Option<usize> {
        tokens
            .windows(2)
            .find(|w| w[1] == keyword)
            .and_then(|w| w[0].parse().ok())
    }

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
                    },
                    "subproject": {
                        "type": "string",
                        "description": "Optional: subproject path relative to project root, e.g. 'shuji-app/src-tauri' for Rust unit tests, 'shuji-app' for frontend tests. Must be a subdirectory of the project. When omitted, auto-detects from the working directory."
                    },
                    "project_type": {
                        "type": "string",
                        "enum": ["rust", "node", "python"],
                        "description": "Optional: force project type. Useful when auto-detection is ambiguous (e.g. monorepo with both Cargo.toml and package.json). When omitted, auto-detects from the target directory."
                    }
                },
                "required": ["scope"]
            }),
        },
    }
}

// ── check_compile ─────────────────────────────────────────────

pub async fn tool_check_compile(working_dir: &Path, args: &serde_json::Value) -> String {
    let project_type = detect_project_type(working_dir);
    let (shell, shell_args) = get_shell();
    let include_tests = args["include_tests"].as_bool().unwrap_or(false);

    let (cmd, timeout) = match project_type.as_str() {
        "rust" => {
            if include_tests {
                (
                    rust_cargo_cmd("test --no-run 2>&1"),
                    std::time::Duration::from_secs(300),
                )
            } else {
                (
                    rust_cargo_cmd("check 2>&1"),
                    std::time::Duration::from_secs(180),
                )
            }
        }
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
                let rust_note = if project_type == "rust" {
                    "\n\n> ℹ️  `cargo check` compiles library and binary targets only. It does **not** compile `#[cfg(test)]` test code. Use `run_tests` to compile and run test targets."
                } else {
                    ""
                };
                ToolOutput::success_raw(
                    "check_compile",
                    &format!(
                        "## Compilation Check\n\nProject type: {} | Command: `{}`\nExit code: {}\n\n✅ Compilation successful.{}. You can now run tests.",
                        project_type, cmd, exit_code, rust_note,
                    ),
                )
            } else {
                if let Some(env_err) = classify_execution_failure(&combined) {
                    let env_msg = format!(
                        "## Compilation Check\n\nProject type: {} | Command: `{}`\nExit code: {}\n\n❌ Environment issue detected.\n\n### Raw Diagnostic Excerpt\n{}\n\nThis is an environment or permission issue, not a code error. Do NOT modify source code. Report the environment issue instead.",
                        project_type, cmd, exit_code,
                        extract_diagnostic_excerpt(&combined, 2000),
                    );
                    let fingerprint = compute_failure_fingerprint(&combined, env_err);
                    return ToolOutput::diagnostic_error(
                        "check_compile",
                        "",
                        env_err,
                        &env_msg,
                        "raw_excerpt",
                        &fingerprint,
                        false,
                        vec![],
                    );
                }
                let compiled = classify_compile_output(&combined, exit_code);
                let parsed_count = compiled.errors.len();
                let mut error_section = if parsed_count == 0 {
                    String::from("### Parsed Errors\nNo rustc-style errors were parsed.\n")
                } else {
                    format!(
                        "### Parsed Errors ({} found)\n{}\n",
                        parsed_count,
                        compiled.errors.join("\n"),
                    )
                };
                if let Some(ref excerpt) = compiled.diagnostic_excerpt {
                    error_section
                        .push_str(&format!("\n### Raw Diagnostic Excerpt\n{}\n", excerpt,));
                }
                let warning_note = if compiled.warning_count > 0 {
                    format!(
                        "\n\n({} warnings suppressed — warnings are not errors, focus on fixing the errors above.)",
                        compiled.warning_count
                    )
                } else {
                    String::new()
                };
                let compile_msg = format!(
                    "## Compilation Check\n\nProject type: {} | Command: `{}`\nExit code: {}\n\n❌ Compilation failed. Fix the errors below before running tests.\n\n{}{}",
                    project_type, cmd, exit_code, error_section, warning_note,
                );
                let fingerprint = compute_failure_fingerprint(&combined, "compile_error");
                let suggested = suggest_rust_diagnostic("compile_error", &combined, "all");
                let diag_grade = if compiled.diagnostic_excerpt.is_some() {
                    "raw_excerpt"
                } else {
                    "structured"
                };
                ToolOutput::diagnostic_error(
                    "check_compile",
                    "",
                    "compile_error",
                    &compile_msg,
                    diag_grade,
                    &fingerprint,
                    true,
                    suggested,
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
            description: "Check if the project compiles without running tests. Rust: cargo check compiles library and binary targets only — it does NOT compile #[cfg(test)] test code. Use run_tests to compile and run test targets. Returns structured error list with file locations.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "include_tests": {
                        "type": "boolean",
                        "description": "If true, compile test targets too (cargo test --no-run for Rust). Use this when you need to verify test code compiles before running tests."
                    }
                }
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_test_compile_cmd_unit() {
        let cmd = rust_test_compile_cmd("unit");
        assert!(cmd.contains("test --lib --no-run"));
    }

    #[test]
    fn test_rust_test_compile_cmd_integration() {
        let cmd = rust_test_compile_cmd("integration");
        assert!(cmd.contains("test --tests --no-run"));
    }

    #[test]
    fn test_rust_test_compile_cmd_all() {
        let cmd = rust_test_compile_cmd("all");
        assert!(cmd.contains("test --no-run"));
    }

    #[test]
    fn test_rust_test_compile_cmd_unknown_scope() {
        let cmd = rust_test_compile_cmd("whatever");
        assert!(cmd.contains("test --no-run"));
    }

    #[test]
    fn test_classify_compile_output_extracts_error_with_location() {
        let output = "\nerror[E0283]: type annotations needed for `LruCache<_, _>`\n  --> src/lib.rs:215:13\n";
        let result = classify_compile_output(output, 1);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("E0283"));
        assert!(result.errors[0].contains("src/lib.rs:215:13"));
        assert!(result.errors[0].contains("type annotations needed"));
        assert!(result.diagnostic_excerpt.is_none());
    }

    #[test]
    fn test_classify_compile_output_classic_error_format() {
        let output = "error: could not compile `foo`\n  --> src/main.rs:10:5\n";
        let result = classify_compile_output(output, 1);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("could not compile"));
    }

    #[test]
    fn test_classify_compile_output_multiple_errors() {
        let output = "\
error[E0308]: mismatched types
  --> src/main.rs:12:8
error[E0277]: trait bound not satisfied
  --> src/lib.rs:34:12
";
        let result = classify_compile_output(output, 1);
        assert_eq!(result.errors.len(), 2);
        assert!(result.errors[0].contains("E0308"));
        assert!(result.errors[1].contains("E0277"));
    }

    #[test]
    fn test_classify_compile_output_counts_warnings() {
        let output = "\
warning: unused variable
warning: unused import
";
        let result = classify_compile_output(output, 0);
        assert_eq!(result.warning_count, 2);
    }

    #[test]
    fn test_classify_compile_output_parsed_zero_shows_diagnostic_excerpt() {
        let output = "Caused by:\n  拒绝访问。 (os error 5)\n";
        let result = classify_compile_output(output, 1);
        assert!(result.errors.is_empty());
        assert!(result.diagnostic_excerpt.is_some());
        let excerpt = result.diagnostic_excerpt.unwrap();
        assert!(excerpt.contains("拒绝访问"));
    }

    #[test]
    fn test_classify_execution_failure_cargo_lock() {
        let output = "error: failed to open: target/debug/.cargo-lock\nCaused by:\n  拒绝访问。";
        assert_eq!(
            classify_execution_failure(output),
            Some("environment_error")
        );
    }

    #[test]
    fn test_classify_execution_failure_permission_denied() {
        let output = "error: Permission denied (os error 13)";
        assert_eq!(
            classify_execution_failure(output),
            Some("environment_error")
        );
    }

    #[test]
    fn test_classify_execution_failure_access_denied() {
        let output = "Access is denied.";
        assert_eq!(
            classify_execution_failure(output),
            Some("environment_error")
        );
    }

    #[test]
    fn test_classify_execution_failure_chinese() {
        let output = "拒绝访问。 (os error 5)";
        assert_eq!(
            classify_execution_failure(output),
            Some("environment_error")
        );
    }

    #[test]
    fn test_classify_execution_failure_none_for_code_error() {
        let output = "error[E0283]: type annotations needed";
        assert_eq!(classify_execution_failure(output), None);
    }

    #[test]
    fn test_classify_execution_failure_failed_to_open() {
        let output = "error: failed to open: some_file.rs";
        assert_eq!(
            classify_execution_failure(output),
            Some("environment_error")
        );
    }

    #[test]
    fn test_extract_diagnostic_excerpt_prioritizes_error_lines() {
        let output = "\
Compiling foo v0.1.0
error: could not compile `foo`
Caused by:
  permission denied
  \\-- some normal line
";
        let excerpt = extract_diagnostic_excerpt(output, 3000);
        assert!(excerpt.contains("could not compile"));
        assert!(excerpt.contains("Caused by:"));
        assert!(excerpt.contains("permission denied"));
    }

    #[test]
    fn test_extract_diagnostic_excerpt_max_chars() {
        let output = (0..100)
            .map(|i| format!("error: line{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let excerpt = extract_diagnostic_excerpt(&output, 500);
        assert!(excerpt.len() <= 500);
        assert!(excerpt.starts_with("error: line"));
    }

    #[test]
    fn test_smart_truncate_stderr_preserves_key_lines() {
        let stderr = "\
   Compiling foo v0.1.0
error[E0283]: type annotations needed
  --> src/lib.rs:215:13
   Compiling bar v0.2.0
";
        let truncated = smart_truncate_stderr(stderr, 200);
        assert!(truncated.contains("E0283"));
        assert!(truncated.contains("src/lib.rs:215:13"));
    }

    #[test]
    fn test_smart_truncate_stderr_with_env_errors() {
        let stderr = "error: failed to open: target/debug/.cargo-lock\nCaused by:\n  拒绝访问。 (os error 5)\n";
        let truncated = smart_truncate_stderr(stderr, 1000);
        assert!(truncated.contains(".cargo-lock"));
        assert!(truncated.contains("拒绝访问"));
    }

    #[test]
    fn test_smart_truncate_stderr_falls_back_to_head() {
        let stderr = "normal line\n".repeat(100);
        let truncated = smart_truncate_stderr(&stderr, 200);
        assert!(truncated.contains("[Truncated"));
    }

    #[test]
    fn test_smart_truncate_stderr_short_no_truncation() {
        let stderr = "error[E0283]: type annotations needed";
        let truncated = smart_truncate_stderr(stderr, 1000);
        assert_eq!(truncated, stderr);
    }

    #[test]
    fn test_parse_test_output_rust() {
        let stdout = "\
test test_create_user ... ok
test test_delete_user ... FAILED

test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
";
        let (passed, failed, total) = parse_test_output(stdout, "");
        assert_eq!(passed, Some(7));
        assert_eq!(failed, Some(1));
        assert!(total.unwrap() >= 2);
    }

    #[test]
    fn test_parse_test_output_pytest() {
        let stdout = "= 5 passed, 2 failed in 0.05s =\n";
        let (passed, failed, _) = parse_test_output(stdout, "");
        assert_eq!(passed, Some(5));
        assert_eq!(failed, Some(2));
    }

    #[test]
    fn test_parse_test_output_empty() {
        let (passed, failed, total) = parse_test_output("No tests found", "");
        assert!(passed.is_none());
        assert!(failed.is_none());
        assert!(total.is_none());
    }

    // ── P2: failure_fingerprint tests ───────────────────────────────────

    #[test]
    fn test_compute_fingerprint_rustc_error_with_location() {
        let output = "\nerror[E0283]: type annotations needed\n  --> src/lib.rs:215:13\n";
        let fp = compute_failure_fingerprint(output, "compile_error");
        assert!(fp.starts_with("compile_error|rustc|E0283|"));
        assert!(fp.contains("src/lib.rs"));
    }

    #[test]
    fn test_compute_fingerprint_rustc_error_no_location() {
        let output = "error[E0308]: mismatched types\n";
        let fp = compute_failure_fingerprint(output, "compile_error");
        assert_eq!(fp, "compile_error|rustc|E0308");
    }

    #[test]
    fn test_compute_fingerprint_classic_error() {
        let output = "error: could not compile `foo`\n";
        let fp = compute_failure_fingerprint(output, "compile_error");
        assert!(fp.starts_with("compile_error|rustc_msg|"));
        assert!(fp.contains("could not compile"));
    }

    #[test]
    fn test_compute_fingerprint_test_failure() {
        let output = "test test_create_user ... FAILED\n";
        let fp = compute_failure_fingerprint(output, "test_failure");
        assert_eq!(fp, "test_failure|test_fail|test_create_user");
    }

    #[test]
    fn test_compute_fingerprint_environment_error_cargo_lock() {
        let output = "error: failed to open: target/debug/.cargo-lock\nCaused by:\n  拒绝访问。\n";
        let fp = compute_failure_fingerprint(output, "environment_error");
        assert_eq!(fp, "environment_error|env|.cargo-lock");
    }

    #[test]
    fn test_compute_fingerprint_environment_error_permission() {
        let output = "error: Permission denied (os error 13)\n";
        let fp = compute_failure_fingerprint(output, "environment_error");
        // "Permission denied" starts with "error:" so it's caught by the classic
        // error path first, producing rustc_msg fingerprint
        assert!(fp.starts_with("environment_error|rustc_msg|"));
        assert!(fp.contains("Permission"));
    }

    #[test]
    fn test_compute_fingerprint_unknown_fallback() {
        let output = "Something went wrong but no clear pattern\n";
        let fp = compute_failure_fingerprint(output, "tool_error");
        assert_eq!(fp, "tool_error|unknown");
    }

    // ── suggest_rust_diagnostic tests ──────────────────────────────────

    #[test]
    fn test_suggest_rust_diagnostic_compile_error() {
        let output = "error[E0283]: type annotations needed\n";
        let suggested = suggest_rust_diagnostic("compile_error", output, "unit");
        assert!(suggested
            .iter()
            .any(|s| s.contains("rustc --explain E0283")));
        assert!(suggested
            .iter()
            .any(|s| s.contains("cargo test --lib --no-run")));
    }

    #[test]
    fn test_suggest_rust_diagnostic_compile_error_integration() {
        let output = "error[E0308]: mismatched types\n";
        let suggested = suggest_rust_diagnostic("compile_error", output, "integration");
        assert!(suggested
            .iter()
            .any(|s| s.contains("rustc --explain E0308")));
        assert!(suggested
            .iter()
            .any(|s| s.contains("cargo test --tests --no-run")));
    }

    #[test]
    fn test_suggest_rust_diagnostic_test_failure() {
        let output = "test result: FAILED. 7 passed; 1 failed\n";
        let suggested = suggest_rust_diagnostic("test_failure", output, "unit");
        assert!(suggested.iter().any(|s| s.contains("--nocapture")));
    }

    #[test]
    fn test_suggest_rust_diagnostic_environment_error() {
        let output = "error: failed to open: target/debug/.cargo-lock\n";
        let suggested = suggest_rust_diagnostic("environment_error", output, "all");
        assert!(suggested.iter().any(|s| s.contains("permissions")));
        assert!(suggested.iter().any(|s| s.contains(".cargo-lock")));
    }

    #[test]
    fn test_suggest_rust_diagnostic_unknown() {
        let suggested = suggest_rust_diagnostic("unknown_error", "", "all");
        assert!(suggested.is_empty());
    }

    // ── P2: diagnostic_error output tests ──────────────────────────────

    #[test]
    fn test_diagnostic_error_includes_all_fields() {
        let result = ToolOutput::diagnostic_error(
            "run_tests",
            "",
            "compile_error",
            "Compilation failed",
            "structured",
            "compile_error|rustc|E0283|lib.rs",
            true,
            vec!["rustc --explain E0283".to_string()],
        );
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error_code"], "compile_error");
        assert_eq!(v["diagnostic_grade"], "structured");
        assert_eq!(v["failure_fingerprint"], "compile_error|rustc|E0283|lib.rs");
        assert_eq!(v["should_modify_code"], true);
        assert_eq!(v["suggested_next"][0], "rustc --explain E0283");
    }

    #[test]
    fn test_diagnostic_error_environment() {
        let result = ToolOutput::diagnostic_error(
            "run_tests",
            "",
            "environment_error",
            "Environment issue detected",
            "raw_excerpt",
            "environment_error|env|.cargo-lock",
            false,
            vec![],
        );
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error_code"], "environment_error");
        assert_eq!(v["should_modify_code"], false);
        assert_eq!(v["diagnostic_grade"], "raw_excerpt");
        // suggested_next should not be present when empty
        assert!(v.get("suggested_next").is_none());
    }
}
