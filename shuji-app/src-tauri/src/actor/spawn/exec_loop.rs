//! 内层 exec 循环：fast 中断、批次安全阀、agent 执行与输出分派。
//!
//! 这是 actor 系统的**真正执行核心**——每一条 Task 消息最终进入此循环，
//! 通过 LLM API + 工具调用循环产生输出。
//!
//! 循环结构：
//! ```text
//! 'exec: loop
//!   ├─ check_fast_interrupt()       — 快速中断检测
//!   ├─ check_plan_iteration_limit() — 工部批次无进展超限保护
//!   ├─ 构造 AgentInput → agent.execute()
//!   ├─ 保存 checkpoint
//!   └─ output::handle_successful_output() — 路由/暂停/批次继续/回退
//!        ├─ Break    → break 'exec
//!        └─ Continue → continue 'exec（工部批次循环）
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::agent::r#trait::AgentInput;
use crate::models::chat::ChatMessage;
use crate::models::role::Role;

use super::super::{ActorContext, FastMessage};
use super::emit::log_dept;
use super::fallback::fallback_to_dispatcher;
use super::mailbox::TaskPayload;
use super::neige::append_neige_talk_and_build_context;
use super::output::{handle_successful_output, ExecStepOutcome};

/// 运行单条 Task 对应的内层 exec 循环。
///
/// 支持工部批次 Continue 循环：当 agent.after_execute() 返回
/// LoopDecision::Continue 时，循环不退出，而是用返回的消息作为
/// 下一轮的 task_description，继续 execute。
///
/// 安全机制：
/// - fast mailbox 中断检测（每次迭代前排空）
/// - 批次无进展超限保护（同批次连续 N 轮不换batch → 强制退出）
pub(super) async fn run_exec_loop(
    ctx: &mut ActorContext,
    role_name: &str,
    task: TaskPayload,
    paused_for_decision: &mut bool,
) {
    // 仅内阁：构建 talk_history → context_messages + 追加皇帝消息到 talk
    let mut context_msgs = append_neige_talk_and_build_context(ctx, &task.content);

    crate::round_metrics::set_role(role_name);
    crate::round_metrics::mark_active(role_name);
    log_dept(ctx, role_name, "started processing");

    // 每轮 Task 只加载一次上下文窗口配置
    let context_config = load_context_config(&ctx.working_dir).await;
    // 快速中断标志：由 fast_rx 排空时设置，AgentController 每次迭代检查
    let fast_cancel = Arc::new(AtomicBool::new(false));
    let max_exec_iterations = ctx.runtime_config.actor.max_exec_iterations;

    let mut exec_iterations: u32 = 0;
    let mut last_plan_current: Option<usize> = None;

    // ── 内层 exec 循环 ──
    'exec: loop {
        exec_iterations += 1;
        crate::round_metrics::tick_iteration(role_name);

        // 快速中断：排空 fast_rx，检测 Interrupt
        if check_fast_interrupt(ctx, role_name, &fast_cancel).await {
            break 'exec;
        }

        // 批次无进展超限保护
        if check_plan_iteration_limit(
            ctx,
            role_name,
            &mut exec_iterations,
            &mut last_plan_current,
            max_exec_iterations,
        ) {
            break 'exec;
        }

        // 获取上一轮持久化的技能名（仅内阁）
        let current_skill = ctx.current_skill.lock().ok().and_then(|s| s.clone());

        // 构造 AgentInput
        if !task.doc_ids.is_empty() {
            crate::agent::runner::inject_upstream_doc_context(&mut context_msgs, &task.doc_ids);
        }

        let input = AgentInput {
            role: ctx.role,
            task_description: task.content.clone(),
            upstream_doc_ids: task.doc_ids.clone(),
            context_messages: context_msgs.clone(),
            project_dir: ctx.project_dir.clone(),
            working_dir: ctx.working_dir.clone(),
            current_skill,
            resume_paused: *paused_for_decision,
            context_window_config: context_config.clone(),
            runtime_config: ctx.runtime_config.clone(),
            discuss_mode: false,
            fast_cancel: fast_cancel.clone(),
            dept_step_tx: ctx.dept_step_tx.clone(),
        };

        // 执行 agent（含 LLM API 调用 + 工具循环）
        let preview: String = task.content.chars().take(60).collect();
        log_console!("[actor] {} ← started executing: {}", role_name, preview);
        let step_result = ctx.agent.execute(&input).await;

        // 执行后保存最终 checkpoint
        let ckpt_desc = task.content.chars().take(80).collect::<String>();
        if crate::storage::checkpoint::save_final(&ctx.working_dir, role_name, &ckpt_desc)
            .await
            .is_none()
        {
            log_console!("[actor] checkpoint save_final failed ({})", role_name);
        }

        // 根据执行结果分派
        match step_result {
            Ok(output) => {
                match handle_successful_output(
                    ctx,
                    role_name,
                    &task.content,
                    output,
                    &task.reply_to,
                    &mut context_msgs,
                    &context_config,
                    &fast_cancel,
                    paused_for_decision,
                )
                .await
                {
                    ExecStepOutcome::Break => break 'exec,
                    ExecStepOutcome::Continue { context_msgs: next } => {
                        context_msgs = next;
                        continue 'exec;
                    }
                }
            }
            Err(e) => {
                // 执行错误处理
                let err_msg = format!("execution error: {}", e);
                ctx.logger.log_agent(ctx.role, &err_msg).await;
                log_dept(ctx, role_name, &format!("❌ {}", err_msg));
                if ctx.role == Role::Neige {
                    let _ = ctx
                        .emperor_tx
                        .try_send(ChatMessage::new("System", &err_msg));
                } else {
                    fallback_to_dispatcher(ctx, role_name, &e.to_string()).await;
                }
                break 'exec;
            }
        }
    }

    crate::round_metrics::mark_idle(role_name);
}

/// 从 context_config.json 加载各角色的上下文窗口配置。
///
/// 每轮 Task 只加载一次（非每次迭代），因为 context_config.json
/// 只有用户手动编辑时才会变，不会在 Task 执行过程中变化。
async fn load_context_config(
    working_dir: &std::path::Path,
) -> Arc<HashMap<String, crate::config::RoleContextConfig>> {
    let path = working_dir.join("context_config.json");
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => {
            match serde_json::from_str::<crate::commands::settings::ContextWindowConfig>(&content) {
                Ok(cfg) => Arc::new(cfg.roles),
                Err(_) => Arc::new(HashMap::new()),
            }
        }
        Err(_) => Arc::new(HashMap::new()),
    }
}

/// 排空 fast mailbox，检测中断信号。
///
/// 每次迭代前先排空 fast_rx 中的所有 FastMessage（最多容量 16），
/// 如果有 Interrupt 则设置 fast_cancel 标志。
///
/// 返回 true 表示应该中断 exec 循环，false 表示继续。
async fn check_fast_interrupt(
    ctx: &ActorContext,
    role_name: &str,
    fast_cancel: &Arc<AtomicBool>,
) -> bool {
    // 排空 fast mailbox
    {
        let mut fast_rx = ctx.fast_rx.lock().await;
        while let Ok(msg) = fast_rx.try_recv() {
            if matches!(msg, FastMessage::Interrupt) {
                fast_cancel.store(true, Ordering::SeqCst);
                log_console!("[actor] {}: fast interrupt received", role_name);
            }
        }
    }

    // 未收到中断 → 继续执行
    if !fast_cancel.load(Ordering::SeqCst) {
        return false;
    }

    // 收到中断 → 通知皇帝，退出
    log_console!("[actor] {}: breaking exec loop (fast interrupt)", role_name);
    if let Err(e) = ctx.emperor_tx.try_send(ChatMessage::new(
        "System",
        &format!("{} has been interrupted by the Emperor", role_name),
    )) {
        log_console!("[actor] emperor_tx full (interrupt): {}", e);
    }
    true
}

/// 工部批次无进展超限保护。
///
/// 检查 agent.plan_display() 的 "current" 字段是否变化：
/// - 变了 → 有进展，重置计数器
/// - 没变 → 同批次内多次迭代，计数器累加
///
/// 超过 max_exec_iterations → 强制退出，通知皇帝。
/// 返回 true 表示应该退出。
fn check_plan_iteration_limit(
    ctx: &ActorContext,
    role_name: &str,
    exec_iterations: &mut u32,
    last_plan_current: &mut Option<usize>,
    max_exec_iterations: u32,
) -> bool {
    // 检测 plan 的 current batch 是否变化
    if let Ok(plan_json) = serde_json::from_str::<serde_json::Value>(&ctx.agent.plan_display()) {
        if let Some(cur) = plan_json["current"].as_u64() {
            let cur_usize = cur as usize;
            if *last_plan_current != Some(cur_usize) {
                // batch 变了 — 有进展，重置计数器
                *exec_iterations = 1;
                *last_plan_current = Some(cur_usize);
            }
        }
    }

    // 未超限 → 继续
    if *exec_iterations <= max_exec_iterations {
        return false;
    }

    // 超限 → 通知皇帝
    log_console!(
        "[actor] {}: plan loop exceeded {} iterations without batch progress, forcing exit",
        role_name,
        max_exec_iterations
    );
    if let Err(e) = ctx.emperor_tx.try_send(ChatMessage::new(
        "System",
        &format!(
            "{} plan loop exceeded iteration limit ({} rounds without batch progress in same batch), please re-route",
            role_name, max_exec_iterations
        ),
    )) {
        log_console!("[actor] emperor_tx full (plan-loop): {}", e);
    }
    true
}
