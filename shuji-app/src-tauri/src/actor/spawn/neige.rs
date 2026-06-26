//! 内阁专用：talk 历史、皇帝 emit、Pipeline 计划执行。
//!
//! 这些函数只在 `ctx.role == Role::Neige` 时被调用（由 output.rs 的
//! handle_successful_output 分派到非内阁部门时直接跳过）。
//!
//! 核心职责：
//! 1. append_neige_talk_and_build_context — 注入 talk_history → context_messages
//! 2. handle_neige_success — 追加 talk + emit 输出 + 可选 pipeline 执行
//! 3. clear_paused_if_needed — Interrupt/Replace 后清理暂停会话

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;

use crate::agent::r#trait::AgentInput;
use crate::models::role::Role;

use super::super::ActorContext;
use super::emit::{emit_to_emperor, emit_to_emperor_with_options};

/// 内阁成功执行后的前置处理。
///
/// **执行顺序**:
/// 1. 追加输出到 talk_history（`内阁: <输出内容>`）
/// 2. 排除纯路由通知（以 "routed to" 开头的不 emit）
/// 3. emit 到皇帝前端面板
/// 4. 如果有 pipeline plan JSON，解析并同步执行 pipeline
///
/// 返回 `true` 表示 pipeline 已处理完毕，调用方应结束 exec 循环。
pub(super) async fn handle_neige_success(
    ctx: &ActorContext,
    output: &crate::agent::r#trait::AgentOutput,
    context_msgs: &[crate::models::message::Message],
    context_config: &Arc<HashMap<String, crate::config::RoleContextConfig>>,
    fast_cancel: &Arc<AtomicBool>,
) -> bool {
    // 记录内阁发言到 talk_history
    if let Ok(mut talk) = ctx.talk_history.lock() {
        talk.push(format!("内阁: {}", output.content));
    }

    // 跳过纯路由通知（如 "routed to 中书令"）
    if !output.content.starts_with("routed to") {
        emit_to_emperor_with_options(
            &ctx.emperor_tx,
            ctx.role,
            &output.content,
            &output.decision_options,
        );
    }

    // 检查是否有 pipeline plan
    let Some(plan_json_str) = &output.plan_json else {
        return false; // 没有 pipeline plan，继续正常流程
    };

    // 有 pipeline plan → 执行 pipeline
    run_submitted_pipeline(
        ctx,
        plan_json_str,
        context_msgs,
        context_config,
        fast_cancel,
    )
    .await;
    true
}

/// 执行内阁提交的 pipeline plan。
///
/// 流程：
/// 1. 反序列化 PipelinePlan JSON
/// 2. 构造 PipelineEngine（注入 peers / cancel / workflow_graph）
/// 3. 保存 runtime 到磁盘（支持恢复）
/// 4. 在文移图上预览 pipeline 所有步骤
/// 5. 执行 pipeline（同步等待完成）
/// 6. 根据结果汇报皇帝
async fn run_submitted_pipeline(
    ctx: &ActorContext,
    plan_json_str: &str,
    context_msgs: &[crate::models::message::Message],
    context_config: &Arc<HashMap<String, crate::config::RoleContextConfig>>,
    fast_cancel: &Arc<AtomicBool>,
) {
    match serde_json::from_str::<crate::pipeline::PipelinePlan>(plan_json_str) {
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
                ctx.runtime_config.clone(),
            );
            // 持久化 runtime → 支持恢复
            engine.save().await.ok();
            // 在文移图上预填 pipeline 所有边
            engine.preview_pipeline_on_graph().await;

            let plan_msg = format!(
                "Pipeline plan submitted: {} ({} steps)",
                engine.runtime.plan.summary,
                engine.runtime.plan.steps.len(),
            );
            emit_to_emperor(&ctx.emperor_tx, ctx.role, &plan_msg);

            // 同步执行 pipeline（阻塞直到完成或失败）
            let result = engine.run().await;
            report_pipeline_result(ctx, &result, context_msgs, context_config, fast_cancel).await;
        }
        Err(e) => {
            log_console!("[pipeline] invalid plan JSON from 内阁: {}", e);
        }
    }
}

/// 根据 pipeline 执行结果向皇帝汇报。
///
/// Complete → 清理 runtime + 唤醒内阁做总结汇报
/// AwaitingUserInput → 转发等待输入提示
/// AwaitingApproval → 转发等待审批提示
/// StepFailed → 转发失败原因
/// Aborted → 清理 runtime + 通知用户
/// Deadlock → 转发死锁信息
async fn report_pipeline_result(
    ctx: &ActorContext,
    result: &crate::pipeline::PipelineResult,
    context_msgs: &[crate::models::message::Message],
    context_config: &Arc<HashMap<String, crate::config::RoleContextConfig>>,
    fast_cancel: &Arc<AtomicBool>,
) {
    use crate::pipeline::PipelineResult;

    match result {
        // pipeline 全部完成 → 让内阁审阅结果并做总结
        PipelineResult::Complete { runtime } => {
            emit_to_emperor(
                &ctx.emperor_tx,
                ctx.role,
                &format!(
                    "Pipeline plan \"{}\" fully executed, generating summary...",
                    runtime.plan.summary
                ),
            );
            crate::pipeline::PlanRuntime::cleanup(&ctx.project_dir).await;

            // 构造总结任务，让内阁重新进入 execute 做审阅
            let summary_task = format!(
                "Pipeline plan \"{}\" has been fully executed. Please review documents and reports produced by all departments, and present a complete task summary to the Emperor, explaining what was accomplished and what output was produced.",
                runtime.plan.summary
            );
            let summary_input = AgentInput {
                role: ctx.role,
                task_description: summary_task,
                upstream_doc_ids: vec![],
                context_messages: context_msgs.to_vec(),
                project_dir: ctx.project_dir.clone(),
                working_dir: ctx.working_dir.clone(),
                current_skill: None,
                resume_paused: false,
                context_window_config: context_config.clone(),
                runtime_config: ctx.runtime_config.clone(),
                discuss_mode: false,
                fast_cancel: fast_cancel.clone(),
                dept_step_tx: ctx.dept_step_tx.clone(),
            };
            if let Ok(summary_output) = ctx.agent.execute(&summary_input).await {
                let summary_text = summary_output.content;
                if !summary_text.trim().is_empty() {
                    emit_to_emperor(&ctx.emperor_tx, ctx.role, &summary_text);
                    if let Ok(mut talk) = ctx.talk_history.lock() {
                        talk.push(format!("内阁: {}", summary_text));
                    }
                }
            }
        }

        PipelineResult::AwaitingUserInput {
            step_id, question, ..
        } => {
            emit_to_emperor(
                &ctx.emperor_tx,
                ctx.role,
                &format!(
                    "Pipeline waiting for user input (step {}):\n{}",
                    step_id, question
                ),
            );
        }

        PipelineResult::AwaitingApproval {
            doc_id, step_id, ..
        } => {
            emit_to_emperor(
                &ctx.emperor_tx,
                ctx.role,
                &format!(
                    "Pipeline waiting for approval (step {}, doc {})",
                    step_id, doc_id
                ),
            );
        }

        PipelineResult::StepFailed {
            step_id, reason, ..
        } => {
            emit_to_emperor(
                &ctx.emperor_tx,
                ctx.role,
                &format!("Pipeline step {} failed: {}", step_id, reason),
            );
        }

        PipelineResult::Aborted { .. } => {
            emit_to_emperor(&ctx.emperor_tx, ctx.role, "Pipeline execution aborted");
            crate::pipeline::PlanRuntime::cleanup(&ctx.project_dir).await;
        }

        PipelineResult::Deadlock { .. } => {
            emit_to_emperor(
                &ctx.emperor_tx,
                ctx.role,
                "Pipeline deadlock: remaining steps have unmet dependencies. Please review the plan.",
            );
        }
    }
}

/// 收到 Interrupt/Replace 时清除内阁暂停会话。
///
/// 仅在 `paused_for_decision` 为 true 时实际执行清理（磁盘上的暂停 session）。
/// 如果不在暂停中，什么都不做。
pub(super) async fn clear_paused_if_needed(_ctx: &ActorContext, paused_for_decision: bool) {
    if paused_for_decision {
        crate::agent::neige::NeigeAgent::clear_paused_session(&_ctx.working_dir).await;
    }
}

/// 为内阁追加皇帝消息到 talk_history，并构建注入用的 context 消息。
///
/// 非内阁角色直接返回空 Vec（没有 talk_history）。
/// 内阁角色：
/// 1. 从 talk_history 读取所有历史对话，转为 Message::assistant
/// 2. 追加当前皇帝消息到 talk_history
pub(super) fn append_neige_talk_and_build_context(
    ctx: &ActorContext,
    content: &str,
) -> Vec<crate::models::message::Message> {
    let mut context_msgs = Vec::new();
    if ctx.role != Role::Neige {
        return context_msgs;
    }

    // 从 talk_history 构建 context_messages（注入为 assistant 消息）
    if let Ok(talk) = ctx.talk_history.lock() {
        for line in talk.iter() {
            context_msgs.push(crate::models::message::Message::assistant(line));
        }
    }
    // 追加当前皇帝消息到 talk_history
    if let Ok(mut talk) = ctx.talk_history.lock() {
        talk.push(format!("皇帝: {}", content));
    }
    context_msgs
}
