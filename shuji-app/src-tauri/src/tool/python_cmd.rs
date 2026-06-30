//! Cross-platform Python interpreter detection for shell commands.

use std::sync::OnceLock;

static PYTHON_CMD: OnceLock<String> = OnceLock::new();

/// Returns the preferred Python interpreter command for shell execution.
///
/// Non-Windows: prefers `python3`, falls back to `python`.
/// Windows: prefers `python`, then `py`, then `python3`.
pub fn python_command() -> String {
    PYTHON_CMD.get_or_init(detect_python_command).clone()
}

fn detect_python_command() -> String {
    let candidates: &[&str] = if cfg!(windows) {
        &["python", "py", "python3"]
    } else {
        &["python3", "python"]
    };

    for cmd in candidates {
        if command_available(cmd) {
            return cmd.to_string();
        }
    }

    candidates[0].to_string()
}

fn command_available(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Build a `python -m <module> <args>` command using the detected interpreter.
pub fn python_module_cmd(module: &str, args: &str) -> String {
    format!("{} -m {} {}", python_command(), module, args)
}

/// Build a pytest command for the given test scope.
pub fn pytest_cmd(scope: &str) -> String {
    match scope {
        "unit" => python_module_cmd("pytest", "tests/ -v"),
        "integration" => python_module_cmd("pytest", "tests/integration/ -v"),
        _ => python_module_cmd("pytest", "-v"),
    }
}

/// Build a venv creation command.
pub fn venv_create_cmd() -> String {
    python_module_cmd("venv", ".venv")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_command_returns_non_empty() {
        let cmd = python_command();
        assert!(!cmd.is_empty());
        assert!(cmd == "python" || cmd == "python3" || cmd == "py");
    }

    #[test]
    fn pytest_cmd_contains_pytest() {
        let cmd = pytest_cmd("all");
        assert!(cmd.contains("pytest"));
        assert!(cmd.starts_with("python") || cmd.starts_with("py "));
    }

    #[test]
    fn venv_create_cmd_contains_venv() {
        let cmd = venv_create_cmd();
        assert!(cmd.contains("venv"));
        assert!(cmd.contains(".venv"));
    }
}
