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

use std::sync::{Mutex, MutexGuard};

/// 锁定 Mutex，若中毒则恢复内部值（PoisonError::into_inner）。
///
/// 适用于"中毒后内部状态仍可用"或"会重新加载"的场景，如配置缓存。
/// 不适用于"中毒意味着数据损坏不可用"的严格场景——用 [`lock_or_error`]。
pub fn lock_or_recover<T>(m: &Mutex<T>) -> anyhow::Result<MutexGuard<'_, T>> {
    m.lock().or_else(|e| {
        // 锁中毒通常因为持锁线程 panic，内部数据可能不完整。
        // 对缓存类数据，重新加载即可恢复，故返回内部 guard。
        log_console!("[util] Mutex poisoned, recovering inner value: {}", e);
        Ok(e.into_inner())
    })
}

/// 锁定 Mutex，若中毒则返回 anyhow::Error（不恢复）。
///
/// 适用于严格失败语义场景。调用方需决定如何处理错误。
pub fn lock_or_error<T>(m: &Mutex<T>) -> anyhow::Result<MutexGuard<'_, T>> {
    m.lock()
        .map_err(|e| anyhow::anyhow!("mutex poisoned: {}", e))
}
