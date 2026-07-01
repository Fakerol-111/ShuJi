//! Lifecycle checks for the tool-iteration loop.
//!
//! Extracted from `AgentController::run()`. Two call sites in the loop:
//!
//! 1. **Top of each iteration** — `lifecycle_top`: cancel / fast_cancel /
//!    force_stop checks, periodic checkpoint, mid-run compaction. The first
//!    three return `Some(RunResult::Stopped)` to short-circuit the loop;
//!    the latter two are side-effect-only.
//!
//! 2. **Suspension point B** (right after the API round-trip returns) —
//!    `lifecycle_suspension_b`: cancel / fast_cancel only. No interrupt
//!    snapshot is taken (the API call already completed), no force_stop,
//!    no checkpoint. The stopped text uses `"interrupted"` / `last_text`
//!    rather than `INTERRUPT_RESPONSE` to distinguish the two paths.
//!
//! Behavior is preserved bit-for-bit in this migration: same flags, same
//! strings, same snapshot semantics.

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::api::session::Session;
use crate::config::RuntimeConfig;

use super::RunResult;

impl super::AgentController {
    /// Top-of-loop lifecycle: cancel / fast_cancel / force_stop, then
    /// periodic checkpoint, then mid-run compaction.
    ///
    /// Returns `Some(RunResult)` if the loop should return immediately;
    /// `None` to continue with this iteration's `session.step()` call.
    ///
    /// `last_text` is the accumulated assistant text so far (used in the
    /// stopped message). `iter` is the current 0-based iteration index
    /// (used by the compaction interval check).
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn lifecycle_top(
        &mut self,
        session: &mut Session,
        cancel: &AtomicBool,
        fast_cancel: Option<&AtomicBool>,
        force_stop: Option<&AtomicBool>,
        config: &RuntimeConfig,
        last_text: &str,
        iter: usize,
    ) -> Option<RunResult> {
        // ── Cancel ────────────────────────────────────────────────────────
        // Save a snapshot for potential resume, then surface the interrupt
        // message to 皇帝.
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            self.interrupt(session).await;
            return Some(RunResult::Stopped(format!(
                "{}{}",
                last_text,
                super::INTERRUPT_RESPONSE
            )));
        }
        // ── Fast cancel ───────────────────────────────────────────────────
        // Same handling as cancel — the distinction is semantic (fast cancel
        // is typically a "user pressed stop and wants out now" signal), but
        // the snapshot + INTERRUPT_RESPONSE behavior is identical.
        if fast_cancel.is_some_and(|f| f.load(std::sync::atomic::Ordering::SeqCst)) {
            self.interrupt(session).await;
            return Some(RunResult::Stopped(format!(
                "{}{}",
                last_text,
                super::INTERRUPT_RESPONSE
            )));
        }
        // ── Force stop ────────────────────────────────────────────────────
        // Different from cancel: no snapshot is taken (force_stop is set by
        // the agent itself, e.g. 工部尚书 batch-plan transitions, not by 皇帝),
        // and the stopped text is just `last_text` (or "stopped" if empty).
        if force_stop.is_some_and(|f| f.load(std::sync::atomic::Ordering::SeqCst)) {
            let result = if last_text.is_empty() {
                "stopped".to_string()
            } else {
                last_text.to_string()
            };
            return Some(RunResult::Stopped(result));
        }

        // ── Periodic checkpoint ───────────────────────────────────────────
        // Fires when `config.checkpoint.interval_secs` has elapsed since the
        // last checkpoint. The handler persists the snapshot to disk; the
        // running session is NOT modified.
        if let Some(ref handler) = self.checkpoint_fn {
            if config.checkpoint.interval_secs > 0
                && self.last_checkpoint.elapsed()
                    >= Duration::from_secs(config.checkpoint.interval_secs)
            {
                let snap = session.snapshot();
                handler(snap).await;
                self.last_checkpoint = Instant::now();
            }
        }

        // ── Mid-run compaction ────────────────────────────────────────────
        // Persists a compressed context to disk. Does NOT restore the
        // session — the compressed version is loaded on the next execute()
        // call. This avoids disrupting the running conversation while still
        // reaping the token savings next turn.
        if let Some(ref handler) = self.compact_handler {
            // Suppress the `manual_is_multiple_of` suggestion — the original
            // code used `% == 0` and we keep that form to minimize diff;
            // `is_multiple_of` is semantically identical (Rust 1.87+).
            #[allow(clippy::manual_is_multiple_of)]
            let due = self.compact_iter_interval > 0
                && iter > 0
                && iter % self.compact_iter_interval as usize == 0
                && config.context_compaction.mid_run_compact;
            if due {
                let snap = session.snapshot();
                handler(snap.messages).await;
            }
        }

        None
    }

    /// Suspension point B: the API round-trip just returned. Check cancel /
    /// fast_cancel only — the API call already completed so there's nothing
    /// to interrupt, and force_stop is irrelevant at this point.
    ///
    /// The stopped text uses `"interrupted"` (or `last_text` if non-empty),
    /// NOT `INTERRUPT_RESPONSE`, so the caller can distinguish "interrupted
    /// mid-API-call" from "interrupted between iterations".
    pub(super) fn lifecycle_suspension_b(
        cancel: &AtomicBool,
        fast_cancel: Option<&AtomicBool>,
        last_text: &str,
    ) -> Option<RunResult> {
        let stopped = |last_text: &str| {
            let result = if last_text.is_empty() {
                "interrupted".to_string()
            } else {
                last_text.to_string()
            };
            RunResult::Stopped(result)
        };
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Some(stopped(last_text));
        }
        if fast_cancel.is_some_and(|f| f.load(std::sync::atomic::Ordering::SeqCst)) {
            return Some(stopped(last_text));
        }
        None
    }
}
