//! Watchdog state for the tool-iteration loop.
//!
//! Extracted from `AgentController::run()` to reduce local-variable pressure
//! inside the main loop. First-pass migration: same thresholds, same strings,
//! same behavior. The struct owns every watchdog-related counter; the loop
//! drives it via `observe_before_tool` / `observe_after_result` /
//! `should_stop_on_error` / `max_iter_reason`.

use std::collections::HashMap;

use crate::config::WatchdogConfig;

/// Snapshot of observations made before a tool is executed.
///
/// `key_arg` is the path/command/id the tool operates on; the caller needs it
/// for delete-create cycle detection (which runs *after* the tool returns).
pub(super) struct BeforeToolObs {
    /// Updated same-tool repetition count (>=1 means same as previous call).
    pub same_tool_count: u32,
}

/// Snapshot of observations made after a tool returns.
///
/// The main loop consumes these to decide playbook injection and force-stop.
pub(super) struct AfterToolObs {
    /// Whether the tool result looks like an error.
    pub is_error: bool,
    /// Consecutive error count for *this* tool name.
    pub err_count: u32,
    /// Total error count across all tools (after this observation).
    pub total_errors: u32,
    /// If a delete-create cycle crossed the warning threshold, its count.
    pub delete_cycle_count: Option<u32>,
    /// Updated read-without-write counter (exposed for playbook-injection checks).
    pub read_without_write: u32,
    /// Dedicated run_tests failure counter that only resets on run_tests success.
    /// Unlike err_count (which clears on ANY tool success), this counter
    /// persists across intervening read_file / search_text calls so the
    /// watchdog can detect true test stalemate patterns.
    pub run_tests_fail_count: u32,
    /// ── P2: Fingerprint tracking ────────────────────────────────────
    /// Number of times the same failure fingerprint has been seen.
    pub fingerprint_repeat_count: u32,
}

/// Why the watchdog requested a force-stop on consecutive errors.
pub(super) enum StopReason {
    /// Same tool failed `max_consecutive_errors` times in a row.
    PerTool { tool: String, err_count: u32 },
    /// Total errors across tools reached `1.5 * max_consecutive_errors`.
    Total { total_errors: u32 },
}

/// Mutable watchdog state carried across iterations of the tool loop.
///
/// All thresholds live in `WatchdogConfig` and are passed in per call; this
/// struct only stores observed counts. That keeps the struct trivially
/// constructible and the threshold wiring explicit at each call site.
pub(super) struct WatchdogState {
    last_tool_name: String,
    last_tool_args: String,
    same_tool_count: u32,
    write_count: u32,
    read_without_write: u32,
    delete_seen: HashMap<String, u32>,
    delete_create_cycles: HashMap<String, u32>,
    tool_error_map: HashMap<String, u32>,
    /// Dedicated counter for run_tests failures. Only increments when
    /// run_tests returns an error, only resets when run_tests succeeds.
    /// This survives intervening successful read_file/search_text calls
    /// that would otherwise clear `tool_error_map`.
    run_tests_fail_count: u32,
    /// ── P0.2: Consecutive invalid tool call counter ──────────────────────
    /// Tracks consecutive `InvalidToolCalls` episodes. Resets on any
    /// successful tool call, so only truly consecutive invalids trigger
    /// the force-stop (3 consecutive → `RunResult::Stopped`).
    consecutive_invalid_tool_calls: u32,
    /// ── P2: Fingerprint tracking ────────────────────────────────────
    /// Map of failure_fingerprint → consecutive count.
    /// Used to detect repeated same-root-cause failures across different
    /// tool calls (e.g., same E0283 appears in both run_tests and check_compile).
    /// Resets when any tool succeeds.
    fingerprint_counts: HashMap<String, u32>,
    /// The most recent failure fingerprint (for progress_note).
    last_fingerprint: String,
    /// Consecutive count for the last fingerprint.
    last_fingerprint_count: u32,
}

impl Default for WatchdogState {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchdogState {
    pub(super) fn new() -> Self {
        Self {
            last_tool_name: String::new(),
            last_tool_args: String::new(),
            same_tool_count: 0,
            write_count: 0,
            read_without_write: 0,
            delete_seen: HashMap::new(),
            delete_create_cycles: HashMap::new(),
            tool_error_map: HashMap::new(),
            run_tests_fail_count: 0,
            consecutive_invalid_tool_calls: 0,
            fingerprint_counts: HashMap::new(),
            last_fingerprint: String::new(),
            last_fingerprint_count: 0,
        }
    }

    /// Update same-tool repetition tracking. Call this before executing a tool.
    ///
    /// `key_arg` resolution matches the original inline logic:
    /// - file/command tools → "path" or "command"
    /// - document tools → "id"
    pub(super) fn observe_before_tool(
        &mut self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> BeforeToolObs {
        let key_arg = args
            .get("path")
            .or_else(|| args.get("command"))
            .or_else(|| args.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if tool_name == self.last_tool_name && key_arg == self.last_tool_args {
            self.same_tool_count += 1;
        } else {
            self.same_tool_count = 0;
            self.last_tool_name = tool_name.to_string();
            self.last_tool_args = key_arg.to_string();
        }
        BeforeToolObs {
            same_tool_count: self.same_tool_count,
        }
    }

    /// Update all post-execution counters: write/read tracking, delete-create
    /// cycle detection, error map. Call this after the tool returns.
    ///
    /// `is_write` / `is_read` classification is the caller's responsibility
    /// (it matches the original `matches!` arms in `run()`).
    pub(super) fn observe_after_result(
        &mut self,
        tool_name: &str,
        args: &serde_json::Value,
        result: &str,
        is_write: bool,
        is_read: bool,
        cfg: &WatchdogConfig,
    ) -> AfterToolObs {
        // ── Write/read tracking ────────────────────────────────────────────
        if is_write {
            self.write_count += 1;
            self.read_without_write = 0;
        } else if is_read {
            self.read_without_write += 1;
            if self.read_without_write == cfg.read_without_write_warning {
                log_console!(
                    "[control] WATCHDOG: {} reads without any write",
                    self.read_without_write
                );
            }
        }

        // ── Delete-create cycle detection ──────────────────────────────────
        // Track when delete_file(path) → create_file(path) repeats on the
        // same path. delete_seen is bumped on delete; on a subsequent
        // create_file for the same path, delete_create_cycles is bumped and
        // (if it crosses the threshold) reported back to the caller.
        let key_path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if tool_name == "delete_file" && !key_path.is_empty() {
            *self.delete_seen.entry(key_path.to_string()).or_insert(0) += 1;
        }
        let mut delete_cycle_count: Option<u32> = None;
        if tool_name == "create_file" && !key_path.is_empty() {
            if let Some(del_count) = self.delete_seen.get(key_path) {
                if *del_count > 0 {
                    let cycle_count = self
                        .delete_create_cycles
                        .entry(key_path.to_string())
                        .or_insert(0);
                    *cycle_count += 1;
                    if *cycle_count >= cfg.delete_create_warning_count {
                        delete_cycle_count = Some(*cycle_count);
                        log_console!(
                            "[control] WATCHDOG: delete-create cycle detected ({}x) on {}",
                            cycle_count,
                            key_path
                        );
                    }
                }
            }
        }

        // ── Error tracking ────────────────────────────────────────────────
        // An error is either an explicit `ok: false` in the result JSON, or
        // a fallback heuristic on the result string (matches original logic).
        let parsed_result = serde_json::from_str::<serde_json::Value>(result).ok();
        let is_error = parsed_result
            .as_ref()
            .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
            .map(|ok| !ok)
            .unwrap_or_else(|| {
                result.contains("failed")
                    || result.contains("error")
                    || result.contains("unknown tool")
            });

        // Extract error_code for environment error detection
        let error_code = parsed_result
            .as_ref()
            .and_then(|v| v.get("error_code").and_then(|e| e.as_str()))
            .unwrap_or("");
        let is_environment_error = error_code == "environment_error";

        // ── P2: Extract failure_fingerprint from result ─────────────────
        let fingerprint = parsed_result
            .as_ref()
            .and_then(|v| v.get("failure_fingerprint").and_then(|f| f.as_str()))
            .unwrap_or("")
            .to_string();
        let fingerprint_repeat = if !fingerprint.is_empty() && is_error && !is_environment_error {
            let prev = self
                .fingerprint_counts
                .get(&fingerprint)
                .copied()
                .unwrap_or(0);
            let count = prev + 1;
            self.fingerprint_counts.insert(fingerprint.clone(), count);
            self.last_fingerprint = fingerprint.clone();
            self.last_fingerprint_count = count;
            count
        } else {
            self.last_fingerprint_count
        };

        // ── Dedicated run_tests failure counter ───────────────────────────
        // This counter only resets when run_tests itself succeeds, NOT when
        // other tools succeed. This prevents the agent from indefinitely
        // alternating run_tests(fail) → read_file(ok) → run_tests(fail)
        // without triggering the stalemate detection.
        // Environment errors (permission/lock/IO) do NOT increment the counter
        // because they are not code issues — the agent should not try to fix code.
        if tool_name == "run_tests" {
            if is_error && !is_environment_error {
                self.run_tests_fail_count += 1;
            } else if !is_error {
                self.run_tests_fail_count = 0;
            }
        }

        let (err_count, total_errors) = if is_error && !is_environment_error {
            let prev = self.tool_error_map.get(tool_name).copied().unwrap_or(0);
            let err_count = prev + 1;
            let total_before: u32 = self.tool_error_map.values().sum();
            self.tool_error_map.insert(tool_name.to_string(), err_count);
            (err_count, total_before + 1)
        } else if !is_error {
            // Any successful tool call resets both error map and invalid-tool-call counter.
            // Also clears fingerprint tracking — a success breaks the failure cycle.
            self.tool_error_map.clear();
            self.consecutive_invalid_tool_calls = 0;
            self.fingerprint_counts.clear();
            self.last_fingerprint_count = 0;
            (0, 0)
        } else {
            // Environment error: transparent to the error map — don't increment,
            // don't clear. The agent should not be penalized for environment issues.
            // Also don't track fingerprints for environment errors.
            (0, 0)
        };

        AfterToolObs {
            is_error,
            err_count,
            total_errors,
            delete_cycle_count,
            read_without_write: self.read_without_write,
            run_tests_fail_count: self.run_tests_fail_count,
            fingerprint_repeat_count: fingerprint_repeat,
        }
    }

    /// Decide whether the latest error observation should force-stop the run.
    ///
    /// Mirrors the original P0-2 rule: stop if either the per-tool error
    /// count hits `max_consecutive_errors`, or the total error count hits
    /// `1.5 * max_consecutive_errors`.
    pub(super) fn should_stop_on_error(
        &self,
        err_count: u32,
        total_errors: u32,
        tool_name: &str,
        max_consecutive: u32,
    ) -> Option<StopReason> {
        if err_count >= max_consecutive {
            Some(StopReason::PerTool {
                tool: tool_name.to_string(),
                err_count,
            })
        } else if total_errors >= (max_consecutive as f32 * 1.5) as u32 {
            Some(StopReason::Total { total_errors })
        } else {
            None
        }
    }

    /// Build the human-readable reason string shown when max iterations is
    /// reached. Same four branches as the original inline `if/else if` chain.
    pub(super) fn max_iter_reason(&self, max_iter: usize) -> String {
        if self.write_count == 0 && self.same_tool_count >= 3 {
            format!(
                "Reached max iterations ({}), repeated tool {} times with no writes",
                max_iter,
                self.same_tool_count + 1
            )
        } else if self.read_without_write >= 8 && self.write_count == 0 {
            format!(
                "Reached max iterations ({}), read {} times with no writes",
                max_iter,
                self.read_without_write + 1
            )
        } else if self.write_count > 0 {
            format!(
                "Reached max iterations ({}), wrote {} files, read {} times",
                max_iter, self.write_count, self.read_without_write
            )
        } else {
            format!(
                "Reached max iterations ({}), no special anomalies",
                max_iter
            )
        }
    }

    /// Construct the `[progress] ...` note appended to tool content when
    /// watchdog thresholds are crossed. Returns `None` if no note is needed.
    ///
    /// Kept here (not in `observe_after_result`) because it needs the *latest*
    /// `same_tool_count` which `observe_after_result` does not return — the
    /// caller already has it from `observe_before_tool`.
    pub(super) fn progress_note(
        &self,
        same_tool_count: u32,
        cfg: &WatchdogConfig,
    ) -> Option<String> {
        let mut notes = Vec::new();
        if same_tool_count >= cfg.same_tool_warning_count {
            notes.push(format!("repeated tool {}", self.last_tool_name));
        }
        if self.read_without_write >= cfg.read_without_write_warning + 3 && self.write_count == 0 {
            notes.push(format!("{} reads without write", self.read_without_write));
        }
        if self.last_fingerprint_count >= 2 {
            notes.push(format!(
                "same error repeated {} times (fingerprint: {})",
                self.last_fingerprint_count, self.last_fingerprint
            ));
        }
        if notes.is_empty() {
            None
        } else {
            Some(format!("\n\n[progress] {}", notes.join(", ")))
        }
    }

    /// ── P0.2: Track consecutive invalid tool call episodes ──────────
    ///
    /// Called when the loop receives `StepResult::InvalidToolCalls`.
    /// Increments the counter and returns it so the caller can decide
    /// whether to force-stop (>= 3). The counter is reset to 0 by
    /// `observe_after_result` on any successful tool execution.
    pub(super) fn track_invalid_tool_calls(&mut self, _broken_count: usize) -> u32 {
        self.consecutive_invalid_tool_calls += 1;
        self.consecutive_invalid_tool_calls
    }

    /// Get the most recent failure fingerprint string.
    pub(super) fn last_fingerprint(&self) -> &str {
        &self.last_fingerprint
    }

    /// Snapshot of (tool → error count) for building the force-stop message.
    pub(super) fn error_breakdown(&self) -> Vec<(String, u32)> {
        self.tool_error_map
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
}
