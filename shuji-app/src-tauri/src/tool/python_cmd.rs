//! Cross-platform Python interpreter detection for shell commands.

use std::path::Path;
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

/// Detect the venv Python path if `.venv` exists in `working_dir`,
/// otherwise fall back to the system Python.
///
/// This ensures `run_tests` and `pytest_cmd` use the same Python that
/// `setup_test_env` installed dependencies into.
pub fn venv_python_or_system(working_dir: &Path) -> String {
    let venv_py = if cfg!(windows) {
        working_dir.join(".venv").join("Scripts").join("python")
    } else {
        working_dir.join(".venv").join("bin").join("python")
    };
    if venv_py.exists() {
        venv_py.to_string_lossy().to_string()
    } else {
        python_command()
    }
}

/// Build a pytest command for the given test scope.
///
/// If `.venv` exists in `working_dir`, the venv Python is used so that
/// dependencies installed by `setup_test_env` are available.
pub fn pytest_cmd(scope: &str, working_dir: &Path) -> String {
    let py = venv_python_or_system(working_dir);
    match scope {
        "unit" => format!("{} -m pytest tests/ -v", py),
        "integration" => format!("{} -m pytest tests/integration/ -v", py),
        _ => format!("{} -m pytest -v", py),
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
        let tmp = tempfile::TempDir::new().unwrap();
        let cmd = pytest_cmd("all", tmp.path());
        assert!(cmd.contains("pytest"));
        assert!(cmd.starts_with("python") || cmd.starts_with("py "));
    }

    #[test]
    fn pytest_cmd_uses_venv_when_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Without venv → system python
        let cmd = pytest_cmd("all", tmp.path());
        assert!(cmd.contains("pytest"));
        assert!(!cmd.contains(".venv"));

        // Simulate venv existence
        let venv_bin = if cfg!(windows) {
            tmp.path().join(".venv").join("Scripts")
        } else {
            tmp.path().join(".venv").join("bin")
        };
        std::fs::create_dir_all(&venv_bin).unwrap();
        std::fs::write(venv_bin.join("python"), "").unwrap();
        let cmd = pytest_cmd("all", tmp.path());
        assert!(cmd.contains(".venv"), "should use venv python: {}", cmd);
    }

    #[test]
    fn venv_python_or_system_falls_back_without_venv() {
        let tmp = tempfile::TempDir::new().unwrap();
        let py = venv_python_or_system(tmp.path());
        assert!(!py.is_empty());
        assert!(!py.contains(".venv"));
    }

    #[test]
    fn venv_create_cmd_contains_venv() {
        let cmd = venv_create_cmd();
        assert!(cmd.contains("venv"));
        assert!(cmd.contains(".venv"));
    }
}
