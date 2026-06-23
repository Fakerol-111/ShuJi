//! Agent 成功输出后的通用分派：路由、暂停、批次继续、plan 推送。
//!
//! `handle_successful_output` 处理 agent.execute() 成功返回后的所有逻辑，
//! 包括：空输出检查、里程碑日志、共享上下文更新、内阁特化处理、
//! 暂停决策、summary 持久化、路由、工部批次继续。
//!
//! 返回 `ExecStepOutcome` 告诉调用方（exec 循环）是 break 还是 continue。

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::agent::r#trait::{AgentOutput, LoopDecision};
use crate::models::role::Role;
use tokio::sync::mpsc;

use super::super::{ActorContext, DeptLogEntry};
use super::emit::log_dept;
use super::fallback::{is_failure_fallback, reset_failure_retry};
use super::neige::handle_neige_success;

/// exec 循环单步结束后的动作。
pub(super) enum ExecStepOutcome {
    /// 结束内层 exec 循环。
    Break,
    /// 工部等 agent 请求继续下一批次。
    Continue {
        context_msgs: Vec<crate::models::message::Message>,
    },
}

/// 处理 agent 的成功的输出。
///
/// **执行顺序**:
/// ```text
/// 1. 空输出 → Break
/// 2. 记录里程碑 + 更新共享上下文
/// 3. 检查是否 failure fallback 消息（防止 fallback 循环）
/// 4. 内阁特化 ← pipeline 计划在此执行
/// 5. 持久化技能名（仅内阁）
/// 6. 检查 paused 标志 ← 等待皇帝决策
/// 7. summary 技能 → 持久化到 state.json
/// 8. 路由 → forward_route
/// 9. after_execute 决定:
///    ├─ Continue → 批次继续
///    └─ Done     → 最终 plan 推送 + pipeline reply
/// ```
pub(super) async fn handle_successful_output(
    ctx: &ActorContext,
    role_name: &str,
    task_content: &str,
    output: AgentOutput,
    reply_to: &Option<mpsc::UnboundedSender<String>>,
    context_msgs: &mut Vec<crate::models::message::Message>,
    context_config: &Arc<HashMap<String, crate::config::RoleContextConfig>>,
    fast_cancel: &Arc<AtomicBool>,
    paused_for_decision: &mut bool,
) -> ExecStepOutcome {
    // 空输出：agent 执行异常但没有明确失败 → 退出
    if output.content.trim().is_empty() {
        return ExecStepOutcome::Break;
    }

    // 记录日志 + 里程碑
    let summary = output.content.chars().take(80).collect::<String>();
    ctx.logger.log_agent(ctx.role, &summary).await;

    let milestone = format!("{} | {}", role_name, summary);
    if let Err(e) = ctx.milestone_tx.try_send(milestone) {
        log_console!(
            "[actor] milestone_tx.try_send failed (execution complete): {}",
            e
        );
    }

    // 更新跨 actor 共享上下文（供 fallback 时参考上游输出）
    if let Ok(mut shared) = ctx.shared_context.lock() {
        shared.insert(ctx.role, output.content.clone());
    }

    // 非 fallback 消息 → 重置失败重试计数
    if !is_failure_fallback(task_content) {
        reset_failure_retry(ctx);
    }

    // ── 内阁特化逻辑 ──
    if ctx.role == Role::Neige {
        // 此处会处理：talk_history 追加、emit 到皇帝、pipeline plan 执行
        if handle_neige_success(ctx, &output, context_msgs, context_config, fast_cancel).await {
            // pipeline 已执行完毕，退出 exec 循环
            return ExecStepOutcome::Break;
        }
    } else {
        // 非内阁部门：写部门日志
        log_dept(ctx, role_name, &format!("→ {}", output.content));
    }

    // 持久化当前技能名（仅内阁）
    persist_skill(ctx, &output);

    // ── 暂停等待皇帝决策 ──
    if output.paused {
        if !*paused_for_decision {
            *paused_for_decision = true;
            log_console!("[actor] {}: paused for emperor decision", role_name);
        }
        return ExecStepOutcome::Break;
    } else if *paused_for_decision {
        // 曾暂停，现在恢复（皇帝已决策）
        *paused_for_decision = false;
        log_console!("[actor] {}: resumed from pause", role_name);
    }

    // ── summary 技能：输出持久化到 state.json ──
    if output.skill.as_deref() == Some("summary") {
        persist_summary_prompt(ctx, &output.content).await;
    }

    // ── 路由：有 route_to → 转发并退出 ──
    if let Some(route) = output.route {
        super::super::routing::forward_route(ctx, route).await;
        return ExecStepOutcome::Break;
    }

    // ── pipeline reply_to ──
    let should_reply = output.route.is_none() && reply_to.is_some();

    // ── 根据 after_execute 决定下一步 ──
    match ctx.agent.after_execute(&output) {
        LoopDecision::Continue(ctx_msg) => {
            // 工部批次继续
            emit_plan_progress(ctx, role_name, &ctx_msg);
            context_msgs.push(crate::models::message::Message::user(&ctx_msg));
            ExecStepOutcome::Continue {
                context_msgs: context_msgs.clone(),
            }
        }
        LoopDecision::Done => {
            // 发送最终 plan 更新
            push_final_plan_update(ctx);

            // 回复 pipeline engine 等待结果
            if should_reply {
                if let Some(reply) = reply_to {
                    let _ = reply.send(output.content.clone());
                }
            }

            ExecStepOutcome::Break
        }
    }
}

/// 持久化 agent 的技能名（仅内阁，跨轮次保持当前 skill）。
fn persist_skill(ctx: &ActorContext, output: &AgentOutput) {
    if let Some(skill_name) = &output.skill {
        if let Ok(mut s) = ctx.current_skill.lock() {
            *s = Some(skill_name.clone());
        }
        crate::round_metrics::set_skill(skill_name);
    }
}

/// summary 技能输出持久化到 state.json 的 summary_prompt 字段。
///
/// 这样后续 summary 技能可以通过上下文感知之前的输出摘要，
/// 实现增量更新而非从头生成。
async fn persist_summary_prompt(ctx: &ActorContext, content: &str) {
    let state_path = ctx.working_dir.join(".shuji").join("state.json");
    if let Ok(file_content) = tokio::fs::read_to_string(&state_path).await {
        if let Ok(mut proj) = serde_json::from_str::<serde_json::Value>(&file_content) {
            if let Some(obj) = proj.as_object_mut() {
                obj.insert(
                    "summary_prompt".into(),
                    serde_json::Value::String(content.to_string()),
                );
                let content = serde_json::to_string_pretty(&proj).unwrap_or_default();
                let _ = tokio::fs::write(&state_path, content).await;
            }
        }
    }
}

/// emit 工部批次计划进度到前端（dept_log + milestone + plan_tx）。
fn emit_plan_progress(ctx: &ActorContext, role_name: &str, ctx_msg: &str) {
    let done = ctx_msg.matches("[x]").count() + ctx_msg.matches("[X]").count();
    let total = done + ctx_msg.matches("[ ]").count();
    let plan_action = if total > 0 {
        format!("Plan: {}/{} complete", done, total)
    } else {
        "Plan output".to_string()
    };

    // 部门日志：进度摘要
    if let Err(e) = ctx
        .dept_log_tx
        .try_send(DeptLogEntry::new(role_name, &plan_action))
    {
        log_console!("[actor] dept_log_tx.try_send failed (plan): {}", e);
    }
    // 部门日志：详细批次内容
    if let Err(e) = ctx
        .dept_log_tx
        .try_send(DeptLogEntry::with_detail(role_name, "plan", ctx_msg))
    {
        log_console!("[actor] dept_log_tx.try_send failed (plan detail): {}", e);
    }
    // 里程碑
    if let Err(e) = ctx
        .milestone_tx
        .try_send(format!("{} | {}", role_name, plan_action))
    {
        log_console!("[actor] milestone_tx.send failed (plan): {}", e);
    }

    // 结构化 plan 进度 → 前端工部进度卡片
    let plan_json = ctx.agent.plan_display();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&plan_json) {
        let _ = ctx.plan_tx.try_send(value);
    }
}

/// 发送最终 plan 更新给前端（exec 循环结束后）。
fn push_final_plan_update(ctx: &ActorContext) {
    let plan_json = ctx.agent.plan_display();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&plan_json) {
        if !value.is_null() {
            let _ = ctx.plan_tx.try_send(value);
        }
    }
}
