use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use super::store::MAX_INJECTED_CHARS;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningConfig {
    #[serde(default = "default_true")]
    pub project_enabled: bool,
    #[serde(default)]
    pub global_enabled: bool,
    #[serde(default = "default_max_injected")]
    pub max_injected_chars_per_role: usize,
    #[serde(default = "default_true")]
    pub auto_extract: bool,
    #[serde(default = "default_true")]
    pub global_requires_approval: bool,
}

fn default_true() -> bool {
    true
}

fn default_max_injected() -> usize {
    MAX_INJECTED_CHARS
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            project_enabled: true,
            global_enabled: false,
            max_injected_chars_per_role: MAX_INJECTED_CHARS,
            auto_extract: true,
            global_requires_approval: true,
        }
    }
}

static CONFIG_CACHE: RwLock<Option<LearningConfig>> = RwLock::new(None);
static TEST_HOME_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);

thread_local! {
    static TEST_CONFIG_PATH_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn test_overrides_active() -> bool {
    let home = TEST_HOME_DIR.read().ok().is_some_and(|g| g.is_some());
    let cfg = TEST_CONFIG_PATH_OVERRIDE.with(|cell| cell.borrow().is_some());
    home || cfg
}

/// Test-only override for config file path (thread-local, parallel-safe).
#[doc(hidden)]
pub fn set_test_config_path(path: Option<PathBuf>) {
    TEST_CONFIG_PATH_OVERRIDE.with(|cell| *cell.borrow_mut() = path);
}

pub fn reset_config_cache() {
    if let Ok(mut guard) = CONFIG_CACHE.write() {
        *guard = None;
    }
}

pub fn config_path() -> Option<PathBuf> {
    if let Some(path) = TEST_CONFIG_PATH_OVERRIDE.with(|cell| cell.borrow().clone()) {
        return Some(path);
    }
    home_dir().map(|h| h.join(".shuji").join("learning_config.json"))
}

pub fn load_config() -> LearningConfig {
    if !test_overrides_active() {
        if let Ok(guard) = CONFIG_CACHE.read() {
            if let Some(ref cfg) = *guard {
                return cfg.clone();
            }
        }
    }

    let cfg = read_config_from_disk();
    if !test_overrides_active() {
        if let Ok(mut guard) = CONFIG_CACHE.write() {
            *guard = Some(cfg.clone());
        }
    }
    cfg
}

pub fn save_config(cfg: &LearningConfig) -> Result<(), String> {
    let path = config_path().ok_or_else(|| "Cannot resolve home directory".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;

    if !test_overrides_active() {
        if let Ok(mut guard) = CONFIG_CACHE.write() {
            *guard = Some(cfg.clone());
        }
    }
    Ok(())
}

pub fn set_global_enabled(enabled: bool) -> Result<(), String> {
    let mut cfg = load_config();
    cfg.global_enabled = enabled;
    save_config(&cfg)
}

fn read_config_from_disk() -> LearningConfig {
    let Some(path) = config_path() else {
        return LearningConfig::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => LearningConfig::default(),
    }
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    if let Ok(guard) = TEST_HOME_DIR.read() {
        if let Some(ref path) = *guard {
            return Some(path.clone());
        }
    }
    env_home_dir()
}

/// Test-only override for `~/.shuji/` paths. Must be paired with serial test locking
/// when used from async tests (see `learning_test::TestHomeGuard`).
#[doc(hidden)]
pub fn set_test_home_dir(path: Option<PathBuf>) {
    if let Ok(mut guard) = TEST_HOME_DIR.write() {
        *guard = path;
    }
}

fn env_home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_config_persists_global_enabled() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let cfg_path = dir.path().join("learning_config.json");
        set_test_config_path(Some(cfg_path));
        reset_config_cache();

        set_global_enabled(true).unwrap();
        assert!(load_config().global_enabled);

        set_global_enabled(false).unwrap();
        assert!(!load_config().global_enabled);

        set_test_config_path(None);
        reset_config_cache();
        Ok(())
    }
}
