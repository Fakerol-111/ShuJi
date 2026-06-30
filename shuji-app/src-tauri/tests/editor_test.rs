//! External IDE integration tests — path safety and spawn behavior.

mod common;

use shuji_app_lib::tool::editor::{
    build_editor_args, check_editor_available, load_editor_config, open_file_in_editor,
    open_project_in_editor, resolve_editor_command, save_editor_config,
    set_test_editor_config_path, EditorConfig, EditorKind,
};
use shuji_app_lib::tool::resolve_scoped_path;
use std::path::{Path, PathBuf};

fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(resolve_scoped_path(root, rel))
}

fn open_editor(root: &Path, rel: &str) -> Result<(), String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(open_file_in_editor(root, rel, None))
}

fn open_project(root: &Path) -> Result<(), String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(open_project_in_editor(root))
}

struct EditorTestGuard;

impl EditorTestGuard {
    fn new(temp: &tempfile::TempDir) -> Self {
        let config_path = temp.path().join("editor_config.json");
        set_test_editor_config_path(Some(config_path));
        Self
    }
}

impl Drop for EditorTestGuard {
    fn drop(&mut self) {
        set_test_editor_config_path(None);
    }
}

#[test]
fn test_reject_absolute_path_for_open() {
    let temp = common::create_test_project("editor_abs");
    let root = temp.path();

    #[cfg(windows)]
    let bad_path = "C:\\Windows\\System32\\cmd.exe";
    #[cfg(not(windows))]
    let bad_path = "/etc/passwd";

    let result = resolve(root, bad_path);
    common::assert_path_error_contains(&result, "absolute paths forbidden");
}

#[test]
fn test_reject_parent_traversal_for_open() {
    let temp = common::create_test_project("editor_traversal");
    let root = temp.path();

    let result = resolve(root, "../outside.txt");
    common::assert_path_error_contains(&result, "parent directory traversal forbidden");
}

#[test]
#[cfg(windows)]
fn test_reject_windows_drive_for_open() {
    let temp = common::create_test_project("editor_drive");
    let root = temp.path();

    let result = resolve(root, "C:\\Windows\\System32\\cmd.exe");
    assert!(result.is_err(), "drive letter path should be rejected");
}

#[test]
fn test_reject_nonexistent_file() {
    let temp = common::create_test_project("editor_missing");
    let root = temp.path();
    let _guard = EditorTestGuard::new(&temp);

    save_editor_config(&EditorConfig {
        editor: EditorKind::Vscode,
        custom_command: None,
        reuse_window: true,
    })
    .unwrap();

    let result = open_editor(root, "src/does_not_exist.rs");
    assert!(result.is_err());
    assert!(
        result.unwrap_err().contains("not found"),
        "expected file-not-found error"
    );
}

#[test]
fn test_resolve_normal_project_file() {
    let temp = common::create_test_project("editor_normal");
    let root = temp.path();

    let resolved = resolve(root, "src/main.rs").unwrap();
    assert_eq!(resolved, root.join("src/main.rs"));
}

#[test]
fn test_build_editor_args_goto_line() {
    let config = EditorConfig {
        editor: EditorKind::Cursor,
        custom_command: None,
        reuse_window: false,
    };
    let args = build_editor_args(&config, Path::new("/project/foo.rs"), Some(10));
    assert_eq!(args, vec!["--goto", "/project/foo.rs:10"]);
}

#[test]
fn test_editor_config_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let _guard = EditorTestGuard::new(&temp);

    let config = EditorConfig {
        editor: EditorKind::Custom,
        custom_command: Some("my-editor".into()),
        reuse_window: false,
    };
    save_editor_config(&config).unwrap();
    let loaded = load_editor_config();
    assert_eq!(loaded, config);
}

#[test]
fn test_check_editor_available_with_absolute_custom_command() {
    let temp = tempfile::tempdir().unwrap();
    let fake_editor = temp.path().join(if cfg!(windows) {
        "check_editor.cmd"
    } else {
        "check_editor"
    });
    std::fs::write(&fake_editor, "").unwrap();

    let config = EditorConfig {
        editor: EditorKind::Custom,
        custom_command: Some(fake_editor.to_string_lossy().into_owned()),
        reuse_window: true,
    };

    let result = check_editor_available(&config);
    assert!(result.is_ok(), "expected custom editor check to pass");
}

#[test]
fn test_check_editor_reports_missing_command() {
    let config = EditorConfig {
        editor: EditorKind::Custom,
        custom_command: Some("definitely-missing-shuji-editor".into()),
        reuse_window: true,
    };

    let err = check_editor_available(&config).unwrap_err();
    assert!(
        err.contains("editor command") && err.contains("not found"),
        "expected missing command error, got: {err}"
    );
}

#[test]
fn test_check_editor_rejects_empty_custom_command() {
    let config = EditorConfig {
        editor: EditorKind::Custom,
        custom_command: Some(" ".into()),
        reuse_window: true,
    };

    let err = check_editor_available(&config).unwrap_err();
    assert!(
        err.contains("custom editor command cannot be empty"),
        "expected validation error, got: {err}"
    );
}

#[test]
#[cfg(windows)]
fn test_windows_resolves_custom_command_without_cmd_extension() {
    let temp = tempfile::tempdir().unwrap();
    let command_without_ext = temp.path().join("fake_code");
    let command_with_ext = command_without_ext.with_extension("cmd");
    std::fs::write(&command_with_ext, "").unwrap();

    let config = EditorConfig {
        editor: EditorKind::Custom,
        custom_command: Some(command_without_ext.to_string_lossy().into_owned()),
        reuse_window: true,
    };

    let resolved = resolve_editor_command(&config).unwrap();
    assert_eq!(
        resolved.to_string_lossy().to_lowercase(),
        command_with_ext.to_string_lossy().to_lowercase()
    );
}

#[test]
fn test_spawn_with_fake_editor() {
    let temp = common::create_test_project("editor_spawn");
    let root = temp.path();
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let file_path = src_dir.join("main.rs");
    std::fs::write(&file_path, "fn main() {}").unwrap();

    let args_file = temp.path().join("spawn_args.txt");
    let fake_editor = temp.path().join(if cfg!(windows) {
        "fake_editor.cmd"
    } else {
        "fake_editor.sh"
    });

    if cfg!(windows) {
        std::fs::write(
            &fake_editor,
            format!(
                "@echo off\r\n echo %* > \"{}\"\r\n",
                args_file.to_string_lossy()
            ),
        )
        .unwrap();
    } else {
        std::fs::write(
            &fake_editor,
            format!("#!/bin/sh\n echo \"$@\" > \"{}\"\n", args_file.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&fake_editor).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&fake_editor, perms).unwrap();
        }
    }

    let _guard = EditorTestGuard::new(&temp);
    save_editor_config(&EditorConfig {
        editor: EditorKind::Custom,
        custom_command: Some(fake_editor.to_string_lossy().into_owned()),
        reuse_window: true,
    })
    .unwrap();

    open_editor(root, "src/main.rs").expect("fake editor spawn should succeed");

    std::thread::sleep(std::time::Duration::from_millis(200));
    let captured = std::fs::read_to_string(&args_file).unwrap_or_default();
    assert!(
        captured.contains("--reuse-window"),
        "expected --reuse-window in spawn args, got: {captured}"
    );
    assert!(
        captured.contains("--goto"),
        "expected --goto in spawn args, got: {captured}"
    );
    assert!(
        captured.contains("main.rs"),
        "expected file path in spawn args, got: {captured}"
    );
}

#[test]
fn test_spawn_project_root_with_fake_editor() {
    let temp = common::create_test_project("editor_project_root");
    let root = temp.path();

    let args_file = temp.path().join("project_spawn_args.txt");
    let fake_editor = temp.path().join(if cfg!(windows) {
        "fake_editor_project.cmd"
    } else {
        "fake_editor_project.sh"
    });

    if cfg!(windows) {
        std::fs::write(
            &fake_editor,
            format!(
                "@echo off\r\n echo %* > \"{}\"\r\n",
                args_file.to_string_lossy()
            ),
        )
        .unwrap();
    } else {
        std::fs::write(
            &fake_editor,
            format!("#!/bin/sh\n echo \"$@\" > \"{}\"\n", args_file.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&fake_editor).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&fake_editor, perms).unwrap();
        }
    }

    let _guard = EditorTestGuard::new(&temp);
    save_editor_config(&EditorConfig {
        editor: EditorKind::Custom,
        custom_command: Some(fake_editor.to_string_lossy().into_owned()),
        reuse_window: true,
    })
    .unwrap();

    open_project(root).expect("fake editor project spawn should succeed");

    std::thread::sleep(std::time::Duration::from_millis(200));
    let captured = std::fs::read_to_string(&args_file).unwrap_or_default();
    assert!(
        captured.contains("--reuse-window"),
        "expected --reuse-window in spawn args, got: {captured}"
    );
    assert!(
        !captured.contains("--goto"),
        "project root should not use --goto, got: {captured}"
    );
}
