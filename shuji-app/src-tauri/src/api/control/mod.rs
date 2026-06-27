use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures::future::join_all;

use crate::api::client::ToolDefinition;
use crate::api::session::{Session, SessionSnapshot};
use crate::config::RuntimeConfig;
use crate::models::dept_step::{DeptStepEntry, DeptStepKind};

// ── Sub-modules ──────────────────────────────────────────────────────────────
mod iterations;
mod types;

// ── Internal imports from sub-modules ───────────────────────────────────────
use iterations::{is_read_tool, max_iterations_for_tools};
use types::route_msg_type_from_str;

// ── Re-export public API (same as original `pub` items in control.rs) ───────
pub use types::{
    role_from_name, CheckpointFn, CompactFn, DeptStepCallback, RouteMsgType, RouteTo, RunResult,
    ToolFuture,
};

const INTERRUPT_RESPONSE: &str = "\n\n[System] Current processing has been interrupted by 皇帝";

/// Control layer for tool-use agents.
///
/// Owns the tool-iteration loop, cancel/interrupt/restart lifecycle,
/// watchdog diagnostics, and anything related to "how" the LLM is
/// driven. The LLM itself is a `Session` — this struct controls it.
pub struct AgentController {
    saved: Option<SessionSnapshot>,
    checkpoint_fn: Option<CheckpointFn>,
    last_checkpoint: Instant,
    compact_handler: Option<CompactFn>,
    compact_iter_interval: u32,
    step_emit: Option<DeptStepCallback>,
    created_doc_ids: Vec<String>,
}

impl Default for AgentController {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentController {
    pub fn new() -> Self {
        Self {
            saved: None,
            checkpoint_fn: None,
            last_checkpoint: Instant::now(),
            compact_handler: None,
            compact_iter_interval: 0,
            step_emit: None,
            created_doc_ids: Vec::new(),
        }
    }

    fn collect_document_from_tool(&mut self, tool_name: &str, result: &str) {
        if !matches!(tool_name, "create_document" | "append_document") {
            return;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(result) else {
            return;
        };
        if v.get("ok").and_then(|o| o.as_bool()) != Some(true) {
            return;
        }
        if let Some(id) = v
            .get("path")
            .and_then(|p| p.as_str())
            .filter(|s| !s.is_empty())
        {
            if !self.created_doc_ids.iter().any(|x| x == id) {
                self.created_doc_ids.push(id.to_string());
            }
        }
    }

    /// Resolve collected document IDs into chat-card metadata.
    pub async fn take_documents(
        &mut self,
        working_dir: &std::path::Path,
    ) -> Vec<crate::models::chat::ChatDocument> {
        let ids = std::mem::take(&mut self.created_doc_ids);
        let mut docs: Vec<crate::models::chat::ChatDocument> = Vec::new();
        for id in ids {
            if let Some(doc) = crate::tool::documents::chat_document_from_id(working_dir, &id).await
            {
                if !docs.iter().any(|d| d.id == doc.id) {
                    docs.push(doc);
                }
            }
        }
        docs
    }

    /// Register a handler for real-time step events.
    /// Called at each iteration, thinking block, tool call, and tool result.
    pub fn set_step_emitter(&mut self, emitter: DeptStepCallback) {
        self.step_emit = Some(emitter);
    }

    /// Register a handler for periodic checkpoint saves.
    /// Called at suspension points when `config.checkpoint.interval_secs` has elapsed.
    pub fn set_checkpoint_handler(&mut self, handler: CheckpointFn) {
        self.checkpoint_fn = Some(handler);
    }

    /// Register a handler for mid-run context compaction.
    /// `interval` controls how many tool-call iterations between compactions.
    /// The handler receives the flat session messages and persists a compressed
    /// version to disk. The running session is NOT modified 鈥?the compressed
    /// context is loaded automatically on the next execute() call.
    /// Helps prevent unbounded context growth during long-running agent sessions.
    pub fn set_compact_handler(&mut self, handler: CompactFn, interval: u32) {
        self.compact_handler = Some(handler);
        self.compact_iter_interval = interval;
    }

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
        let mut tool_error_map: HashMap<String, u32> = HashMap::new();

        // Watchdog trackers
        let mut last_tool_name = String::new();
        let mut last_tool_args = String::new();
        let mut same_tool_count: u32 = 0;
        let mut write_count: u32 = 0;
        let mut read_without_write: u32 = 0;

        // Delete-create cycle tracking
        // Maps path 鈫?cycle count. Incremented when create_file follows delete_file on same path.
        use std::collections::HashMap as HashMapColl;
        let mut delete_seen: HashMapColl<String, u32> = HashMapColl::new();
        let mut delete_create_cycles: HashMapColl<String, u32> = HashMapColl::new();

        for iter in 0..max_iter {
            // 鈹€鈹€ Emit iteration step 鈹€鈹€
            if let Some(ref emit) = self.step_emit {
                emit(DeptStepKind::Iteration {
                    n: (iter + 1) as u32,
                });
            }

            // 鈹€鈹€ Interrupt / force-stop / fast-cancel check 鈹€鈹€
            if cancel.load(Ordering::SeqCst) {
                self.interrupt(session).await;
                let result = format!("{}{}", last_text, INTERRUPT_RESPONSE);
                return Ok(RunResult::Stopped(result));
            }
            if fast_cancel.is_some_and(|f| f.load(Ordering::SeqCst)) {
                self.interrupt(session).await;
                let result = format!("{}{}", last_text, INTERRUPT_RESPONSE);
                return Ok(RunResult::Stopped(result));
            }
            if force_stop.is_some_and(|f| f.load(Ordering::SeqCst)) {
                let result = if last_text.is_empty() {
                    "stopped".to_string()
                } else {
                    last_text
                };
                return Ok(RunResult::Stopped(result));
            }

            // 鈹€鈹€ Periodic checkpoint 鈹€鈹€
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

            // 鈹€鈹€ Mid-run compaction: persist compressed context to disk 鈹€鈹€
            // Does NOT restore the session 鈥?the compressed version is loaded
            // on the next execute() call. This avoids disrupting the running
            // conversation while still reaping the token savings next turn.
            if let Some(ref handler) = self.compact_handler {
                if self.compact_iter_interval > 0
                    && iter > 0
                    && iter % self.compact_iter_interval as usize == 0
                    && config.context_compaction.mid_run_compact
                {
                    let snap = session.snapshot();
                    handler(snap.messages).await;
                }
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

            // 鈹€鈹€ Suspension point B: API just returned, don't process if cancelled 鈹€鈹€
            if cancel.load(Ordering::SeqCst) {
                let result = if last_text.is_empty() {
                    "interrupted".to_string()
                } else {
                    last_text.clone()
                };
                return Ok(RunResult::Stopped(result));
            }
            if fast_cancel.is_some_and(|f| f.load(Ordering::SeqCst)) {
                let result = if last_text.is_empty() {
                    "interrupted".to_string()
                } else {
                    last_text.clone()
                };
                return Ok(RunResult::Stopped(result));
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

                    // 鈹€鈹€ P2-1: Parallel read-only execution 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
                    // Execute all read tools concurrently, store results by index.
                    // Writes still execute serially below to avoid file contention.
                    let mut read_results: HashMap<usize, String> = HashMap::new();
                    {
                        let read_futures: Vec<_> = calls
                            .iter()
                            .enumerate()
                            .filter(|(_, tc)| is_read_tool(&tc.name))
                            .map(|(idx, tc)| {
                                let name = tc.name.clone();
                                let args = tc.args.clone();
                                // tool_exec is Sync, so it's safe to call from multiple futures
                                let fut = tool_exec(&name, &args);
                                async move { (idx, fut.await) }
                            })
                            .collect();
                        for (idx, result) in join_all(read_futures).await {
                            read_results.insert(idx, result);
                        }
                    }

                    for (idx, tc) in calls.iter().enumerate() {
                        // 鈹€鈹€ Emit tool call step 鈹€鈹€
                        if let Some(ref emit) = self.step_emit {
                            emit(DeptStepKind::ToolCall {
                                tool: tc.name.clone(),
                                args: tc.args.clone(),
                            });
                        }

                        // 鈹€鈹€ Fast cancel check before each tool 鈹€鈹€
                        if fast_cancel.is_some_and(|f| f.load(Ordering::SeqCst)) {
                            self.interrupt(session).await;
                            // Feed remaining tool results to keep message state consistent
                            for remaining in &calls[idx..] {
                                session.feed_tool_result(
                                    &remaining.id,
                                    &remaining.name,
                                    "Cancelled: fast interrupt signal received",
                                );
                            }
                            let result = format!("{}{}", last_text, INTERRUPT_RESPONSE);
                            return Ok(RunResult::Stopped(result));
                        }

                        // 鈹€鈹€ Same-tool watchdog 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
                        // Extract key argument for similarity detection:
                        // - file/command tools 鈫?"path" or "command"
                        // - document tools 鈫?"id" (read_document, append_document, etc.)
                        let key_arg = tc
                            .args
                            .get("path")
                            .or_else(|| tc.args.get("command"))
                            .or_else(|| tc.args.get("id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if tc.name == last_tool_name && key_arg == last_tool_args {
                            same_tool_count += 1;
                        } else {
                            same_tool_count = 0;
                            last_tool_name = tc.name.clone();
                            last_tool_args = key_arg.to_string();
                        }

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

                        // 鈹€鈹€ Emit tool result step 鈹€鈹€
                        if let Some(ref emit) = self.step_emit {
                            let is_error = serde_json::from_str::<serde_json::Value>(&result)
                                .ok()
                                .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
                                .map(|ok| !ok)
                                .unwrap_or(false);
                            let summary = result.chars().take(200).collect();
                            emit(DeptStepKind::ToolResult {
                                tool: tc.name.clone(),
                                ok: !is_error,
                                summary,
                            });
                        }

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

                        // 鈹€鈹€ Route detection (output-driven) 鈹€鈹€
                        // Check the tool output for operation=="route_to" instead of
                        // matching tool names before execution. This keeps the dispatcher
                        // generic 鈥?any tool can signal a control-flow transition.
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&result) {
                            if v["operation"].as_str() == Some("route_to") {
                                let to_name = tc.args["to"].as_str().unwrap_or("");
                                let my_role = session.role();

                                // Self-routing check
                                if role_from_name(to_name).is_some_and(|r| r.name() == my_role) {
                                    let msg = format!(
                                        "Routing to self ({}) is forbidden. Please route to another department, or output results directly instead of routing.",
                                        my_role
                                    );
                                    session.feed_tool_result(&tc.id, &tc.name, &msg);
                                    continue;
                                }
                                let target = match role_from_name(to_name) {
                                    Some(r) => r,
                                    None => {
                                        let msg = format!("Unknown target department: {}", to_name);
                                        session.feed_tool_result(&tc.id, &tc.name, &msg);
                                        continue;
                                    }
                                };

                                // Feed route result for this call
                                session.feed_tool_result(&tc.id, &tc.name, &result);

                                // Feed dummy results for remaining calls in this batch
                                // so the assistant message's tool_calls are balanced with
                                // tool_results 鈥?otherwise the next API request returns 400.
                                for remaining in &calls[idx + 1..] {
                                    session.feed_tool_result(
                                        &remaining.id,
                                        &remaining.name,
                                        "Cancelled: this batch interrupted due to routing to another department",
                                    );
                                }

                                let msg_type = route_msg_type_from_str(
                                    tc.args["type"].as_str().unwrap_or("task"),
                                )
                                .unwrap_or(RouteMsgType::Task);
                                let subject_raw =
                                    tc.args["subject"].as_str().unwrap_or("").to_string();
                                let payload = tc
                                    .args
                                    .get("inline")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string());
                                let (task_subject, doc_ids) =
                                    crate::pipeline::artifacts::split_route_task_and_doc_ids(
                                        &subject_raw,
                                        payload.as_deref(),
                                    );
                                let route = RouteTo {
                                    target,
                                    msg_type,
                                    subject: task_subject,
                                    payload,
                                    doc_ids,
                                };
                                let summary = format!(
                                    "Routed to {} ({}): {}{}",
                                    target.name(),
                                    match msg_type {
                                        RouteMsgType::Task => "new task",
                                        RouteMsgType::Replace => "replace",
                                        RouteMsgType::Interrupt => "interrupt",
                                    },
                                    route.subject,
                                    if route.doc_ids.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" [docs: {}]", route.doc_ids.join(", "))
                                    },
                                );
                                let out = if last_text.is_empty() {
                                    summary
                                } else {
                                    format!("{}{}", last_text, summary)
                                };
                                return Ok(RunResult::Routed { text: out, route });
                            }
                        }

                        // 鈹€鈹€ Write/read tracking 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
                        let is_write = matches!(
                            tc.name.as_str(),
                            "create_file"
                                | "modify_file"
                                | "append_file"
                                | "delete_file"
                                | "rename_file"
                        );
                        if is_write {
                            write_count += 1;
                            read_without_write = 0;
                        } else if is_read {
                            read_without_write += 1;
                            if read_without_write == config.watchdog.read_without_write_warning {
                                log_console!(
                                    "[control] WATCHDOG: {} reads without any write",
                                    read_without_write
                                );
                            }
                        }

                        // 鈹€鈹€ Delete-create cycle detection 鈹€鈹€鈹€鈹€鈹€
                        // Track when delete_file(path) 鈫?create_file(path) repeats on the same path.
                        let key_path = tc.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        if tc.name == "delete_file" && !key_path.is_empty() {
                            *delete_seen.entry(key_path.to_string()).or_insert(0) += 1;
                        }
                        let mut delete_cycle_count: Option<u32> = None;
                        if tc.name == "create_file" && !key_path.is_empty() {
                            if let Some(del_count) = delete_seen.get(key_path) {
                                if *del_count > 0 {
                                    let cycle_count = delete_create_cycles
                                        .entry(key_path.to_string())
                                        .or_insert(0);
                                    *cycle_count += 1;
                                    if *cycle_count >= config.watchdog.delete_create_warning_count {
                                        delete_cycle_count = Some(*cycle_count);
                                        log_console!(
                                            "[control] WATCHDOG: delete-create cycle detected ({}x) on {}",
                                            cycle_count, key_path
                                        );
                                    }
                                }
                            }
                        }

                        // 鈹€鈹€ Progress note 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
                        let mut notes = Vec::new();
                        if same_tool_count >= config.watchdog.same_tool_warning_count {
                            notes.push(format!("repeated tool {}", tc.name));
                        }
                        if read_without_write >= config.watchdog.read_without_write_warning + 3
                            && write_count == 0
                        {
                            notes.push(format!("{} reads without write", read_without_write));
                        }
                        let progress_note = if notes.is_empty() {
                            String::new()
                        } else {
                            format!("\n\n[progress] {}", notes.join(", "))
                        };

                        // 鈹€鈹€ Consecutive error tracking 鈹€鈹€鈹€鈹€
                        let is_error = serde_json::from_str::<serde_json::Value>(&result)
                            .ok()
                            .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
                            .map(|ok| !ok)
                            .unwrap_or_else(|| {
                                result.contains("failed")
                                    || result.contains("error")
                                    || result.contains("unknown tool")
                            });

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
                            // Compute counts before mutable borrow of tool_error_map
                            let prev_count = tool_error_map.get(&tc.name).copied().unwrap_or(0);
                            let total_before: u32 = tool_error_map.values().sum();
                            let err_count = prev_count + 1;
                            let total_errors = total_before + 1;
                            tool_error_map.insert(tc.name.clone(), err_count);

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
                                let error_tools: Vec<String> = tool_error_map
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

                            // 鈹€鈹€ P0-2: Force-stop: check both per-tool and total 鈹€鈹€
                            if err_count >= config.watchdog.max_consecutive_errors
                                || total_errors
                                    >= (config.watchdog.max_consecutive_errors as f32 * 1.5) as u32
                            {
                                let reason = if err_count >= config.watchdog.max_consecutive_errors
                                {
                                    format!("{} failed {} consecutive times", tc.name, err_count)
                                } else {
                                    format!("total {} errors across tools", total_errors)
                                };
                                let error_details: Vec<String> = tool_error_map
                                    .iter()
                                    .map(|(k, v)| format!("{}: {} errors", k, v))
                                    .collect();
                                last_text = format!(
                                    "Tool errors exceeded limit ({}). Terminating. Details: {}",
                                    reason,
                                    error_details.join("; "),
                                );
                                session.feed_tool_result(&tc.id, &tc.name, &tool_content);
                                for remaining in &calls[idx + 1..] {
                                    session.feed_tool_result(
                                        &remaining.id,
                                        &remaining.name,
                                        "Cancelled: tool consecutive error, terminating",
                                    );
                                }
                                return Ok(RunResult::Stopped(last_text));
                            }
                        } else {
                            tool_error_map.clear();
                        }

                        session.feed_tool_result(&tc.id, &tc.name, &tool_content);
                    }
                }
            }
        }

        // 鈹€鈹€ Max iterations reached 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
        log_console!("[control] tool-call limit ({}) reached", max_iter);
        let reason = if write_count == 0 && same_tool_count >= 3 {
            format!(
                "Reached max iterations ({}), repeated tool {} times with no writes",
                max_iter,
                same_tool_count + 1
            )
        } else if read_without_write >= 8 && write_count == 0 {
            format!(
                "Reached max iterations ({}), read {} times with no writes",
                max_iter,
                read_without_write + 1
            )
        } else if write_count > 0 {
            format!(
                "Reached max iterations ({}), wrote {} files, read {} times",
                max_iter, write_count, read_without_write
            )
        } else {
            format!(
                "Reached max iterations ({}), no special anomalies",
                max_iter
            )
        };
        // 鈹€鈹€ Graceful wrap-up: inject summary request, do one extra step 鈹€鈹€
        // Instead of silently cutting off, give the LLM a chance to produce
        // a coherent wrap-up and route to its superior for authorization.
        session.set_reasoning(true);
        let wrap_up = format!(
            "\n\n---\n[System] Tool call limit reached ({} calls). {}. \
             \nPlease immediately summarize the work completed and any difficulties encountered. If you need to continue, call route_to to route to 灏氫功浠?explaining the reason and requesting further authorization. \
             \nIf no routing is needed, directly output the work summary.\n---",
            max_iter, reason,
        );
        session.inject(&wrap_up);

        let final_text = match session.step().await? {
            crate::api::session::StepResult::Text(t) => t,
            crate::api::session::StepResult::ToolCalls { calls, text } => {
                let combined = text;
                // Execute first tool call 鈥?check if it's a route_to
                if let Some(tc) = calls.first() {
                    let result = tool_exec(&tc.name, &tc.args).await;
                    self.collect_document_from_tool(&tc.name, &result);
                    // Route detection (same logic as main loop)
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&result) {
                        if v["operation"].as_str() == Some("route_to") {
                            let to_name = tc.args["to"].as_str().unwrap_or("");
                            if let Some(target) = role_from_name(to_name) {
                                let msg_type = route_msg_type_from_str(
                                    tc.args["type"].as_str().unwrap_or("task"),
                                )
                                .unwrap_or(RouteMsgType::Task);
                                let subject_raw =
                                    tc.args["subject"].as_str().unwrap_or("").to_string();
                                let payload = tc
                                    .args
                                    .get("inline")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string());
                                let (task_subject, doc_ids) =
                                    crate::pipeline::artifacts::split_route_task_and_doc_ids(
                                        &subject_raw,
                                        payload.as_deref(),
                                    );
                                let route = RouteTo {
                                    target,
                                    msg_type,
                                    subject: task_subject,
                                    payload,
                                    doc_ids,
                                };
                                log_console!(
                                    "[control] max-iter wrap-up: routed to {} ({})",
                                    target.name(),
                                    route.subject,
                                );
                                return Ok(RunResult::Routed {
                                    text: combined,
                                    route,
                                });
                            }
                        }
                    }
                    // Not a route 鈥?feed result for context
                    session.feed_tool_result(&tc.id, &tc.name, &result);
                }
                combined
            }
        };

        let result = if last_text.is_empty() {
            format!(
                "Tool call limit reached ({}). Last response:\n{}",
                max_iter, final_text
            )
        } else {
            format!(
                "{}\n\n---\nTool call limit reached ({}). Work summary:\n{}",
                last_text, max_iter, final_text
            )
        };
        Ok(RunResult::Done(result))
    }
}

/// Convenience helper: configure step_emit on a controller from AgentInput's dept_step_tx.
/// Call this after creating the controller and before calling run().
pub fn setup_agent_step_emitter(
    controller: &mut AgentController,
    dept_step_tx: &Option<crate::models::dept_step::DeptStepSender>,
    dept: &str,
) {
    if let Some(ref tx) = dept_step_tx {
        let tx = tx.clone();
        let dept = dept.to_string();
        controller.set_step_emitter(Box::new(move |kind| {
            let _ = tx.send(DeptStepEntry::new(&dept, kind));
        }));
    }
}

impl AgentController {
    /// Interrupt the current session.
    ///
    /// Save a snapshot of the current conversation state for
    /// potential resume. Does NOT make an API call 鈥?the LLM
    /// acknowledges the interruption naturally on the next
    /// user message.
    pub async fn interrupt(&mut self, session: &mut Session) {
        self.saved = Some(session.snapshot());
        log_console!("[control] interrupt: snapshot saved");
    }

    /// Restart from a saved snapshot with a new instruction.
    pub fn restart_with(&mut self, session: &mut Session, new_instruction: &str) {
        if let Some(snap) = self.saved.take() {
            session.restore(&snap);
            session.inject(&format!(
                "System: Previous operation was interrupted. 鐨囧笣 has given a new instruction: {}",
                new_instruction
            ));
            log_console!("[control] restart_with: snapshot restored, new instruction injected");
        } else {
            log_console!(
                "[control] restart_with: no saved snapshot 鈥?injecting as new instruction"
            );
            session.inject(&format!(
                "System: 鐨囧笣 has given a new instruction, please start processing: {}",
                new_instruction
            ));
        }
    }

    /// Take the saved snapshot (for external inspection), leaving None.
    pub fn take_snapshot(&mut self) -> Option<SessionSnapshot> {
        self.saved.take()
    }
}
