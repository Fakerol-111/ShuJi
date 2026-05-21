use std::path::Path;

/// Create a test file with given content.
pub fn create_test_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
}

/// Create a test directory structure.
pub fn create_test_structure(root: &Path, structure: &[(&str, &str)]) {
    for (path, content) in structure {
        create_test_file(root, path, content);
    }
}

/// Create a symlink (for testing symlink escape attacks).
#[cfg(unix)]
pub fn create_symlink(src: &Path, dst: &Path) {
    std::os::unix::fs::symlink(src, dst).unwrap();
}

#[cfg(windows)]
pub fn create_symlink(src: &Path, dst: &Path) {
    std::os::windows::fs::symlink_file(src, dst).unwrap();
}
