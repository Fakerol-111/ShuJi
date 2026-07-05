use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::SystemTime;

type ProjectReadCache = HashMap<PathBuf, (SystemTime, String)>;
type ReadCache = HashMap<PathBuf, ProjectReadCache>;

// ── P2-2: In-memory read cache (per-project bucketed) ────
/// Per-project read cache: working_dir → (path → (mtime, cached result)).
/// Keying by working_dir isolates projects so switching projects doesn't
/// require clearing the entire cache. Multi-project concurrent use is safe.
static READ_CACHE: LazyLock<Mutex<ReadCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn bucket<'a>(cache: &'a mut ReadCache, working_dir: &Path) -> &'a mut ProjectReadCache {
    cache.entry(working_dir.to_path_buf()).or_default()
}

/// Look up a cached read result. Returns `Some(raw_content)` if the file's
/// mtime hasn't changed since the cache entry was created.
///
/// **Important**: The returned value is the **raw file content**, NOT a
/// JSON-wrapped ToolOutput. Callers must still format the result through
/// `ToolOutput::read_file(...)` or similar before returning it as a tool
/// result. Returning the raw content directly would break `is_error()`
/// detection (the JSON parser would fail and fall back to keyword matching,
/// causing false positives if the content contains words like "error").
pub fn cache_lookup(working_dir: &Path, path: &Path) -> Option<String> {
    let mut cache = READ_CACHE.lock().ok()?;
    let proj = bucket(&mut cache, working_dir);
    if let Some((cached_mtime, cached_result)) = proj.get(path) {
        if let Ok(current_mtime) = std::fs::metadata(path).and_then(|m| m.modified()) {
            if current_mtime == *cached_mtime {
                return Some(cached_result.clone());
            }
        }
    }
    None
}

/// Insert a read result into the cache.
pub fn cache_insert(working_dir: &Path, path: PathBuf, mtime: SystemTime, result: String) {
    if let Ok(mut cache) = READ_CACHE.lock() {
        bucket(&mut cache, working_dir).insert(path, (mtime, result));
    }
}

/// Invalidate cache entries whose key starts with or equals `path`.
/// Call after any write/delete/rename/patch operation.
pub fn cache_invalidate(working_dir: &Path, path: &Path) {
    if let Ok(mut cache) = READ_CACHE.lock() {
        if let Some(proj) = cache.get_mut(working_dir) {
            proj.retain(|k, _| !k.starts_with(path));
        }
    }
}

/// Clear the entire read cache (all projects).
/// Call on application shutdown or when a full reset is needed.
pub fn cache_clear_all() {
    if let Ok(mut cache) = READ_CACHE.lock() {
        cache.clear();
    }
}
