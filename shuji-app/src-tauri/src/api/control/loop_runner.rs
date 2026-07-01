//! Main tool-iteration loop for `AgentController`.
//!
//! Extracted from `mod.rs` to keep the controller's struct definition,
//! constructors, setters, and lifecycle methods (interrupt / restart /
//! snapshot) in `mod.rs`, and the loop mechanics here.
//!
//! The loop drives a `Session` through repeated `step()` / `step_stream()`
//! calls, executing tool calls and feeding results back. Watchdog state,
//! route detection, lifecycle checks, tool execution, and max-iter wrap-up
//! are all delegated to sibling modules:
//! - `watchdog::WatchdogState` — repetition / write-read / delete-create / error counters
//! - `routing::detect_route` — route_to parsing (main loop + wrap-up)
//! - `lifecycle::lifecycle_top` / `lifecycle_suspension_b` — cancel / checkpoint / compaction
//! - `tool_exec::*` — parallel read execution, emit helpers, feed-remaining
//! - `wrap_up::wrap_up_max_iterations` — graceful max-iter summary step

use std::sync::atomic::{AtomicBool, Ordering};

use crate::api::client::ToolDefinition;
use crate::api::session::Session;
use crate::config::RuntimeConfig;
use crate::models::dept_step::DeptStepKind;

use super::iterations::max_iterations_for_tools;
use super::routing::{detect_route, route_summary, RouteOutcome};
use super::tool_exec::{
    emit_tool_call, emit_tool_result, execute_read_tools_parallel, feed_cancelled_remaining_tools,
};
use super::watchdog::{StopReason, WatchdogState};
use super::{RunResult, ToolFuture};
// Note: INTERRUPT_RESPONSE is a mod.rs const; reference it as `super::INTERRUPT_RESPONSE`.

impl super::AgentController {
    /// Run the tool-iteration loop.
    ///
    /// 1. Call `session.step()` (one API round-trip)
    /// 2. If tool calls 鈫?execute each via `tool_exec`, feed results back
    /// 3. If text 鈫?return `RunResult::Done`
    /// 4. If `cancel` is set 鈫?`interrupt()` and return `RunResult::Stopped`
    ///
    /// Route detection is output-driven: after executing a tool via `tool_exec`,
    /// the result JSON is checked for `operation == "route_to"`. This keeps the
    /// dispatcher generic 鈥?any tool can signal a control-flow transition via
    /// its output, not via pre-execution name matching.
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &mut self,
        session: &mut Session,
        tool_exec: &(dyn for<'a> Fn(&'a str, &'a serde_json::Value) -> ToolFuture + Sync),
        cancel: &AtomicBool,
        tools: &[ToolDefinition],
        force_stop: Option<&AtomicBool>,
        config: &RuntimeConfig,
        fast_cancel: Option<&AtomicBool>,
    ) -> anyhow::Result<RunResult> {
        let max_iter = max_iterations_for_tools(tools, config, session.role());
        let mut last_text = String::new();

        // Watchdog state — all repetition / write-read / delete-create /
        // error counters live in one struct to keep the loop body readable.
        // Thresholds are still read from `config.watchdog` at each call site
        // (no behavior change in this migration).
        let mut wd = WatchdogState::new();

        for iter in 0..max_iter {
            // 鈹€鈹€ Emit iteration step 鈹€鈹€
            if let Some(ref emit) = self.step_emit {
                emit(DeptStepKind::Iteration {
                    n: (iter + 1) as u32,
                });
            }

            // ── Lifecycle: cancel / fast_cancel / force_stop / checkpoint / compaction ──
            // Single call replaces the five inline checks that used to live at
            // the top of each iteration. Returns Some(RunResult) to short-circuit
            // the loop; None to continue with this iteration's API call.
            if let Some(stopped) = self
                .lifecycle_top(
                    session,
                    cancel,
                    fast_cancel,
                    force_stop,
                    config,
                    &last_text,
                    iter,
                )
                .await
            {
                return Ok(stopped);
            }
            log_console!("[control] tool-call iter={}/{}", iter + 1, max_iter);

            let step_emit = &self.step_emit;
            let step_result = if config.api.streaming.enabled {
                session
                    .step_stream(|chunk| {
                        if let Some(emit) = step_emit {
                            match chunk {
                                crate::api::stream::AgentStreamChunk::TextDelta(delta) => {
                                    emit(DeptStepKind::TextDelta { delta });
                                }
                                crate::api::stream::AgentStreamChunk::ReasoningDelta(delta) => {
                                    emit(DeptStepKind::ReasoningDelta { delta });
                                }
                            }
                        }
                    })
                    .await?
            } else {
                session.step().await?
            };

            // ── Suspension point B: API just returned, don't process if cancelled ──
            // No interrupt snapshot (the API call already completed), no
            // force_stop, no checkpoint. Stopped text uses "interrupted" /
            // last_text (not INTERRUPT_RESPONSE) to distinguish from
            // between-iteration interrupts.
            if let Some(stopped) = Self::lifecycle_suspension_b(cancel, fast_cancel, &last_text) {
                return Ok(stopped);
            }
            match step_result {
                crate::api::session::StepResult::Text(text) => {
                    if let Some(ref emit) = self.step_emit {
                        emit(DeptStepKind::Text {
                            content: text.clone(),
                        });
                    }
                    let combined = if last_text.is_empty() {
                        text
                    } else {
                        format!("{}{}", last_text, text)
                    };
                    last_text.clear();
                    return Ok(RunResult::Done(combined));
                }

                crate::api::session::StepResult::ToolCalls { calls, text } => {
                    // Accumulate text from assistant messages that also carried tool_calls
                    if !text.is_empty() {
                        last_text.push_str(&text);
                    }

                    // 鈹€鈹€ Emit thinking step 鈹€鈹€
                    if !text.is_empty() {
                        if let Some(ref emit) = self.step_emit {
                            emit(DeptStepKind::Thinking {
                                content: text.clone(),
                            });
                        }
                    }

                    // ── P2-1: Parallel read-only execution ─────────────────
                    // All read tools in this batch run concurrently via join_all.
                    // Writes are NOT here — they run serially below to avoid
                    // file contention. Results are keyed by call index.
                    let mut read_results = execute_read_tools_parallel(&calls, tool_exec).await;

                    for (idx, tc) in calls.iter().enumerate() {
                        // ── Emit tool call step ──────────────────────────
                        emit_tool_call(self.step_emit.as_ref(), tc);

                        // ── Fast cancel check before each tool ────────────
                        if fast_cancel.is_some_and(|f| f.load(Ordering::SeqCst)) {
                            self.interrupt(session).await;
                            // Feed remaining tool results to keep message state consistent.
                            feed_cancelled_remaining_tools(
                                session,
                                &calls,
                                idx,
                                "Cancelled: fast interrupt signal received",
                            );
                            let result = format!("{}{}", last_text, super::INTERRUPT_RESPONSE);
                            return Ok(RunResult::Stopped(result));
                        }

                        // ── Same-tool watchdog ────────────────────────────────
                        // Observe tool before execution; updates same_tool_count
                        // inside WatchdogState. The "repeated N times" log fires
                        // exactly once when the count first crosses the warning
                        // threshold (kept inline to preserve original behavior).
                        let before = wd.observe_before_tool(&tc.name, &tc.args);
                        let same_tool_count = before.same_tool_count;
                        if same_tool_count == config.watchdog.same_tool_warning_count {
                            log_console!(
                                "[control] WATCHDOG: {} repeated {} times",
                                tc.name,
                                same_tool_count
                            );
                        }

                        // 鈹€鈹€ Execute tool (unified, no special-case intercept) 鈹€鈹€
                        // Use pre-computed result for reads (P2-1 parallel exec),
                        // execute writes serially to avoid file contention.
                        let result = if let Some(pre) = read_results.remove(&idx) {
                            pre
                        } else {
                            tool_exec(&tc.name, &tc.args).await
                        };
                        self.collect_document_from_tool(&tc.name, &result);

                        // ── Emit tool result step ────────────────────────
                        emit_tool_result(self.step_emit.as_ref(), &tc.name, &result);

                        // ── Watchdog intervention hints ──
                        // Playbook-backed recovery hints are appended to tool_content
                        // after write/read tracking (see below).
                        let is_read = matches!(
                            tc.name.as_str(),
                            "read_file"
                                | "list_dir"
                                | "list_dir_tree"
                                | "find_document"
                                | "read_document"
                                | "search_text"
                        );

                        // ── Route detection (output-driven) ───────────────────
                        // Check the tool output for operation=="route_to" instead of
                        // matching tool names before execution. This keeps the dispatcher
                        // generic — any tool can signal a control-flow transition.
                        // `detect_route` returns a typed outcome; the loop decides the
                        // side effects (feed error / feed dummy / return Routed).
                        let my_role = session.role();
                        match detect_route(&result, &tc.args, Some(my_role)) {
                            RouteOutcome::SelfRoute { my_role } => {
                                let msg = format!(
                                    "Routing to self ({}) is forbidden. Please route to another department, or output results directly instead of routing.",
                                    my_role
                                );
                                session.feed_tool_result(&tc.id, &tc.name, &msg);
                                continue;
                            }
                            RouteOutcome::UnknownTarget { to_name } => {
                                let msg = format!("Unknown target department: {}", to_name);
                                session.feed_tool_result(&tc.id, &tc.name, &msg);
                                continue;
                            }
                            RouteOutcome::Route(route) => {
                                // Feed route result for this call.
                                session.feed_tool_result(&tc.id, &tc.name, &result);

                                // Feed dummy results for remaining calls in this batch
                                // so the assistant message's tool_calls are balanced with
                                // tool_results — otherwise the next API request returns 400.
                                feed_cancelled_remaining_tools(
                                    session,
                                    &calls,
                                    idx + 1,
                                    "Cancelled: this batch interrupted due to routing to another department",
                                );

                                let summary = route_summary(&route);
                                let out = if last_text.is_empty() {
                                    summary
                                } else {
                                    format!("{}{}", last_text, summary)
                                };
                                return Ok(RunResult::Routed { text: out, route });
                            }
                            RouteOutcome::NotRoute => {
                                // Fall through to write/read tracking below.
                            }
                        }

                        // ── Write/read classification ───────────────────────
                        // is_write feeds WatchdogState::observe_after_result for
                        // write/read tracking. is_read was already computed above
                        // for route detection.
                        let is_write = matches!(
                            tc.name.as_str(),
                            "create_file"
                                | "modify_file"
                                | "append_file"
                                | "delete_file"
                                | "rename_file"
                        );

                        // ── Watchdog post-result observation ────────────────
                        // One call updates: write/read counters, delete-create
                        // cycle detection, and the error map. Returns everything
                        // the loop needs for playbook injection + force-stop.
                        // (The read-without-write warning log fires inside,
                        // preserving the original "exactly once on threshold
                        // crossing" behavior.)
                        let after = wd.observe_after_result(
                            &tc.name,
                            &tc.args,
                            &result,
                            is_write,
                            is_read,
                            &config.watchdog,
                        );
                        let is_error = after.is_error;
                        let err_count = after.err_count;
                        let total_errors = after.total_errors;
                        let delete_cycle_count = after.delete_cycle_count;
                        let read_without_write = after.read_without_write;

                        // ── Progress note (uses same_tool_count from observe_before_tool) ──
                        let progress_note = wd
                            .progress_note(same_tool_count, &config.watchdog)
                            .unwrap_or_default();

                        let mut tool_content = result.clone();
                        if !progress_note.is_empty() {
                            tool_content.push_str(&progress_note);
                        }

                        use crate::playbook::{
                            append_watchdog_intervention, WatchdogEvent, WatchdogHintContext,
                        };

                        if same_tool_count >= config.watchdog.same_tool_warning_count {
                            append_watchdog_intervention(
                                &mut tool_content,
                                WatchdogEvent::RepeatedTool,
                                &WatchdogHintContext::new(same_tool_count).tool(&tc.name),
                            );
                            log_console!(
                                "[control] WATCHDOG: repeated-tool playbook injected for {}",
                                tc.name
                            );
                        }
                        if is_read
                            && read_without_write >= config.watchdog.read_without_write_warning
                        {
                            append_watchdog_intervention(
                                &mut tool_content,
                                WatchdogEvent::ReadWithoutWrite,
                                &WatchdogHintContext::new(read_without_write),
                            );
                            log_console!(
                                "[control] WATCHDOG: read-without-write playbook injected ({} reads)",
                                read_without_write
                            );
                        }
                        if let Some(cycle_count) = delete_cycle_count {
                            // key_path was tracked inside observe_after_result; re-extract
                            // here for the playbook hint (path is needed in the message).
                            let key_path =
                                tc.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                            append_watchdog_intervention(
                                &mut tool_content,
                                WatchdogEvent::DeleteCreateCycle,
                                &WatchdogHintContext::new(cycle_count).path(key_path),
                            );
                            log_console!(
                                "[control] WATCHDOG: delete-create playbook injected for {}",
                                key_path
                            );
                        }

                        if is_error {
                            let first_line = result.lines().next().unwrap_or(&result);
                            let preview = if first_line.len() > 120 {
                                let end = first_line.floor_char_boundary(120);
                                format!("{}...", &first_line[..end])
                            } else {
                                first_line.to_string()
                            };
                            log_console!(
                                "[control] tool error (#{} total, #{}/{} for {})",
                                total_errors,
                                err_count,
                                config.watchdog.max_consecutive_errors,
                                tc.name
                            );
                            log_console!("  {}", preview);

                            // ── Consecutive tool errors → playbook ──
                            if total_errors >= 3 {
                                let error_tools: Vec<String> = wd
                                    .error_breakdown()
                                    .iter()
                                    .map(|(k, v)| format!("{}x {}", v, k))
                                    .collect();
                                append_watchdog_intervention(
                                    &mut tool_content,
                                    WatchdogEvent::ConsecutiveToolErrors,
                                    &WatchdogHintContext::new(total_errors)
                                        .detail(&error_tools.join(", ")),
                                );
                                log_console!(
                                    "[control] consecutive-tool-errors playbook injected (total_errors={})",
                                    total_errors
                                );
                            }

                            // ── Test stalemate detection ──
                            if tc.name == "run_tests"
                                && err_count >= config.watchdog.test_stalemate_threshold
                            {
                                append_watchdog_intervention(
                                    &mut tool_content,
                                    WatchdogEvent::TestRedLoop,
                                    &WatchdogHintContext::new(err_count),
                                );
                                log_console!(
                                    "[control] WATCHDOG: test-red playbook injected (consecutive={})",
                                    err_count
                                );
                            }

                            // ── P0-2: Force-stop on per-tool or total error threshold ──
                            // WatchdogState::should_stop_on_error decides which limit
                            // was crossed and returns the matching reason variant.
                            // (tool_error_map.clear() on success already happened
                            // inside observe_after_result, preserving the original
                            // "any success resets the map" behavior.)
                            if let Some(stop_reason) = wd.should_stop_on_error(
                                err_count,
                                total_errors,
                                &tc.name,
                                config.watchdog.max_consecutive_errors,
                            ) {
                                let reason = match stop_reason {
                                    StopReason::PerTool { tool, err_count } => {
                                        format!("{} failed {} consecutive times", tool, err_count)
                                    }
                                    StopReason::Total { total_errors } => {
                                        format!("total {} errors across tools", total_errors)
                                    }
                                };
                                let error_details: Vec<String> = wd
                                    .error_breakdown()
                                    .iter()
                                    .map(|(k, v)| format!("{}: {} errors", k, v))
                                    .collect();
                                last_text = format!(
                                    "Tool errors exceeded limit ({}). Terminating. Details: {}",
                                    reason,
                                    error_details.join("; "),
                                );
                                session.feed_tool_result(&tc.id, &tc.name, &tool_content);
                                feed_cancelled_remaining_tools(
                                    session,
                                    &calls,
                                    idx + 1,
                                    "Cancelled: tool consecutive error, terminating",
                                );
                                return Ok(RunResult::Stopped(last_text));
                            }
                        }
                        session.feed_tool_result(&tc.id, &tc.name, &tool_content);
                    }
                }
            }
        }

        // ── Max iterations reached: graceful wrap-up ───────────────────
        // Delegate to `wrap_up_max_iterations`: injects a summary request,
        // does one extra API step, and returns Routed (if the model called
        // route_to) or Done (with the work summary).
        return self
            .wrap_up_max_iterations(session, tool_exec, &wd, &last_text, max_iter)
            .await;
    }
}
