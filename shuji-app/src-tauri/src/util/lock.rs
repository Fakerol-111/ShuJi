//! 锁辅助工具：把 `Mutex::lock().unwrap()` 的 panic 风险转为显式错误处理。
//!
//! 背景：项目中多处 `Mutex::lock().unwrap()` 在锁中毒时会 panic，panic 在 actor
//! 系统中会传导放大。本模块提供 `lock_or_recover` 与 `lock_or_error` 两种策略：
//! - `lock_or_recover`：恢复中毒锁的内部值（假设内部状态仍可用），适合状态缓存类
//!   （如 ESAA 合约缓存，重新加载即可）
//! - `lock_or_error`：把中毒错误转为 `anyhow::Error`，适合需要严格失败语义的场景
//!
//! 用法：
//! ```ignore
//! use crate::util::lock::lock_or_recover;
//! let guard = lock_or_recover(&self.contracts)?;
//! ```

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Lock Mutex, recovering from poison.
pub fn lock_or_recover<T>(m: &Mutex<T>) -> anyhow::Result<MutexGuard<'_, T>> {
    m.lock().or_else(|e| {
        log_console!("[util] Mutex poisoned, recovering inner value: {}", e);
        Ok(e.into_inner())
    })
}

/// Lock Mutex, returning error on poison.
pub fn lock_or_error<T>(m: &Mutex<T>) -> anyhow::Result<MutexGuard<'_, T>> {
    m.lock()
        .map_err(|e| anyhow::anyhow!("mutex poisoned: {}", e))
}

/// Lock RwLock for reading, recovering from poison.
pub fn rwlock_read_or_recover<T>(m: &RwLock<T>) -> anyhow::Result<RwLockReadGuard<'_, T>> {
    m.read().or_else(|e| {
        log_console!(
            "[util] RwLock poisoned (read), recovering inner value: {}",
            e
        );
        Ok(e.into_inner())
    })
}

/// Lock RwLock for reading, returning error on poison.
pub fn rwlock_read_or_error<T>(m: &RwLock<T>) -> anyhow::Result<RwLockReadGuard<'_, T>> {
    m.read()
        .map_err(|e| anyhow::anyhow!("RwLock poisoned (read): {}", e))
}

/// Lock RwLock for writing, recovering from poison.
pub fn rwlock_write_or_recover<T>(m: &RwLock<T>) -> anyhow::Result<RwLockWriteGuard<'_, T>> {
    m.write().or_else(|e| {
        log_console!(
            "[util] RwLock poisoned (write), recovering inner value: {}",
            e
        );
        Ok(e.into_inner())
    })
}

/// Lock RwLock for writing, returning error on poison.
pub fn rwlock_write_or_error<T>(m: &RwLock<T>) -> anyhow::Result<RwLockWriteGuard<'_, T>> {
    m.write()
        .map_err(|e| anyhow::anyhow!("RwLock poisoned (write): {}", e))
}
