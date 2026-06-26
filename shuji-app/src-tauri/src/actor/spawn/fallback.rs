//! 非内阁部门执行失败时回退尚书令。
//!
//! 核心原则：
//! - **非内阁失败** → 构造 `[failure fallback]` 消息发给尚书令重调度
//! - **尚书令失败** → 不自回退（无限循环风险），直接汇报皇帝
//! - **最大 3 次重试** → 超限后通知皇帝人工介入

use crate::api::control::RouteMsgType;
use crate::models::chat::ChatMessage;
use crate::models::role::Role;

use super::super::{ActorContext, ActorMessage};
use super::emit::log_dept;

/// 最大失败重试次数：超限后停止回退，通知皇帝人工介入。
const MAX_FAILURE_RETRIES: u32 = 3;

/// 检查内容是否为 failure fallback 消息。
///
/// fallback 消息以 `[failure fallback` 开头，用于：
/// 1. 防止 fallback 循环（fallback 消息不会触发新的 fallback）
/// 2. 复位重试计数（正常消息重置计数）
pub(super) fn is_failure_fallback(content: &str) -> bool {
    content.trim_start().starts_with("[failure fallback")
}

/// 重置指定角色的失败重试计数，成功执行后调用。
pub(super) fn reset_failure_retry(ctx: &ActorContext) {
    if let Ok(mut retries) = ctx.failure_retries.lock() {
        retries.remove(&ctx.role);
    }
}

/// 非内阁部门执行失败时自动回退到尚书令重新调度。
///
/// **回退流程**:
/// 1. 累加失败计数: `failure_retries[role] += 1`
/// 2. 超过 3 次 → 通知皇帝人工介入，停止回退
/// 3. 构造 `[failure fallback|retry=N/3]` 消息
/// 4. 发送到尚书令 mailbox → 尚书令根据错误信息重新路由
///
/// **尚书令自身失败** → 不自回退，直接汇报皇帝（避免无限循环）。
pub(super) async fn fallback_to_dispatcher(ctx: &ActorContext, role_name: &str, error: &str) {
    // ── 尚书令自身失败 ──
    if ctx.role == Role::Shangshuling {
        let _ = ctx.emperor_tx.try_send(ChatMessage::new(
            "System",
            &format!(
                "{} execution failed, cannot self-fallback. Error: {}",
                ctx.role.name(),
                error
            ),
        ));
        return;
    }

    // ── 累加重试计数 ──
    let retry_count = match ctx.failure_retries.lock() {
        Ok(mut retries) => {
            let next = retries.get(&ctx.role).copied().unwrap_or(0) + 1;
            retries.insert(ctx.role, next);
            next
        }
        Err(_) => {
            let _ = ctx.emperor_tx.try_send(ChatMessage::new(
                "System",
                &format!(
                    "{} execution failed, and retry count could not be recorded. Error: {}",
                    ctx.role.name(),
                    error
                ),
            ));
            return;
        }
    };

    // ── 超过最大重试次数 ──
    if retry_count > MAX_FAILURE_RETRIES {
        if let Err(e) = ctx.emperor_tx.try_send(ChatMessage::new(
            "System",
            &format!(
                "{} execution failed after {} retries. Last error: {}\nManual intervention required.",
                ctx.role.name(),
                MAX_FAILURE_RETRIES,
                error,
            ),
        )) {
            log_console!("[actor] emperor_tx.try_send failed (retries exhausted): {}", e);
        }
        log_dept(
            ctx,
            role_name,
            "failure fallback retries exhausted, reported",
        );
        return;
    }

    // ── 构造 fallback 消息 ──
    let fallback_content = format!(
        "[failure fallback|retry={}/{}]\nDepartment: {}\nError: {}\nPlease re-route to an appropriate department to fix.",
        retry_count,
        MAX_FAILURE_RETRIES,
        ctx.role.name(),
        error,
    );

    // ── 发送到尚书令 ──
    match ctx.peers.get(&Role::Shangshuling) {
        Some(tx) => {
            let _ = tx.send(ActorMessage::new(fallback_content, RouteMsgType::Task));
            log_dept(
                ctx,
                role_name,
                &format!(
                    "→ fallback to 尚书令 (retry {}/{})",
                    retry_count, MAX_FAILURE_RETRIES
                ),
            );
        }
        None => {
            let _ = ctx.emperor_tx.try_send(ChatMessage::new(
                "System",
                &format!(
                    "{} execution failed and cannot fallback (尚书令 not found): {}",
                    ctx.role.name(),
                    error
                ),
            ));
        }
    }
}
