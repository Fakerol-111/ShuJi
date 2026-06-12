use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::mpsc;

use crate::agent::r#trait::AgentInput;
use crate::api::control::RouteMsgType;
use crate::models::chat::ChatMessage;
use crate::models::role::Role;

use super::{ActorContext, ActorMessage, DeptLogEntry, FastMessage};

/// Run a single actor's event loop.
///
/// Each actor:
/// 1. Receives messages from its mailbox (mpsc channel)
/// 2. Executes tasks via its agent
/// 3. Routes results (text → emperor, RouteTo → peer actor)
/// 4. Handles Interrupt/Replace via cancel flag
pub async fn run_actor(mut ctx: ActorContext) {
    let role_name = ctx.role.name().to_string();
    log_console!("[actor] {}: started", role_name);

    let mut pending_replace: Option<String> = None;
    let mut paused_for_decision = false;

    while let Some(msg) = ctx.rx.recv().await {
        // ── Interrupt / Replace handling ──────────────
        match msg.msg_type {
            RouteMsgType::Interrupt => {
                ctx.cancel.store(true, Ordering::SeqCst);
                if paused_for_decision {
                    crate::agent::neige::NeigeAgent::clear_paused_session(&ctx.working_dir).await;
                    paused_for_decision = false;
                }
                log_dept(&ctx, &role_name, "收到中断信号");
                // 清空信箱：中断后所有历史消息都已过时
                while ctx.rx.try_recv().is_ok() {}
                continue;
            }
            RouteMsgType::Replace => {
                ctx.cancel.store(true, Ordering::SeqCst);
                if paused_for_decision {
                    crate::agent::neige::NeigeAgent::clear_paused_session(&ctx.working_dir).await;
                    paused_for_decision = false;
                }
                pending_replace = Some(msg.subject.clone());
                log_dept(&ctx, &role_name, &format!("收到替换指令: {}", msg.subject));
                // 不 continue！直接 fall through 到执行逻辑，
                // 让 pending_replace 立即生效
            }
            RouteMsgType::Task => {
                if !paused_for_decision {
                    ctx.agent.reset_plan();
                }
                // 清空 fast mailbox 中的残留中断信号，避免上一轮取消的残留影响新任务
                {
                    let mut fast_rx = ctx.fast_rx.lock().await;
                    while fast_rx.try_recv().is_ok() {}
                }
            }
        }

        // ── Reset cancel flag ──────────────────────────
        ctx.cancel.store(false, Ordering::SeqCst);

        // If a Replace was queued while we were busy, use that
        // instead of the original Task content.
        let content = if let Some(replacement) = pending_replace.take() {
            log_console!(
                "[actor] {}: using replacement instead of original task",
                role_name
            );
            replacement
        } else {
            msg.subject().to_string()
        };

        // ── Build context (only 内阁 gets talk history) ──
        let mut context_msgs = Vec::new();
        if ctx.role == crate::models::role::Role::Neige {
            // Talk: full conversation with emperor
            if let Ok(talk) = ctx.talk_history.lock() {
                for line in talk.iter() {
                    context_msgs.push(crate::models::message::Message::assistant(line));
                }
            }
            // Append current emperor message to talk history
            if let Ok(mut talk) = ctx.talk_history.lock() {
                talk.push(format!("皇帝: {}", content));
            }
        }

        // Track current role for round metrics
        crate::round_metrics::set_role(&role_name);
        crate::round_metrics::mark_active(&role_name);

        // ── Execute (with plan loop for 工部尚书) ────
        log_dept(&ctx, &role_name, "开始处理");

        let mut exec_iterations: u32 = 0;
        let mut last_plan_current: Option<usize> = None;
        let max_exec_iterations = ctx.runtime_config.actor.max_exec_iterations;
        // Load per-role context window overrides ONCE per task (not every iteration).
        // context_config.json only changes when the user edits it manually — never mid-task.
        let context_config: Arc<HashMap<String, crate::config::RoleContextConfig>> = {
            let path = ctx.working_dir.join("context_config.json");
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => {
                    match serde_json::from_str::<crate::commands::settings::ContextWindowConfig>(
                        &content,
                    ) {
                        Ok(cfg) => Arc::new(cfg.roles),
                        Err(_) => Arc::new(HashMap::new()),
                    }
                }
                Err(_) => Arc::new(HashMap::new()),
            }
        };

        // Fast interrupt: set by draining fast_rx, checked by AgentController
        let fast_cancel = Arc::new(AtomicBool::new(false));
        'exec: loop {
            exec_iterations += 1;
            crate::round_metrics::tick_iteration(&role_name);

            // ── Drain fast mailbox ──────────────────────────
            {
                let mut fast_rx = ctx.fast_rx.lock().await;
                while let Ok(msg) = fast_rx.try_recv() {
                    match msg {
                        FastMessage::Interrupt => {
                            fast_cancel.store(true, Ordering::SeqCst);
                            log_console!("[actor] {}: fast interrupt received", role_name);
                        }
                    }
                }
            }
            if fast_cancel.load(Ordering::SeqCst) {
                log_console!("[actor] {}: breaking exec loop (fast interrupt)", role_name);
                if let Err(e) = ctx.emperor_tx.try_send(ChatMessage::new(
                    "系统",
                    &format!("{} 已被皇帝中断", role_name),
                )) {
                    log_console!("[actor] emperor_tx full (interrupt): {}", e);
                }
                break 'exec;
            }

            // Safety: break if stuck without plan progress.
            // If the plan's current batch has changed, the agent is making
            // legitimate progress → reset the counter.
            if let Ok(plan_json) =
                serde_json::from_str::<serde_json::Value>(&ctx.agent.plan_display())
            {
                if let Some(cur) = plan_json["current"].as_u64() {
                    let cur_usize = cur as usize;
                    if last_plan_current != Some(cur_usize) {
                        // Batch changed — progress is being made, reset counter
                        exec_iterations = 1;
                        last_plan_current = Some(cur_usize);
                    }
                }
            }
            if exec_iterations > max_exec_iterations {
                log_console!("[actor] {}: plan loop exceeded {} iterations without batch progress, forcing exit",
                    role_name, max_exec_iterations);
                if let Err(e) = ctx.emperor_tx.try_send(ChatMessage::new(
                    "系统",
                    &format!(
                        "{} 计划循环超过次数限制（同一批次内 {} 轮未推进），请重新路由",
                        role_name, max_exec_iterations
                    ),
                )) {
                    log_console!("[actor] emperor_tx full (plan-loop): {}", e);
                }
                break 'exec;
            }
            let current_skill = ctx.current_skill.lock().ok().and_then(|s| s.clone());

            let input = AgentInput {
                role: ctx.role,
                task_description: content.clone(),
                context_messages: context_msgs.clone(),
                project_dir: ctx.project_dir.clone(),
                working_dir: ctx.working_dir.clone(),
                current_skill,
                resume_paused: paused_for_decision,
                context_window_config: context_config.clone(),
                runtime_config: ctx.runtime_config.clone(),
                discuss_mode: false,
                fast_cancel: fast_cancel.clone(),
            };

            let step_result = {
                let preview: String = content.chars().take(60).collect();
                log_console!("[actor] {} ← 开始执行: {}", role_name, preview);
                ctx.agent.execute(&input).await
            };

            // ── Final checkpoint after execution ──
            let ckpt_desc = content.chars().take(80).collect::<String>();
            if crate::storage::checkpoint::save_final(&ctx.working_dir, &role_name, &ckpt_desc)
                .await
                .is_none()
            {
                log_console!("[actor] checkpoint save_final 失败 ({})", role_name);
            }

            match step_result {
                Ok(output) => {
                    let summary = output.content.chars().take(80).collect::<String>();
                    ctx.logger.log_agent(ctx.role, &summary).await;

                    if output.content.trim().is_empty() {
                        break 'exec;
                    }

                    let milestone = format!("{} | {}", role_name, summary);
                    if let Err(e) = ctx.milestone_tx.try_send(milestone) {
                        log_console!("[actor] milestone_tx.try_send 失败 (执行完成): {}", e);
                    }
                    if let Ok(mut shared) = ctx.shared_context.lock() {
                        shared.insert(ctx.role, output.content.clone());
                    }
                    if !is_failure_fallback(&content) {
                        reset_failure_retry(&ctx);
                    }
                    if ctx.role == Role::Neige {
                        if let Ok(mut talk) = ctx.talk_history.lock() {
                            talk.push(format!("内阁: {}", output.content));
                        }
                        // Emit content to emperor (even when paired with route_to),
                        // but suppress purely internal routing notifications
                        if !output.content.starts_with("已路由") {
                            self::emit_to_emperor(&ctx.emperor_tx, ctx.role, &output.content);
                        }

                        // ── Pipeline integration: if 内阁 submitted a pipeline plan ──
                        if let Some(ref plan_json_str) = output.plan_json {
                            match serde_json::from_str::<crate::pipeline::PipelinePlan>(
                                plan_json_str,
                            ) {
                                Ok(plan) => {
                                    log_console!(
                                        "[pipeline] 内阁 submitted plan: {} ({} steps)",
                                        plan.summary,
                                        plan.steps.len()
                                    );

                                    let mut engine = crate::pipeline::engine::PipelineEngine::new(
                                        plan,
                                        ctx.peers.clone(),
                                        Arc::new(HashMap::new()),
                                        Arc::new(Mutex::new(HashMap::new())),
                                        ctx.cancel.clone(),
                                        ctx.project_dir.clone(),
                                        ctx.workflow_graph.clone(),
                                    );
                                    engine.save().await.ok();

                                    // 文移图：预填充管道计划的所有 planned 边
                                    engine.preview_pipeline_on_graph().await;

                                    // Emit plan summary
                                    let plan_msg = format!(
                                        "📋 管道计划已提交：{}（{} 步骤）",
                                        engine.runtime.plan.summary,
                                        engine.runtime.plan.steps.len(),
                                    );
                                    self::emit_to_emperor(&ctx.emperor_tx, ctx.role, &plan_msg);

                                    // Execute pipeline
                                    let result = engine.run().await;

                                    match result {
                                        crate::pipeline::PipelineResult::Complete {
                                            ref runtime,
                                        } => {
                                            self::emit_to_emperor(
                                                &ctx.emperor_tx,
                                                ctx.role,
                                                &format!(
                                                "✅ 管道计划「{}」已全部执行完毕，正在生成摘要…",
                                                runtime.plan.summary),
                                            );
                                            crate::pipeline::PlanRuntime::cleanup(&ctx.project_dir)
                                                .await;

                                            // ── 唤醒内阁：汇报管道执行成果 ──
                                            let summary_task = format!(
                                                "管道计划「{}」已全部执行完毕。请查阅各部门产出的文档和报告，\
                                                向皇帝呈现一份完整的任务摘要，说明完成了哪些工作、产出了什么成果。",
                                                runtime.plan.summary);
                                            let summary_input = AgentInput {
                                                role: ctx.role,
                                                task_description: summary_task,
                                                context_messages: context_msgs.clone(),
                                                project_dir: ctx.project_dir.clone(),
                                                working_dir: ctx.working_dir.clone(),
                                                current_skill: None,
                                                resume_paused: false,
                                                context_window_config: context_config.clone(),
                                                runtime_config: ctx.runtime_config.clone(),
                                                discuss_mode: false,
                                                fast_cancel: fast_cancel.clone(),
                                            };
                                            if let Ok(summary_output) =
                                                ctx.agent.execute(&summary_input).await
                                            {
                                                let summary_text = summary_output.content;
                                                if !summary_text.trim().is_empty() {
                                                    self::emit_to_emperor(
                                                        &ctx.emperor_tx,
                                                        ctx.role,
                                                        &summary_text,
                                                    );
                                                    if let Ok(mut talk) = ctx.talk_history.lock() {
                                                        talk.push(format!(
                                                            "内阁: {}",
                                                            summary_text
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                        crate::pipeline::PipelineResult::AwaitingUserInput {
                                            step_id,
                                            question,
                                            ..
                                        } => {
                                            self::emit_to_emperor(
                                                &ctx.emperor_tx,
                                                ctx.role,
                                                &format!(
                                                    "⏳ 管道等待用户输入（步骤 {}）：\n{}",
                                                    step_id, question
                                                ),
                                            );
                                        }
                                        crate::pipeline::PipelineResult::AwaitingApproval {
                                            doc_id,
                                            step_id,
                                            ..
                                        } => {
                                            self::emit_to_emperor(
                                                &ctx.emperor_tx,
                                                ctx.role,
                                                &format!(
                                                    "⏳ 管道等待审批（步骤 {}，文档 {}）",
                                                    step_id, doc_id
                                                ),
                                            );
                                        }
                                        crate::pipeline::PipelineResult::StepFailed {
                                            step_id,
                                            reason,
                                            ..
                                        } => {
                                            self::emit_to_emperor(
                                                &ctx.emperor_tx,
                                                ctx.role,
                                                &format!(
                                                    "❌ 管道步骤 {} 执行失败：{}",
                                                    step_id, reason
                                                ),
                                            );
                                        }
                                        crate::pipeline::PipelineResult::Aborted { .. } => {
                                            self::emit_to_emperor(
                                                &ctx.emperor_tx,
                                                ctx.role,
                                                "🛑 管道执行已中止",
                                            );
                                            crate::pipeline::PlanRuntime::cleanup(&ctx.project_dir)
                                                .await;
                                        }
                                        crate::pipeline::PipelineResult::Deadlock { .. } => {
                                            self::emit_to_emperor(&ctx.emperor_tx, ctx.role,
                                                "❌ 管道死锁：所有剩余步骤的依赖无法满足，请检查计划。");
                                        }
                                    }
                                }
                                Err(e) => {
                                    log_console!("[pipeline] invalid plan JSON from 内阁: {}", e);
                                }
                            }
                            break 'exec;
                        }
                    } else {
                        log_dept(&ctx, &role_name, &format!("→ {}", output.content));
                    }

                    // Persist current skill for next turn (内阁 only)
                    if let Some(skill_name) = &output.skill {
                        if let Ok(mut s) = ctx.current_skill.lock() {
                            *s = Some(skill_name.clone());
                        }
                        crate::round_metrics::set_skill(skill_name);
                    }

                    // ── Pause for emperor decision ──
                    if output.paused {
                        if !paused_for_decision {
                            paused_for_decision = true;
                            log_console!("[actor] {}: paused for emperor decision", role_name);
                        }
                        break 'exec;
                    } else if paused_for_decision {
                        paused_for_decision = false;
                        log_console!("[actor] {}: resumed from pause", role_name);
                    }

                    // If summary mode, save output to summary_prompt in state.json
                    if output.skill.as_deref() == Some("summary") {
                        let state_path = ctx.working_dir.join(".shuji").join("state.json");
                        if let Ok(content) = tokio::fs::read_to_string(&state_path).await {
                            if let Ok(mut proj) =
                                serde_json::from_str::<serde_json::Value>(&content)
                            {
                                if let Some(obj) = proj.as_object_mut() {
                                    obj.insert(
                                        "summary_prompt".into(),
                                        serde_json::Value::String(output.content.clone()),
                                    );
                                    let content =
                                        serde_json::to_string_pretty(&proj).unwrap_or_default();
                                    let _ = tokio::fs::write(&state_path, content).await;
                                }
                            }
                        }
                    }

                    // If route_to present → forward and exit plan loop
                    if let Some(route) = output.route {
                        super::routing::forward_route(&ctx, route).await;
                        break 'exec;
                    }
                    // ── Pipeline reply_to: if pipeline engine is waiting for output,
                    // send AgentOutput.content back via reply_to channel.
                    // Only sent on LoopDecision::Done (not on Continue for 工部 batch loop).
                    // ──
                    let should_reply = output.route.is_none() && msg.reply_to.is_some();

                    // Let agent decide: need another round?
                    match ctx.agent.after_execute(&output) {
                        crate::agent::r#trait::LoopDecision::Continue(ctx_msg) => {
                            // Emit plan progress to frontend
                            let done =
                                ctx_msg.matches("[x]").count() + ctx_msg.matches("[X]").count();
                            let total = done + ctx_msg.matches("[ ]").count();
                            let plan_action = if total > 0 {
                                format!("执行计划：{}/{} 完成", done, total)
                            } else {
                                "执行计划已输出".to_string()
                            };
                            if let Err(e) = ctx
                                .dept_log_tx
                                .try_send(DeptLogEntry::new(&role_name, &plan_action))
                            {
                                log_console!("[actor] dept_log_tx.try_send 失败 (计划): {}", e);
                            }
                            if let Err(e) = ctx
                                .dept_log_tx
                                .try_send(DeptLogEntry::with_detail(&role_name, "计划", &ctx_msg))
                            {
                                log_console!("[actor] dept_log_tx.try_send 失败 (计划详情): {}", e);
                            }
                            if let Err(e) = ctx
                                .milestone_tx
                                .try_send(format!("{} | {}", role_name, plan_action))
                            {
                                log_console!("[actor] milestone_tx.send 失败 (计划): {}", e);
                            }
                            // Emit structured plan progress for frontend card
                            let plan_json = ctx.agent.plan_display();
                            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&plan_json)
                            {
                                let _ = ctx.plan_tx.try_send(value);
                            }
                            context_msgs.push(crate::models::message::Message::user(&ctx_msg));
                            continue 'exec;
                        }
                        crate::agent::r#trait::LoopDecision::Done => {
                            // Send reply back to pipeline engine if waiting
                            if should_reply {
                                if let Some(reply) = &msg.reply_to {
                                    let _ = reply.send(output.content.clone());
                                }
                            }
                            break 'exec;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("执行错误: {}", e);
                    ctx.logger.log_agent(ctx.role, &err_msg).await;
                    log_dept(&ctx, &role_name, &format!("❌ {}", err_msg));
                    if ctx.role == Role::Neige {
                        let _ = ctx.emperor_tx.try_send(ChatMessage::new("系统", &err_msg));
                    } else {
                        fallback_to_dispatcher(&ctx, &role_name, &e.to_string()).await;
                    }
                    break 'exec;
                }
            }
        }
        crate::round_metrics::mark_idle(&role_name);
    }

    crate::round_metrics::mark_idle(&role_name);
    log_console!("[actor] {}: stopped", role_name);
}

const MAX_FAILURE_RETRIES: u32 = 3;

fn is_failure_fallback(content: &str) -> bool {
    content.trim_start().starts_with("[失败回退")
}

fn reset_failure_retry(ctx: &ActorContext) {
    if let Ok(mut retries) = ctx.failure_retries.lock() {
        retries.remove(&ctx.role);
    }
}

async fn fallback_to_dispatcher(ctx: &ActorContext, role_name: &str, error: &str) {
    if ctx.role == Role::Shangshuling {
        let _ = ctx.emperor_tx.try_send(ChatMessage::new(
            "系统",
            &format!("{} 执行失败，无法自回退。错误: {}", ctx.role.name(), error),
        ));
        return;
    }

    let retry_count = match ctx.failure_retries.lock() {
        Ok(mut retries) => {
            let next = retries.get(&ctx.role).copied().unwrap_or(0) + 1;
            retries.insert(ctx.role, next);
            next
        }
        Err(_) => {
            let _ = ctx.emperor_tx.try_send(ChatMessage::new(
                "系统",
                &format!(
                    "{} 执行失败，且无法记录重试次数。错误: {}",
                    ctx.role.name(),
                    error
                ),
            ));
            return;
        }
    };

    if retry_count > MAX_FAILURE_RETRIES {
        if let Err(e) = ctx.emperor_tx.try_send(ChatMessage::new(
            "系统",
            &format!(
                "{} 执行失败，已重试 {} 次仍未解决。最后错误: {}\n请人工介入。",
                ctx.role.name(),
                MAX_FAILURE_RETRIES,
                error,
            ),
        )) {
            log_console!("[actor] emperor_tx.try_send 失败 (重试耗尽): {}", e);
        }
        log_dept(ctx, role_name, "失败回退次数耗尽，已上报");
        return;
    }

    let fallback_content = format!(
        "[失败回退|retry={}/{}]\n部门: {}\n错误: {}\n请重新调度到合适的部门修复。",
        retry_count,
        MAX_FAILURE_RETRIES,
        ctx.role.name(),
        error,
    );

    match ctx.peers.get(&Role::Shangshuling) {
        Some(tx) => {
            let _ = tx.send(ActorMessage::new(fallback_content, RouteMsgType::Task));
            log_dept(
                ctx,
                role_name,
                &format!(
                    "→ 回退到尚书令 (retry {}/{})",
                    retry_count, MAX_FAILURE_RETRIES
                ),
            );
        }
        None => {
            let _ = ctx.emperor_tx.try_send(ChatMessage::new(
                "系统",
                &format!(
                    "{} 执行失败且无法回退（找不到尚书令）: {}",
                    ctx.role.name(),
                    error
                ),
            ));
        }
    }
}

/// Emit a chat message to the emperor's frontend.
/// Parses `<options>` tags from the content into structured ChatOption objects.
fn emit_to_emperor(tx: &mpsc::Sender<ChatMessage>, role: Role, content: &str) {
    let role_name = role.name();
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return;
    }
    let (clean_content, options) = parse_options(trimmed);
    let mut msg = ChatMessage::new(role_name, &clean_content);
    msg.options = options;
    if let Err(e) = tx.try_send(msg) {
        log_console!("[actor] emperor_tx.try_send 失败 ({}): {}", role_name, e);
    }
}

/// Extract `<options>` from content, return (content_without_options, parsed_options).
fn parse_options(content: &str) -> (String, Vec<crate::models::chat::ChatOption>) {
    let mut options = Vec::new();
    // Find <options> ... </options> block
    if let Some(start) = content.find("<options>") {
        if let Some(end) = content.find("</options>") {
            let block = &content[start..end + "</options>".len()];
            // Extract each <option key="X" label="Y" description="Z" />
            let mut pos = 0;
            while let Some(opt_start) = block[pos..].find("<option ") {
                let abs_start = pos + opt_start;
                if let Some(opt_end) = block[abs_start..].find("/>") {
                    let tag = &block[abs_start..abs_start + opt_end + 2];
                    let key = extract_attr(tag, "key").unwrap_or_default();
                    let label = extract_attr(tag, "label").unwrap_or_default();
                    let desc = extract_attr(tag, "description").unwrap_or_default();
                    if !key.is_empty() {
                        options.push(crate::models::chat::ChatOption {
                            key,
                            label,
                            description: desc,
                        });
                    }
                    pos = abs_start + opt_end + 2;
                } else {
                    break;
                }
            }
            // Remove the options block from content
            let clean = format!(
                "{}{}",
                content[..start].trim(),
                content[end + "</options>".len()..].trim()
            );
            return (clean, options);
        }
    }
    (content.to_string(), options)
}

/// Extract an attribute value from an XML-like tag string.
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(val_start) = tag.find(&pattern) {
        let val_begin = val_start + pattern.len();
        if let Some(val_end) = tag[val_begin..].find('\"') {
            return Some(tag[val_begin..val_begin + val_end].to_string());
        }
    }
    None
}

/// Emit a department log entry to the frontend status panel.
pub(super) fn log_dept(ctx: &ActorContext, dept: &str, action: &str) {
    if let Err(e) = ctx.dept_log_tx.try_send(DeptLogEntry::new(dept, action)) {
        log_console!("[actor] dept_log_tx.try_send 失败 ({}): {}", dept, e);
    }
}
