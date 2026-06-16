use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures::future::join_all;

use crate::api::client::ToolDefinition;
use crate::api::session::{Session, SessionSnapshot};
use crate::config::RuntimeConfig;
use crate::models::dept_step::{DeptStepEntry, DeptStepKind};
use crate::models::role::Role;

pub type ToolFuture = Pin<Box<dyn Future<Output = String> + Send + 'static>>;

/// Callback for periodic checkpoint saves.
/// Receives an owned SessionSnapshot (cloned inside the controller),
/// so the async block does not borrow the caller's session.
pub type CheckpointFn =
    Box<dyn Fn(SessionSnapshot) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Callback for mid-run context compaction.
/// Takes the flat messages array and persists a compacted version to disk.
/// Does NOT modify the in-memory session — the compressed context is loaded
/// automatically on the next execute() call. This avoids disrupting the
/// running conversation mid-turn.
pub type CompactFn =
    Box<dyn Fn(Vec<serde_json::Value>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

const INTERRUPT_RESPONSE: &str = "\n\n[System] Current processing has been interrupted by 皇帝";

/// Callback for real-time step events emitted during AgentController::run().
/// Receives a DeptStepKind to emit for each iteration, thinking, tool call, etc.
pub type DeptStepCallback = Box<dyn Fn(DeptStepKind) + Send + Sync>;

/// Type of a cross-department routing message.
#[derive(Debug, Clone, Copy)]
pub enum RouteMsgType {
    Task,
    Replace,
    Interrupt,
}

/// Structured routing instruction produced by the LLM calling `route_to`.
#[derive(Debug, Clone)]
pub struct RouteTo {
    pub target: Role,
    pub msg_type: RouteMsgType,
    pub subject: String,
    /// Optional inline payload for short instructions (bypasses document write).
    pub payload: Option<String>,
}

/// Outcome of one AgentController::run() call.
#[derive(Debug, Clone)]
pub enum RunResult {
    /// Agent completed normally (text-only response).
    Done(String),
    /// Agent issued a route_to instruction — forward the route.
    Routed { text: String, route: RouteTo },
    /// Agent was interrupted / force-stopped / consecutive errors.
    Stopped(String),
}

impl RunResult {
    /// Extract text regardless of variant.
    pub fn text(&self) -> &str {
        match self {
            RunResult::Done(t) | RunResult::Stopped(t) => t,
            RunResult::Routed { text, .. } => text,
        }
    }

    /// Consume and return text.
    pub fn into_text(self) -> String {
        match self {
            RunResult::Done(t) | RunResult::Stopped(t) => t,
            RunResult::Routed { text, .. } => text,
        }
    }

    /// Extract RouteTo if present.
    pub fn into_route(self) -> Option<RouteTo> {
        match self {
            RunResult::Routed { route, .. } => Some(route),
            _ => None,
        }
    }

    /// Consume and return the legacy `(String, Option<RouteTo>)` tuple.
    /// This is a migration helper — new code should match on the enum directly.
    pub fn into_tuple(self) -> (String, Option<RouteTo>) {
        match self {
            RunResult::Done(text) => (text, None),
            RunResult::Routed { text, route } => (text, Some(route)),
            RunResult::Stopped(text) => (text, None),
        }
    }
}

fn route_msg_type_from_str(s: &str) -> Option<RouteMsgType> {
    match s {
        "task" => Some(RouteMsgType::Task),
        "replace" => Some(RouteMsgType::Replace),
        "interrupt" => Some(RouteMsgType::Interrupt),
        _ => None,
    }
}

pub fn role_from_name(s: &str) -> Option<Role> {
    Role::from_name(s)
}

/// Check if a tool name is a read-only operation (safe to parallelize).
fn is_read_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "read_document"
            | "list_dir"
            | "list_dir_tree"
            | "find_document"
            | "search_text"
            | "summarize_logs"
    )
}

/// Iteration budget based on tool set.
fn max_iterations_for_tools(tools: &[ToolDefinition], config: &RuntimeConfig) -> usize {
    let has_write_file = tools.iter().any(|t| {
        matches!(
            t.function.name.as_str(),
            "create_file" | "modify_file" | "append_file" | "delete_file" | "rename_file"
        )
    });
    let has_append_document = tools.iter().any(|t| {
        matches!(
            t.function.name.as_str(),
            "append_document" | "modify_document"
        )
    });

    if has_write_file {
        config.tool_iterations.write_heavy
    } else if has_append_document {
        config.tool_iterations.document_heavy
    } else {
        config.tool_iterations.readonly
    }
}

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
        }
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
    /// version to disk. The running session is NOT modified — the compressed
    /// context is loaded automatically on the next execute() call.
    /// Helps prevent unbounded context growth during long-running agent sessions.
    pub fn set_compact_handler(&mut self, handler: CompactFn, interval: u32) {
        self.compact_handler = Some(handler);
        self.compact_iter_interval = interval;
    }

    /// Run the tool-iteration loop.
    ///
    /// 1. Call `session.step()` (one API round-trip)
    /// 2. If tool calls → execute each via `tool_exec`, feed results back
    /// 3. If text → return `RunResult::Done`
    /// 4. If `cancel` is set → `interrupt()` and return `RunResult::Stopped`
    ///
    /// Route detection is output-driven: after executing a tool via `tool_exec`,
    /// the result JSON is checked for `operation == "route_to"`. This keeps the
    /// dispatcher generic — any tool can signal a control-flow transition via
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
        let max_iter = max_iterations_for_tools(tools, config);
        let mut last_text = String::new();
        let mut consecutive_errors: u32 = 0;

        // Watchdog trackers
        let mut last_tool_name = String::new();
        let mut last_tool_args = String::new();
        let mut same_tool_count: u32 = 0;
        let mut write_count: u32 = 0;
        let mut read_without_write: u32 = 0;

        // Delete-create cycle tracking
        // Maps path → cycle count. Incremented when create_file follows delete_file on same path.
        use std::collections::HashMap as HashMapColl;
        let mut delete_seen: HashMapColl<String, u32> = HashMapColl::new();
        let mut delete_create_cycles: HashMapColl<String, u32> = HashMapColl::new();

        for iter in 0..max_iter {
            // ── Emit iteration step ──
            if let Some(ref emit) = self.step_emit {
                emit(DeptStepKind::Iteration {
                    n: (iter + 1) as u32,
                });
            }

            // ── Interrupt / force-stop / fast-cancel check ──
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

            // ── Periodic checkpoint ──
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

            // ── Mid-run compaction: persist compressed context to disk ──
            // Does NOT restore the session — the compressed version is loaded
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

            let step_result = session.step().await?;

            // ── Suspension point B: API just returned, don't process if cancelled ──
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

                    // ── Emit thinking step ──
                    if !text.is_empty() {
                        if let Some(ref emit) = self.step_emit {
                            emit(DeptStepKind::Thinking {
                                content: text.clone(),
                            });
                        }
                    }

                    // ── P2-1: Parallel read-only execution ─────────────
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
                        // ── Emit tool call step ──
                        if let Some(ref emit) = self.step_emit {
                            emit(DeptStepKind::ToolCall {
                                tool: tc.name.clone(),
                                args: tc.args.clone(),
                            });
                        }

                        // ── Fast cancel check before each tool ──
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

                        // ── Same-tool watchdog ─────────────
                        // Extract key argument for similarity detection:
                        // - file/command tools → "path" or "command"
                        // - document tools → "id" (read_document, append_document, etc.)
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

                        // ── Execute tool (unified, no special-case intercept) ──
                        // Use pre-computed result for reads (P2-1 parallel exec),
                        // execute writes serially to avoid file contention.
                        let result = if let Some(pre) = read_results.remove(&idx) {
                            pre
                        } else {
                            tool_exec(&tc.name, &tc.args).await
                        };

                        // ── Emit tool result step ──
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
                        // Append corrective reminders to the tool result when
                        // the LLM is stuck in a loop. This is a closed-loop
                        // intervention: the LLM sees these as part of the tool
                        // output and can self-correct without a full stop.
                        let mut intervention_hints: Vec<String> = Vec::new();
                        if same_tool_count >= config.watchdog.same_tool_warning_count {
                            intervention_hints.push(format!(
                                "Intervention] You have called the {} tool {} times (with the same arguments). If this is not an intentional batched operation, please consider switching operations or moving to the next step.",
                                tc.name, same_tool_count + 1,
                            ));
                        }
                        let is_read = matches!(
                            tc.name.as_str(),
                            "read_file"
                                | "list_dir"
                                | "list_dir_tree"
                                | "find_document"
                                | "read_document"
                                | "search_text"
                        );
                        if is_read
                            && read_without_write >= config.watchdog.read_without_write_warning
                        {
                            intervention_hints.push(format!(
                                "⚠️ You have read files {} times without producing any output. Please check whether you need to create a file or modify code.",
                                read_without_write + 1,
                            ));
                        }
                        let intervention_note = if intervention_hints.is_empty() {
                            String::new()
                        } else {
                            format!("\n\n[Intervention] {}", intervention_hints.join(" "))
                        };

                        // ── Route detection (output-driven) ──
                        // Check the tool output for operation=="route_to" instead of
                        // matching tool names before execution. This keeps the dispatcher
                        // generic — any tool can signal a control-flow transition.
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
                                // tool_results — otherwise the next API request returns 400.
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
                                let subject = tc.args["subject"].as_str().unwrap_or("").to_string();
                                let payload = tc
                                    .args
                                    .get("inline")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string());
                                let route = RouteTo {
                                    target,
                                    msg_type,
                                    subject,
                                    payload,
                                };
                                let summary = format!(
                                    "Routed to {} ({}): {}",
                                    target.name(),
                                    match msg_type {
                                        RouteMsgType::Task => "new task",
                                        RouteMsgType::Replace => "replace",
                                        RouteMsgType::Interrupt => "interrupt",
                                    },
                                    route.subject,
                                );
                                let out = if last_text.is_empty() {
                                    summary
                                } else {
                                    format!("{}{}", last_text, summary)
                                };
                                return Ok(RunResult::Routed { text: out, route });
                            }
                        }

                        // ── Write/read tracking ───────────
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

                        // ── Delete-create cycle detection ─────
                        // Track when delete_file(path) → create_file(path) repeats on the same path.
                        let key_path = tc.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        if tc.name == "delete_file" && !key_path.is_empty() {
                            *delete_seen.entry(key_path.to_string()).or_insert(0) += 1;
                        }
                        let mut delete_cycle_hint = String::new();
                        if tc.name == "create_file" && !key_path.is_empty() {
                            if let Some(del_count) = delete_seen.get(key_path) {
                                if *del_count > 0 {
                                    let cycle_count = delete_create_cycles
                                        .entry(key_path.to_string())
                                        .or_insert(0);
                                    *cycle_count += 1;
                                    if *cycle_count >= config.watchdog.delete_create_warning_count {
                                        delete_cycle_hint = format!(
                                            "\n\n[Intervention] You have executed {} delete → create cycles on the same file ({}). \
                                            For existing files that need local changes, use edit_file (search/replace). \
                                            For batch changes, use apply_patch. The delete+create pattern wastes tokens and can lose file history.",
                                            key_path, *cycle_count,
                                        );
                                        log_console!(
                                            "[control] WATCHDOG: delete-create cycle detected ({}x) on {}",
                                            cycle_count, key_path
                                        );
                                    }
                                }
                            }
                        }

                        // ── Progress note ─────────────────
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

                        // ── Consecutive error tracking ────
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
                        if !intervention_note.is_empty() {
                            tool_content.push_str(&intervention_note);
                            log_console!(
                                "[control] WATCHDOG: intervention hint injected for {}",
                                tc.name
                            );
                        }
                        if !delete_cycle_hint.is_empty() {
                            tool_content.push_str(&delete_cycle_hint);
                            log_console!(
                                "[control] WATCHDOG: delete-cycle hint injected for {}",
                                tc.name
                            );
                        }

                        if is_error {
                            consecutive_errors += 1;
                            let first_line = result.lines().next().unwrap_or(&result);
                            let preview = if first_line.len() > 120 {
                                let end = first_line.floor_char_boundary(120);
                                format!("{}...", &first_line[..end])
                            } else {
                                first_line.to_string()
                            };
                            log_console!(
                                "[control] tool error (consecutive #{}/{})",
                                consecutive_errors,
                                config.watchdog.max_consecutive_errors
                            );
                            log_console!("  {}", preview);

                            // ── Test stalemate detection ──────
                            if tc.name == "run_tests"
                                && consecutive_errors >= config.watchdog.test_stalemate_threshold
                            {
                                let stalemate_hint = format!(
                                    "\n\n⚠️ Test stalemate detected: `run_tests` failed {} consecutive times. \
                                     Suggestions: ① Check test code and implementation match the contract \
                                     ② `read_file` to confirm current source content \
                                     ③ Follow [playbook: test-red] systematic troubleshooting \
                                     ④ If still unresolved, `wake_cabinet` for assistance",
                                    consecutive_errors
                                );
                                tool_content.push_str(&stalemate_hint);
                                log_console!(
                                    "[control] WATCHDOG: test stalemate detected (consecutive={})",
                                    consecutive_errors
                                );
                            }

                            if consecutive_errors >= config.watchdog.max_consecutive_errors {
                                last_text = format!(
                                    "Tool failed {} consecutive times, terminating. Last error: {}",
                                    config.watchdog.max_consecutive_errors, result
                                );
                                // Feed the current tool result before returning
                                session.feed_tool_result(&tc.id, &tc.name, &tool_content);
                                // Feed dummy results for remaining unprocessed tools
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
                            consecutive_errors = 0;
                        }

                        session.feed_tool_result(&tc.id, &tc.name, &tool_content);
                    }
                }
            }
        }

        // ── Max iterations reached ─────────────────────
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
        // ── Graceful wrap-up: inject summary request, do one extra step ──
        // Instead of silently cutting off, give the LLM a chance to produce
        // a coherent wrap-up and route to its superior for authorization.
        session.set_reasoning(true);
        let wrap_up = format!(
            "\n\n---\n[System] Tool call limit reached ({} calls). {}. \
             \nPlease immediately summarize the work completed and any difficulties encountered. If you need to continue, call route_to to route to 尚书令 explaining the reason and requesting further authorization. \
             \nIf no routing is needed, directly output the work summary.\n---",
            max_iter, reason,
        );
        session.inject(&wrap_up);

        let final_text = match session.step().await? {
            crate::api::session::StepResult::Text(t) => t,
            crate::api::session::StepResult::ToolCalls { calls, text } => {
                let combined = text;
                // Execute first tool call — check if it's a route_to
                if let Some(tc) = calls.first() {
                    let result = tool_exec(&tc.name, &tc.args).await;
                    // Route detection (same logic as main loop)
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&result) {
                        if v["operation"].as_str() == Some("route_to") {
                            let to_name = tc.args["to"].as_str().unwrap_or("");
                            if let Some(target) = role_from_name(to_name) {
                                let msg_type = route_msg_type_from_str(
                                    tc.args["type"].as_str().unwrap_or("task"),
                                )
                                .unwrap_or(RouteMsgType::Task);
                                let subject = tc.args["subject"].as_str().unwrap_or("").to_string();
                                let payload = tc
                                    .args
                                    .get("inline")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string());
                                let route = RouteTo {
                                    target,
                                    msg_type,
                                    subject,
                                    payload,
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
                    // Not a route — feed result for context
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
    /// potential resume. Does NOT make an API call — the LLM
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
                "System: Previous operation was interrupted. 皇帝 has given a new instruction: {}",
                new_instruction
            ));
            log_console!("[control] restart_with: snapshot restored, new instruction injected");
        } else {
            log_console!(
                "[control] restart_with: no saved snapshot — injecting as new instruction"
            );
            session.inject(&format!(
                "System: 皇帝 has given a new instruction, please start processing: {}",
                new_instruction
            ));
        }
    }

    /// Take the saved snapshot (for external inspection), leaving None.
    pub fn take_snapshot(&mut self) -> Option<SessionSnapshot> {
        self.saved.take()
    }
}
