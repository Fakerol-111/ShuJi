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
        let is_error = serde_json::from_str::<serde_json::Value>(result)
            .ok()
            .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
            .map(|ok| !ok)
            .unwrap_or_else(|| {
                result.contains("failed")
                    || result.contains("error")
                    || result.contains("unknown tool")
            });

        let (err_count, total_errors) = if is_error {
            let prev = self.tool_error_map.get(tool_name).copied().unwrap_or(0);
            let err_count = prev + 1;
            let total_before: u32 = self.tool_error_map.values().sum();
            self.tool_error_map.insert(tool_name.to_string(), err_count);
            (err_count, total_before + 1)
        } else {
            self.tool_error_map.clear();
            (0, 0)
        };

        AfterToolObs {
            is_error,
            err_count,
            total_errors,
            delete_cycle_count,
            read_without_write: self.read_without_write,
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
        if notes.is_empty() {
            None
        } else {
            Some(format!("\n\n[progress] {}", notes.join(", ")))
        }
    }

    /// Snapshot of (tool → error count) for building the force-stop message.
    pub(super) fn error_breakdown(&self) -> Vec<(String, u32)> {
        self.tool_error_map
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
}
