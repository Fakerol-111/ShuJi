#![allow(dead_code)]
use std::sync::atomic::{AtomicBool, Ordering};

use crate::api::client::ToolDefinition;
use crate::api::session::{Session, SessionSnapshot};
use crate::models::role::Role;

const MAX_CONSECUTIVE_ERRORS: u32 = 5;
const MAX_TOOL_ITERATIONS_READONLY: usize = 25;
const MAX_TOOL_ITERATIONS_WRITE_HEAVY: usize = 60;
const INTERRUPT_RESPONSE: &str = "\n\n[系统] 当前处理已被皇帝中断";

/// Type of a cross-department routing message.
#[derive(Debug, Clone)]
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
/// Iteration budget: agents with `write_file` get more rounds.
fn max_iterations_for_tools(tools: &[ToolDefinition]) -> usize {
    if tools.iter().any(|t| t.function.name == "write_file") {
        MAX_TOOL_ITERATIONS_WRITE_HEAVY
    } else {
        MAX_TOOL_ITERATIONS_READONLY
    }
}

/// Control layer for tool-use agents.
///
/// Owns the tool-iteration loop, cancel/interrupt/restart lifecycle,
/// watchdog diagnostics, and anything related to "how" the LLM is
/// driven.  The LLM itself is a `Session` — this struct controls it.
pub struct AgentController {
    saved: Option<SessionSnapshot>,
}

impl AgentController {
    pub fn new() -> Self {
        Self { saved: None }
    }

    /// Run the tool-iteration loop.
    ///
    /// 1. Call `session.step()` (one API round-trip)
    /// 2. If tool calls → execute each via `tool_exec`, feed results back
    /// 3. If text → return it
    /// 4. If `cancel` is set → `interrupt()` and return intercepted text
    ///
    /// The `tool_exec` closure is synchronous by design (all tools in this
    /// codebase are fast file I/O or command spawning with internal timeout).
    pub async fn run(
        &mut self,
        session: &mut Session,
        tool_exec: &(dyn Fn(&str, &serde_json::Value) -> String + Sync),
        cancel: &AtomicBool,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<(String, Option<RouteTo>)> {
        let max_iter = max_iterations_for_tools(tools);
        let mut last_text = String::new();
        let mut consecutive_errors: u32 = 0;

        // Watchdog trackers
        let mut last_tool_name = String::new();
        let mut last_tool_args = String::new();
        let mut same_tool_count: u32 = 0;
        let mut write_count: u32 = 0;
        let mut read_without_write: u32 = 0;

        for iter in 0..max_iter {
            // ── Interrupt check ──────────────────────────────
            if cancel.load(Ordering::SeqCst) {
                self.interrupt(session).await;
                let result = format!("{}{}", last_text, INTERRUPT_RESPONSE);
                return Ok((result, None));
            }

            log_console!(
                "[control] tool-call iter={}/{}",
                iter + 1,
                max_iter
            );

            let step_result = session.step().await?;

            match step_result {
                crate::api::session::StepResult::Text(text) => {
                    return Ok((text, None));
                }

                crate::api::session::StepResult::ToolCalls(calls) => {
                    for tc in &calls {
                        // ── Same-tool watchdog ─────────────
                        let key_arg = tc.args.get("path")
                            .or_else(|| tc.args.get("command"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if tc.name == last_tool_name && key_arg == last_tool_args {
                            same_tool_count += 1;
                        } else {
                            same_tool_count = 0;
                            last_tool_name = tc.name.clone();
                            last_tool_args = key_arg.to_string();
                        }

                        // ── Cross-department routing ──────
                        if tc.name == "route_to" {
                            let target = match role_from_name(
                                tc.args["to"].as_str().unwrap_or("")
                            ) {
                                Some(r) => r,
                                None => {
                                    let msg = format!("未知目标部门: {}", tc.args["to"]);
                                    session.feed_tool_result(&tc.id, &tc.name, &msg);
                                    continue;
                                }
                            };
                            let msg_type = route_msg_type_from_str(
                                tc.args["type"].as_str().unwrap_or("task")
                            ).unwrap_or(RouteMsgType::Task);
                            let subject = tc.args["subject"]
                                .as_str()
                                .unwrap_or("")
                                .to_string();
                            let summary = format!(
                                "路由到{}（{}）：{}",
                                target.name(),
                                match msg_type {
                                    RouteMsgType::Task => "新任务",
                                    RouteMsgType::Replace => "替换",
                                    RouteMsgType::Interrupt => "中断",
                                },
                                subject,
                            );
                            let route = RouteTo { target, msg_type, subject };
                            return Ok((summary, Some(route)));
                        }

                        if same_tool_count == 3 {
                            log_console!(
                                "[control] WATCHDOG: {} repeated {} times",
                                tc.name, same_tool_count
                            );
                        }

                        // ── Execute tool ──────────────────
                        let result = tool_exec(&tc.name, &tc.args);

                        // ── Write/read tracking ───────────
                        let is_write = tc.name.contains("write");
                        let is_read = tc.name.contains("read");
                        if is_write {
                            write_count += 1;
                            read_without_write = 0;
                        } else if is_read {
                            read_without_write += 1;
                            if read_without_write == 5 {
                                log_console!(
                                    "[control] WATCHDOG: {} reads without any write",
                                    read_without_write
                                );
                            }
                        }

                        // ── Progress note ─────────────────
                        let mut notes = Vec::new();
                        if same_tool_count >= 3 {
                            notes.push(format!("重复调用{}", tc.name));
                        }
                        if read_without_write >= 8 && write_count == 0 {
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

                        if is_error {
                            consecutive_errors += 1;
                            let first_line =
                                result.lines().next().unwrap_or(&result);
                            let preview = if first_line.len() > 120 {
                                let end = first_line.floor_char_boundary(120);
                                format!("{}...", &first_line[..end])
                            } else {
                                first_line.to_string()
                            };
                            log_console!(
                                "[control] tool error (consecutive #{}/{})",
                                consecutive_errors, MAX_CONSECUTIVE_ERRORS
                            );
                            log_console!("  {}", preview);
                            if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                                last_text = format!(
                                    "工具连续出错{}次，终止调用。最后错误：{}",
                                    MAX_CONSECUTIVE_ERRORS, result
                                );
                                return Ok((last_text, None));
                            }
                        } else {
                            consecutive_errors = 0;
                        }

                        let tool_content = if progress_note.is_empty() {
                            result
                        } else {
                            format!("{}{}", result, progress_note)
                        };

                        session.feed_tool_result(&tc.id, &tc.name, &tool_content);
                    }
                }
            }
        }

        // ── Max iterations reached ─────────────────────
        log_console!("[control] tool-call limit ({}) reached", max_iter);
        let reason = if write_count == 0 && same_tool_count >= 3 {
            format!("调用{}次达上限，其中重复工具{}次，无任何写入", max_iter, same_tool_count + 1)
        } else if read_without_write >= 8 && write_count == 0 {
            format!("调用{}次达上限，读取{}次未写入", max_iter, read_without_write + 1)
        } else if write_count > 0 {
            format!("调用{}次达上限，写入{}次文件，读取{}次", max_iter, write_count, read_without_write)
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
        Ok((result, None))
    }

    /// Interrupt the current session.
    ///
    /// 1. Save a snapshot of the current conversation
    /// 2. Inject a system message "皇帝中断了当前操作"
    /// 3. Call `step()` once so the LLM acknowledges the interruption
    ///    (the response is discarded — it's just clean-up)
    pub async fn interrupt(&mut self, session: &mut Session) {
        log_console!("[control] interrupt: saving snapshot and injecting stop signal");
        self.saved = Some(session.snapshot());
        session.inject("系统：皇帝已经中断了当前操作，请在当前上下文中中止一切动作，并输出简短确认。");
        // Let the LLM acknowledge — swallow errors silently
        match session.step().await {
            Ok(_) => {}
            Err(e) => log_console!("[control] interrupt step warning: {}", e),
        }
        log_console!("[control] interrupt done");
    }

    /// Restart from a saved snapshot with a new instruction.
    ///
    /// 1. Restore the saved conversation
    /// 2. Inject "皇帝给出了新指令：..."
    /// 3. The caller then calls `run()` again with the same session
    pub fn restart_with(&mut self, session: &mut Session, new_instruction: &str) {
        if let Some(snap) = self.saved.take() {
            session.restore(&snap);
            session.inject(&format!(
                "系统：之前的操作已被中断。皇帝给出了新指令：{}",
                new_instruction
            ));
            log_console!("[control] restart_with: snapshot restored, new instruction injected");
        } else {
            log_console!("[control] restart_with: no saved snapshot — injecting as new instruction");
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
