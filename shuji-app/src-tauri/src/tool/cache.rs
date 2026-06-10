use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::SystemTime;

// ── P2-2: In-memory read cache ─────────────────────────────
/// Session-level read cache: maps resolved path → (mtime, cached result string).
/// After any write/delete/rename/patch, the affected path(s) are invalidated.
static READ_CACHE: LazyLock<Mutex<HashMap<PathBuf, (SystemTime, String)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Look up a cached read result. Returns `Some(result)` if the file's mtime
/// hasn't changed since the cache entry was created.
pub fn cache_lookup(path: &Path) -> Option<String> {
    let cache = READ_CACHE.lock().ok()?;
    if let Some((cached_mtime, cached_result)) = cache.get(path) {
        if let Ok(current_mtime) = std::fs::metadata(path).and_then(|m| m.modified()) {
            if current_mtime == *cached_mtime {
                return Some(format!(
                    "{}[缓存命中: 内容未变 (cached: true)]",
                    cached_result
                ));
            }
        }
    }
    None
}

/// Insert a read result into the cache.
pub fn cache_insert(path: PathBuf, mtime: SystemTime, result: String) {
    if let Ok(mut cache) = READ_CACHE.lock() {
        cache.insert(path, (mtime, result));
    }
}

/// Invalidate cache entries whose key starts with or equals `path`.
/// Call after any write/delete/rename/patch operation.
pub fn cache_invalidate(path: &Path) {
    if let Ok(mut cache) = READ_CACHE.lock() {
        cache.retain(|k, _| !k.starts_with(path));
    }
}

/// Clear the entire read cache.
/// Call on project switch to prevent stale cache entries from the previous project.
pub fn cache_clear_all() {
    if let Ok(mut cache) = READ_CACHE.lock() {
        cache.clear();
    }
}
