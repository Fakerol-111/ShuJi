use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::api::client::ToolDefinition;
use crate::api::session::{Session, SessionSnapshot};
use crate::config::RuntimeConfig;
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

const INTERRUPT_RESPONSE: &str = "\n\n[系统] 当前处理已被皇帝中断";

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
        }
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

        for iter in 0..max_iter {
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
                    "已停止".to_string()
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
                    "已中断".to_string()
                } else {
                    last_text.clone()
                };
                return Ok(RunResult::Stopped(result));
            }
            if fast_cancel.is_some_and(|f| f.load(Ordering::SeqCst)) {
                let result = if last_text.is_empty() {
                    "已中断".to_string()
                } else {
                    last_text.clone()
                };
                return Ok(RunResult::Stopped(result));
            }

            match step_result {
                crate::api::session::StepResult::Text(text) => {
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
                    for (idx, tc) in calls.iter().enumerate() {
                        // ── Fast cancel check before each tool ──
                        if fast_cancel.is_some_and(|f| f.load(Ordering::SeqCst)) {
                            self.interrupt(session).await;
                            // Feed remaining tool results to keep message state consistent
                            for remaining in &calls[idx..] {
                                session.feed_tool_result(
                                    &remaining.id,
                                    &remaining.name,
                                    "已取消：收到快速中断信号",
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
                        let result = tool_exec(&tc.name, &tc.args).await;

                        // ── Watchdog intervention hints ──
                        // Append corrective reminders to the tool result when
                        // the LLM is stuck in a loop. This is a closed-loop
                        // intervention: the LLM sees these as part of the tool
                        // output and can self-correct without a full stop.
                        let mut intervention_hints: Vec<String> = Vec::new();
                        if same_tool_count >= config.watchdog.same_tool_warning_count {
                            intervention_hints.push(format!(
                                "⚠️ 你已连续调用 {} 工具 {} 次（相同参数）。如果这不是有意的分批操作，请考虑切换操作类型或进入下一步。",
                                tc.name, same_tool_count + 1,
                            ));
                        }
                        let is_read =
                            matches!(tc.name.as_str(), "read_file" | "list_dir" | "list_dir_tree"
                                | "find_document" | "read_document" | "search_text");
                        if is_read
                            && read_without_write >= config.watchdog.read_without_write_warning
                        {
                            intervention_hints.push(format!(
                                "⚠️ 你已读取 {} 次文件但尚未产生任何输出。请检查是否需要创建文件或修改代码。",
                                read_without_write + 1,
                            ));
                        }
                        let intervention_note = if intervention_hints.is_empty() {
                            String::new()
                        } else {
                            format!("\n\n[干预] {}", intervention_hints.join(" "))
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
                                        "禁止路由给自己（{}）。请路由到其他部门，或直接输出结果而非继续路由。",
                                        my_role
                                    );
                                    session.feed_tool_result(&tc.id, &tc.name, &msg);
                                    continue;
                                }
                                let target = match role_from_name(to_name) {
                                    Some(r) => r,
                                    None => {
                                        let msg = format!("未知目标部门: {}", to_name);
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
                                        "已取消：本批任务因路由到其他部门而中断",
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
                                    "路由到 {}（{}）：{}",
                                    target.name(),
                                    match msg_type {
                                        RouteMsgType::Task => "新任务",
                                        RouteMsgType::Replace => "替换",
                                        RouteMsgType::Interrupt => "中断",
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

                        // ── Progress note ─────────────────
                        let mut notes = Vec::new();
                        if same_tool_count >= config.watchdog.same_tool_warning_count {
                            notes.push(format!("重复调用{}", tc.name));
                        }
                        if read_without_write >= config.watchdog.read_without_write_warning + 3
                            && write_count == 0
                        {
                            notes.push(format!("读取{}次未写入", read_without_write));
                        }
                        let progress_note = if notes.is_empty() {
                            String::new()
                        } else {
                            format!("\n\n[progress] {}", notes.join("，"))
                        };

                        // ── Consecutive error tracking ────
                        let is_error = serde_json::from_str::<serde_json::Value>(&result)
                            .ok()
                            .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
                            .map(|ok| !ok)
                            .unwrap_or_else(|| {
                                result.contains("失败")
                                    || result.contains("错误")
                                    || result.contains("未知工具")
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
                            if consecutive_errors >= config.watchdog.max_consecutive_errors {
                                last_text = format!(
                                    "工具连续出错{}次，终止调用。最后错误：{}",
                                    config.watchdog.max_consecutive_errors, result
                                );
                                // Feed the current tool result before returning
                                session.feed_tool_result(&tc.id, &tc.name, &tool_content);
                                // Feed dummy results for remaining unprocessed tools
                                for remaining in &calls[idx + 1..] {
                                    session.feed_tool_result(
                                        &remaining.id,
                                        &remaining.name,
                                        "已取消：工具连续错误，终止调用",
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
                "调用{}次达上限，其中重复工具{}次，无任何写入",
                max_iter,
                same_tool_count + 1
            )
        } else if read_without_write >= 8 && write_count == 0 {
            format!(
                "调用{}次达上限，读取{}次未写入",
                max_iter,
                read_without_write + 1
            )
        } else if write_count > 0 {
            format!(
                "调用{}次达上限，写入{}次文件，读取{}次",
                max_iter, write_count, read_without_write
            )
        } else {
            format!("调用{}次达上限，无特殊异常", max_iter)
        };
        let limit_notice = format!(
            "\n\n---\n[系统] {}。{}\n\n<route to=\"内阁\" priority=\"fast\" subject=\"工具调用达上限：{}\" />",
            reason,
            "如需继续，请路由回本部门重新执行",
            reason,
        );
        let result = if last_text.is_empty() {
            format!(
                "工具调用已达上限（{}次），未返回有效内容。{}",
                max_iter, limit_notice
            )
        } else {
            format!("{}{}", last_text, limit_notice)
        };
        Ok(RunResult::Done(result))
    }

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
                "系统：之前的操作已被中断。皇帝给出了新指令：{}",
                new_instruction
            ));
            log_console!("[control] restart_with: snapshot restored, new instruction injected");
        } else {
            log_console!(
                "[control] restart_with: no saved snapshot — injecting as new instruction"
            );
            session.inject(&format!(
                "系统：皇帝给出了新指令，请开始处理：{}",
                new_instruction
            ));
        }
    }

    /// Take the saved snapshot (for external inspection), leaving None.
    pub fn take_snapshot(&mut self) -> Option<SessionSnapshot> {
        self.saved.take()
    }
}
