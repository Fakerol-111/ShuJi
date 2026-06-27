//! mailbox 消息分派：Interrupt / Replace / Task。
//!
//! `dispatch_mailbox_message` 是 actor 外层循环的第一步——根据消息类型
//! 决定如何处理，并输出"Skip（中断）"或"Run（可执行）"。

use std::sync::atomic::Ordering;

use crate::api::control::RouteMsgType;
use tokio::sync::mpsc;

use super::super::{ActorContext, ActorMessage};
use super::emit::log_dept;
use super::neige::clear_paused_if_needed;

/// 跨多轮 mailbox 消息保持的 actor 运行状态。
///
/// 会在多轮消息之间保持（如 Interrupt → Task 的时序），
/// 但不会跨 exec 循环保持（每次 exec 循环开始前重新初始化或按需传入）。
#[derive(Default)]
pub(super) struct ActorRunState {
    /// Replace 消息注入的新任务内容。
    /// 当 actor 在处理旧任务时收到 Replace 消息，内容暂存于此，
    /// 等当前消息处理完毕后替代原始 content 进入 exec 循环。
    pub pending_replace: Option<String>,
    /// 当前是否处于"等待皇帝决策"的暂停状态。
    /// true 时，新 Task 应该恢复执行而不是从头开始。
    pub paused_for_decision: bool,
}

/// 一条 Task 消息解析后的执行载荷。
pub(super) struct TaskPayload {
    /// 实际要执行的任务内容（不含文档 ID）。
    pub content: String,
    /// 上游传入的文档 ID，注入 context 而非任务正文。
    pub doc_ids: Vec<String>,
    /// Pipeline 引擎等待结果的回执通道（可选）。
    pub reply_to: Option<mpsc::UnboundedSender<String>>,
    /// 为 false 时，内阁不得提取或提交 pipeline plan。
    pub allow_pipeline_plan: bool,
}

/// dispatch_mailbox_message 的返回结果。
pub(super) enum MailboxOutcome {
    /// Interrupt：跳过本轮，继续等下一条消息。
    Skip,
    /// Task / Replace：可以进入 exec 循环。
    Run(TaskPayload),
}

/// 处理一条 mailbox 消息，决定下一步动作。
///
/// **三种消息类型的分派逻辑**:
///
/// ### Interrupt
/// - 设置 cancel flag → 清除暂停状态（如有） → 排空 mailbox 残留 → 返回 Skip
/// - 注意：Interrupt **不等待** exec 循环结束，而是让 actor 立即回顶部等新消息。
///
/// ### Replace
/// - 设置 cancel flag → 清除暂停状态 → 把新内容存入 `pending_replace` → fall through 到 Run
/// - 不会让 actor 跳过本轮，而是让本轮 exec 循环使用替换后的内容。
///
/// ### Task
/// - 非暂停恢复时重置 agent 的计划（agent.reset_plan()）
/// - 清空 fast mailbox 中的残留中断信号（避免旧取消影响新任务）
/// - fall through 到 Run
///
/// ### 共性路径（Task 和 Replace 共同汇入）
/// - 重置 cancel flag（actor 准备好执行了）
/// - 如果有 `pending_replace` → 使用替换内容；否则使用原始 Task 内容
/// - 返回 `Run(TaskPayload { content, reply_to })`
pub(super) async fn dispatch_mailbox_message(
    ctx: &mut ActorContext,
    role_name: &str,
    msg: ActorMessage,
    state: &mut ActorRunState,
) -> MailboxOutcome {
    match msg.msg_type {
        // ── Interrupt：立即中断，跳过本轮 ──
        RouteMsgType::Interrupt => {
            ctx.cancel.store(true, Ordering::SeqCst);
            clear_paused_if_needed(ctx, state.paused_for_decision).await;
            state.paused_for_decision = false;
            log_dept(ctx, role_name, "interrupt signal received");
            // 排空 mailbox：中断后所有残留消息都过时了
            while ctx.rx.try_recv().is_ok() {}
            return MailboxOutcome::Skip;
        }

        // ── Replace：替换内容，继续执行 ──
        RouteMsgType::Replace => {
            ctx.cancel.store(true, Ordering::SeqCst);
            clear_paused_if_needed(ctx, state.paused_for_decision).await;
            state.paused_for_decision = false;
            state.pending_replace = Some(msg.subject.clone());
            log_dept(
                ctx,
                role_name,
                &format!("replace instruction received: {}", msg.subject),
            );
            // 注意：不 return，fall through 到下面的内容选择逻辑
        }

        // ── Task：正常执行 ──
        RouteMsgType::Task => {
            if !state.paused_for_decision {
                ctx.agent.reset_plan();
            }
            // 清空 fast mailbox 中的残留中断信号
            let mut fast_rx = ctx.fast_rx.lock().await;
            while fast_rx.try_recv().is_ok() {}
        }
    }

    // ── 共性子路径：重置 cancel + 选择执行内容 ──

    ctx.cancel.store(false, Ordering::SeqCst);

    // 如果有 pending_replace，用替换内容；否则用原始 subject
    let content = if let Some(replacement) = state.pending_replace.take() {
        log_console!(
            "[actor] {}: using replacement instead of original task",
            role_name
        );
        replacement
    } else {
        msg.subject.clone()
    };

    MailboxOutcome::Run(TaskPayload {
        content,
        doc_ids: msg.doc_ids.clone(),
        reply_to: msg.reply_to,
        allow_pipeline_plan: msg.allow_pipeline_plan,
    })
}
