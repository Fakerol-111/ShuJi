use std::cell::RefCell;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::path::resolve_scoped_path;

thread_local! {
    static TEST_CONFIG_PATH_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EditorKind {
    Vscode,
    Cursor,
    Trae,
    Zed,
    Sublime,
    Jetbrains,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EditorConfig {
    pub editor: EditorKind,
    #[serde(default)]
    pub custom_command: Option<String>,
    #[serde(default = "default_true")]
    pub reuse_window: bool,
}

fn default_true() -> bool {
    true
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            editor: EditorKind::Vscode,
            custom_command: None,
            reuse_window: true,
        }
    }
}

/// Test-only override for editor config file path (thread-local, parallel-safe).
#[doc(hidden)]
pub fn set_test_editor_config_path(path: Option<PathBuf>) {
    TEST_CONFIG_PATH_OVERRIDE.with(|cell| *cell.borrow_mut() = path);
}

fn shuji_home() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".shuji")
}

fn editor_config_path() -> PathBuf {
    if let Some(path) = TEST_CONFIG_PATH_OVERRIDE.with(|cell| cell.borrow().clone()) {
        return path;
    }
    shuji_home().join("editor_config.json")
}

pub fn try_load_editor_config() -> Result<EditorConfig, String> {
    let path = editor_config_path();
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取编辑器配置失败 ({}): {}", path.display(), e))?;
    serde_json::from_str(&content).map_err(|e| format!("编辑器配置 JSON 解析失败: {}", e))
}

pub fn load_editor_config() -> EditorConfig {
    match try_load_editor_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            log_console!("[editor] load_editor_config fallback to default: {}", e);
            EditorConfig::default()
        }
    }
}

pub fn save_editor_config(config: &EditorConfig) -> Result<(), String> {
    validate_editor_config(config)?;
    let path = editor_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create config directory: {}", e))?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("failed to serialize config: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("failed to write editor config: {}", e))
}

pub fn validate_editor_config(config: &EditorConfig) -> Result<(), String> {
    if matches!(config.editor, EditorKind::Custom) {
        let cmd = config.custom_command.as_deref().unwrap_or("").trim();
        if cmd.is_empty() {
            return Err("custom editor command cannot be empty".into());
        }
        validate_custom_command(cmd)?;
    }
    Ok(())
}

pub fn validate_custom_command(cmd: &str) -> Result<(), String> {
    const FORBIDDEN: &[char] = &[
        ';', '|', '&', '>', '<', '\n', '\r', '`', '$', '(', ')', '"', '\'',
    ];
    if cmd.chars().any(|c| FORBIDDEN.contains(&c)) {
        return Err("custom command must not contain shell metacharacters".into());
    }
    if cmd.split_whitespace().count() > 1 {
        return Err(
            "custom command must be a single executable name or path without arguments".into(),
        );
    }
    Ok(())
}

pub fn resolve_editor_executable(config: &EditorConfig) -> Result<String, String> {
    match config.editor {
        EditorKind::Vscode => Ok("code".into()),
        EditorKind::Cursor => Ok("cursor".into()),
        EditorKind::Trae => Ok("trae".into()),
        EditorKind::Zed => Ok("zed".into()),
        EditorKind::Sublime => Ok("subl".into()),
        EditorKind::Jetbrains => Ok("idea".into()),
        EditorKind::Custom => {
            let cmd = config.custom_command.as_deref().unwrap_or("").trim();
            validate_custom_command(cmd)?;
            Ok(cmd.to_string())
        }
    }
}

fn has_path_separator(cmd: &str) -> bool {
    cmd.contains('/') || cmd.contains('\\')
}

fn executable_candidates(path: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        if path.extension().is_none() {
            let pathext =
                std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
            let mut candidates = Vec::new();
            for ext in pathext.split(';') {
                let ext = ext.trim();
                if ext.is_empty() {
                    continue;
                }
                let ext = ext.trim_start_matches('.');
                candidates.push(path.with_extension(ext));
            }
            // Extensionless launchers (e.g. VS Code's `code` shell script) are not Win32 executables.
            candidates.push(path.to_path_buf());
            return candidates;
        }
    }

    vec![path.to_path_buf()]
}

#[cfg(windows)]
fn is_windows_launchable(path: &Path) -> bool {
    path.extension()
        .map(|ext| {
            matches!(
                ext.to_string_lossy().to_ascii_lowercase().as_str(),
                "exe" | "cmd" | "bat" | "com"
            )
        })
        .unwrap_or(false)
}

fn find_existing_executable(path: &Path) -> Option<PathBuf> {
    let candidates = executable_candidates(path);

    #[cfg(windows)]
    {
        if let Some(found) = candidates
            .iter()
            .find(|candidate| candidate.is_file() && is_windows_launchable(candidate))
        {
            return Some(found.clone());
        }
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn find_in_dirs(cmd: &str, dirs: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    dirs.into_iter()
        .find_map(|dir| find_existing_executable(&dir.join(cmd)))
}

fn path_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(paths) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&paths));
    }

    #[cfg(windows)]
    {
        if let Some(extra) = windows_registry_path_dirs() {
            for dir in extra {
                if !dirs.iter().any(|existing| existing == &dir) {
                    dirs.push(dir);
                }
            }
        }
    }

    dirs
}

#[cfg(windows)]
fn windows_registry_path_dirs() -> Option<Vec<PathBuf>> {
    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Environment", "/v", "Path"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = stdout
        .lines()
        .find_map(|line| {
            line.split("REG_EXPAND_SZ")
                .nth(1)
                .or_else(|| line.split("REG_SZ").nth(1))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    Some(std::env::split_paths(value).collect())
}

#[cfg(windows)]
fn where_command(cmd: &str) -> Option<PathBuf> {
    let output = std::process::Command::new("where").arg(cmd).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<String> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();

    for line in &lines {
        let path = PathBuf::from(line);
        if path.is_file() && is_windows_launchable(&path) {
            return Some(path);
        }
    }

    lines
        .first()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn well_known_editor_bins(kind: &EditorKind) -> Vec<PathBuf> {
    let mut bins = Vec::new();
    let local_app_data = env_path("LOCALAPPDATA");
    let program_files = env_path("ProgramFiles");
    let program_files_x86 = env_path("ProgramFiles(x86)");

    match kind {
        EditorKind::Vscode => {
            if let Some(base) = local_app_data {
                bins.push(base.join("Programs/Microsoft VS Code/bin/code.cmd"));
            }
            if let Some(base) = program_files {
                bins.push(base.join("Microsoft VS Code/bin/code.cmd"));
            }
            if let Some(base) = program_files_x86 {
                bins.push(base.join("Microsoft VS Code/bin/code.cmd"));
            }
        }
        EditorKind::Cursor => {
            if let Some(base) = local_app_data {
                bins.push(base.join("Programs/cursor/resources/app/bin/cursor.cmd"));
                bins.push(base.join("Programs/Cursor/resources/app/bin/cursor.cmd"));
            }
            if let Some(base) = program_files {
                bins.push(base.join("cursor/resources/app/bin/cursor.cmd"));
                bins.push(base.join("Cursor/resources/app/bin/cursor.cmd"));
            }
        }
        EditorKind::Trae => {
            if let Some(base) = local_app_data {
                bins.push(base.join("Programs/Trae/bin/trae.cmd"));
            }
        }
        EditorKind::Zed => {
            if let Some(base) = local_app_data {
                bins.push(base.join("Programs/Zed/bin/zed.exe"));
            }
        }
        EditorKind::Sublime => {
            if let Some(base) = program_files {
                bins.push(base.join("Sublime Text/subl.exe"));
                bins.push(base.join("Sublime Text 3/subl.exe"));
            }
        }
        EditorKind::Jetbrains => {
            if let Some(base) = program_files {
                bins.push(base.join("JetBrains/IntelliJ IDEA/bin/idea64.exe"));
            }
        }
        EditorKind::Custom => {}
    }

    #[cfg(target_os = "macos")]
    {
        match kind {
            EditorKind::Vscode => {
                bins.push(PathBuf::from("/usr/local/bin/code"));
                bins.push(PathBuf::from(
                    "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
                ));
            }
            EditorKind::Cursor => {
                bins.push(PathBuf::from("/usr/local/bin/cursor"));
                bins.push(PathBuf::from(
                    "/Applications/Cursor.app/Contents/Resources/app/bin/cursor",
                ));
            }
            EditorKind::Zed => {
                bins.push(PathBuf::from("/usr/local/bin/zed"));
                bins.push(PathBuf::from("/Applications/Zed.app/Contents/MacOS/zed"));
            }
            EditorKind::Sublime => {
                bins.push(PathBuf::from(
                    "/Applications/Sublime Text.app/Contents/SharedSupport/bin/subl",
                ));
            }
            _ => {}
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = env_path("HOME") {
            // Flatpak user exports
            bins.push(home.join(".local/share/flatpak/exports/bin/com.visualstudio.code"));
            bins.push(home.join(".local/share/flatpak/exports/bin/cursor"));
        }

        match kind {
            EditorKind::Vscode => {
                bins.push(PathBuf::from("/usr/bin/code"));
                bins.push(PathBuf::from("/usr/local/bin/code"));
                bins.push(PathBuf::from("/snap/bin/code"));
                bins.push(PathBuf::from(
                    "/var/lib/flatpak/exports/bin/com.visualstudio.code",
                ));
            }
            EditorKind::Cursor => {
                bins.push(PathBuf::from("/usr/bin/cursor"));
                bins.push(PathBuf::from("/usr/local/bin/cursor"));
                bins.push(PathBuf::from("/snap/bin/cursor"));
                bins.push(PathBuf::from("/var/lib/flatpak/exports/bin/cursor"));
            }
            EditorKind::Zed => {
                bins.push(PathBuf::from("/usr/bin/zed"));
                bins.push(PathBuf::from("/usr/local/bin/zed"));
            }
            EditorKind::Sublime => {
                bins.push(PathBuf::from("/usr/bin/subl"));
                bins.push(PathBuf::from("/usr/local/bin/subl"));
            }
            EditorKind::Jetbrains => {
                bins.push(PathBuf::from("/usr/bin/idea"));
                bins.push(PathBuf::from("/usr/local/bin/idea"));
            }
            EditorKind::Trae | EditorKind::Custom => {}
        }
    }

    bins
}

fn find_editor_executable(cmd: &str, kind: &EditorKind) -> Option<PathBuf> {
    let path = Path::new(cmd);
    if path.is_absolute() || has_path_separator(cmd) {
        return find_existing_executable(path);
    }

    if let Some(found) = find_in_dirs(cmd, path_search_dirs()) {
        return Some(found);
    }

    for candidate in well_known_editor_bins(kind) {
        if let Some(found) = find_existing_executable(&candidate) {
            return Some(found);
        }
    }

    #[cfg(windows)]
    if let Some(found) = where_command(cmd) {
        return Some(found);
    }

    None
}

pub fn resolve_editor_command(config: &EditorConfig) -> Result<PathBuf, String> {
    validate_editor_config(config)?;
    let editor = resolve_editor_executable(config)?;
    find_editor_executable(&editor, &config.editor).ok_or_else(|| {
        format!(
            "editor command '{editor}' not found; install it, add it to PATH, or configure a custom command in Settings"
        )
    })
}

pub fn check_editor_available(config: &EditorConfig) -> Result<String, String> {
    let command = resolve_editor_command(config)?;
    Ok(format!(
        "editor command '{}' is available",
        command.display()
    ))
}

fn format_path_with_line(target: &Path, line: Option<u32>) -> String {
    let base = target.display().to_string();
    match line {
        Some(l) => format!("{base}:{l}"),
        None => base,
    }
}

/// Build argv for spawning an external editor. Exported for tests.
pub fn build_spawn_args(
    config: &EditorConfig,
    target: &Path,
    line: Option<u32>,
    is_directory: bool,
) -> Vec<String> {
    let mut args = Vec::new();

    match config.editor {
        EditorKind::Vscode | EditorKind::Cursor | EditorKind::Trae | EditorKind::Custom => {
            if config.reuse_window {
                args.push("--reuse-window".into());
            }
            if is_directory {
                args.push(target.display().to_string());
            } else {
                args.push("--goto".into());
                args.push(format_path_with_line(target, line));
            }
        }
        EditorKind::Zed => {
            args.push(format_path_with_line(
                target,
                if is_directory { None } else { line },
            ));
        }
        EditorKind::Sublime => {
            args.push(format_path_with_line(
                target,
                if is_directory { None } else { line },
            ));
        }
        EditorKind::Jetbrains => {
            args.push(target.display().to_string());
            if !is_directory {
                if let Some(l) = line {
                    args.push("--line".into());
                    args.push(l.to_string());
                }
            }
        }
    }

    args
}

/// Backward-compatible helper used by existing tests.
pub fn build_editor_args(config: &EditorConfig, abs_path: &Path, line: Option<u32>) -> Vec<String> {
    build_spawn_args(config, abs_path, line, false)
}

fn spawn_error(editor: &str, e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        format!(
            "editor command '{editor}' not found; install it, add it to PATH, or configure a custom command in Settings"
        )
    } else {
        format!("failed to launch editor: {e}")
    }
}

pub async fn spawn_editor_at(
    config: &EditorConfig,
    target: &Path,
    line: Option<u32>,
    is_directory: bool,
) -> Result<(), String> {
    let editor = resolve_editor_command(config)?;
    let args = build_spawn_args(config, target, line, is_directory);

    #[cfg(windows)]
    {
        if editor
            .extension()
            .map(|ext| {
                matches!(
                    ext.to_string_lossy().to_ascii_lowercase().as_str(),
                    "cmd" | "bat"
                )
            })
            .unwrap_or(false)
        {
            let mut command = tokio::process::Command::new("cmd");
            command.arg("/C").arg(&editor).args(&args);
            return command
                .spawn()
                .map_err(|e| spawn_error(&editor.display().to_string(), &e))
                .map(|_| ());
        }
    }

    tokio::process::Command::new(&editor)
        .args(&args)
        .spawn()
        .map_err(|e| spawn_error(&editor.display().to_string(), &e))?;

    Ok(())
}

pub async fn spawn_editor(
    config: &EditorConfig,
    abs_path: &Path,
    line: Option<u32>,
) -> Result<(), String> {
    spawn_editor_at(config, abs_path, line, false).await
}

pub async fn open_file_in_editor(
    project_dir: &Path,
    rel_path: &str,
    line: Option<u32>,
) -> Result<(), String> {
    if !project_dir.is_dir() {
        return Err("invalid project directory".into());
    }

    let abs_path = resolve_scoped_path(project_dir, rel_path).await?;

    let meta = tokio::fs::metadata(&abs_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("file not found: {rel_path}")
        } else {
            format!("cannot read file metadata: {e}")
        }
    })?;

    if !meta.is_file() {
        return Err("only regular files can be opened in an external editor".into());
    }

    let config = load_editor_config();
    spawn_editor(&config, &abs_path, line).await
}

pub async fn open_project_in_editor(project_dir: &Path) -> Result<(), String> {
    if !project_dir.is_dir() {
        return Err("invalid project directory".into());
    }

    let abs_path = tokio::fs::canonicalize(project_dir)
        .await
        .map_err(|e| format!("project directory canonicalization failed: {e}"))?;

    let config = load_editor_config();
    spawn_editor_at(&config, &abs_path, None, true).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_with_line() {
        let config = EditorConfig {
            editor: EditorKind::Vscode,
            custom_command: None,
            reuse_window: true,
        };
        let args = build_editor_args(&config, Path::new("/tmp/foo.rs"), Some(42));
        assert_eq!(args[0], "--reuse-window");
        assert_eq!(args[1], "--goto");
        assert!(args[2].ends_with("foo.rs:42"));
    }

    #[test]
    fn zed_uses_path_colon_line() {
        let config = EditorConfig {
            editor: EditorKind::Zed,
            custom_command: None,
            reuse_window: true,
        };
        let args = build_spawn_args(&config, Path::new("/tmp/foo.rs"), Some(7), false);
        assert_eq!(args, vec!["/tmp/foo.rs:7"]);
    }

    #[test]
    fn jetbrains_uses_line_flag() {
        let config = EditorConfig {
            editor: EditorKind::Jetbrains,
            custom_command: None,
            reuse_window: false,
        };
        let args = build_spawn_args(&config, Path::new("/tmp/foo.rs"), Some(12), false);
        assert_eq!(args[0], "/tmp/foo.rs");
        assert_eq!(args[1], "--line");
        assert_eq!(args[2], "12");
    }

    #[test]
    fn project_root_vscode_no_goto() {
        let config = EditorConfig {
            editor: EditorKind::Vscode,
            custom_command: None,
            reuse_window: true,
        };
        let args = build_spawn_args(&config, Path::new("/tmp/project"), None, true);
        assert_eq!(args, vec!["--reuse-window", "/tmp/project"]);
    }

    #[test]
    fn rejects_custom_command_with_shell_metacharacters() {
        assert!(validate_custom_command("code --wait").is_err());
        assert!(validate_custom_command("cmd /C code").is_err());
        assert!(validate_custom_command("editor.exe").is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn prefers_cmd_over_shell_script_launcher() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("code");
        let cmd = temp.path().join("code.cmd");
        std::fs::write(&script, "#!/usr/bin/env sh\n").unwrap();
        std::fs::write(&cmd, "@echo off\r\n").unwrap();

        let resolved = find_existing_executable(&script).expect("expected cmd launcher");
        assert_eq!(
            resolved.to_string_lossy().to_lowercase(),
            cmd.to_string_lossy().to_lowercase()
        );
    }

    #[test]
    #[cfg(windows)]
    fn well_known_vscode_includes_local_appdata_bin() {
        let bins = well_known_editor_bins(&EditorKind::Vscode);
        assert!(
            bins.iter().any(|path| {
                path.to_string_lossy()
                    .to_lowercase()
                    .contains("microsoft vs code")
                    && path.to_string_lossy().to_lowercase().ends_with("code.cmd")
            }),
            "expected VS Code well-known path, got: {bins:?}"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn well_known_vscode_includes_linux_paths() {
        let bins = well_known_editor_bins(&EditorKind::Vscode);
        let paths: Vec<String> = bins
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        assert!(
            paths
                .iter()
                .any(|p| p.contains("/usr/bin/code") || p.contains("/snap/bin/code")),
            "expected Linux VS Code paths, got: {paths:?}"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn well_known_cursor_includes_linux_paths() {
        let bins = well_known_editor_bins(&EditorKind::Cursor);
        let paths: Vec<String> = bins
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        assert!(
            paths
                .iter()
                .any(|p| p.contains("/usr/bin/cursor") || p.contains("/snap/bin/cursor")),
            "expected Linux Cursor paths, got: {paths:?}"
        );
    }

    #[test]
    fn finds_absolute_custom_command() {
        let temp = tempfile::tempdir().unwrap();
        let editor = temp.path().join(if cfg!(windows) {
            "my_editor.cmd"
        } else {
            "my_editor"
        });
        std::fs::write(&editor, "").unwrap();

        let config = EditorConfig {
            editor: EditorKind::Custom,
            custom_command: Some(editor.to_string_lossy().into_owned()),
            reuse_window: true,
        };

        assert_eq!(resolve_editor_command(&config).unwrap(), editor);
    }
}
