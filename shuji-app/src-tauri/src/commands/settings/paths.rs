//! Path helpers and .env role-prefix mapping.

use std::collections::HashMap;

pub(crate) fn dotenv_path() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".env")
}

pub(crate) fn dotenv_path_parent() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join(".env")
}

pub(crate) fn api_config_path() -> std::path::PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let candidate = cwd.join("api_config.json");
    if candidate.exists() {
        return candidate;
    }
    if let Some(parent) = cwd.parent() {
        let parent_candidate = parent.join("api_config.json");
        if parent_candidate.exists() {
            return parent_candidate;
        }
    }
    cwd.join("api_config.json")
}

pub(crate) fn context_config_path() -> std::path::PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let candidate = cwd.join("context_config.json");
    if candidate.exists() {
        return candidate;
    }
    if let Some(parent) = cwd.parent() {
        let parent_candidate = parent.join("context_config.json");
        if parent_candidate.exists() {
            return parent_candidate;
        }
    }
    cwd.join("context_config.json")
}

pub(crate) fn workflow_preset_path(project_dir: &str) -> std::path::PathBuf {
    std::path::Path::new(project_dir)
        .join(".shuji")
        .join("workflow_preset.json")
}

/// (short_prefix, canonical_role_name) — maps .env.template short names
/// to the canonical names used by `for_role()` and `build_agents()`.
pub(crate) const ROLE_PREFIXES: &[(&str, &str)] = &[
    ("DEFAULT", "default"),
    ("MENXIA", "menxiashizhong"),
    ("ZHONGSHU", "zhongshuling"),
    ("NEIGE", "neige"),
    ("SHANGSHU", "shangshuling"),
    ("LIBUP", "libushangshu"),
    ("HUBU", "hubu"),
    ("LIBUR", "liburshangshu"),
    ("BINGBU", "bingbushangshu"),
    ("XINGBU", "xingbushangshu"),
    ("GONGBU", "gongbushangshu"),
];

pub(crate) fn prefix_for_role(role_name: &str) -> &str {
    for (prefix, name) in ROLE_PREFIXES {
        if *name == role_name {
            return prefix;
        }
    }
    role_name
}

/// Read .env file — supports KEY=VALUE, # comments, skips blanks.
pub(crate) fn load_dotenv() -> HashMap<String, String> {
    let paths = [dotenv_path(), dotenv_path_parent()];
    for path in &paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let mut vars = HashMap::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some(eq) = trimmed.find('=') {
                    let key = trimmed[..eq].trim().to_string();
                    let val = trimmed[eq + 1..].trim().to_string();
                    vars.insert(key, val);
                }
            }
            log_console!(
                "[debug] loaded .env from {} ({} vars)",
                path.display(),
                vars.len()
            );
            return vars;
        }
    }
    log_console!(
        "[debug] no .env found (cwd={})",
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
    HashMap::new()
}
