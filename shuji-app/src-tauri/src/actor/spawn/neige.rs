//! 内阁专用：talk 历史、皇帝 emit、Pipeline 计划提交（经 supervisor 后台执行）。

use std::collections::HashMap;
use std::sync::Arc;

use crate::models::role::Role;
use crate::pipeline::supervisor::PipelineNotifyContext;

use super::super::ActorContext;
use super::emit::emit_to_emperor_with_options;

/// 内阁成功执行后的前置处理。
///
/// 若有 pipeline plan → 委托 supervisor 后台执行并立即返回（不阻塞 mailbox）。
pub(super) async fn handle_neige_success(
    ctx: &ActorContext,
    output: &crate::agent::r#trait::AgentOutput,
    _context_msgs: &[crate::models::message::Message],
    _context_config: &Arc<HashMap<String, crate::config::RoleContextConfig>>,
    _fast_cancel: &Arc<std::sync::atomic::AtomicBool>,
) -> bool {
    if let Ok(mut talk) = ctx.talk_history.lock() {
        talk.push(format!("内阁: {}", output.content));
    }

    if !output.content.starts_with("routed to") {
        emit_to_emperor_with_options(
            &ctx.emperor_tx,
            ctx.role,
            &output.content,
            &output.decision_options,
            &output.documents,
        );
    }

    let Some(plan_json_str) = &output.plan_json else {
        return false;
    };

    delegate_pipeline_to_supervisor(ctx, plan_json_str).await;
    true
}

async fn delegate_pipeline_to_supervisor(ctx: &ActorContext, plan_json_str: &str) {
    let plan = match serde_json::from_str::<crate::pipeline::PipelinePlan>(plan_json_str) {
        Ok(p) => p,
        Err(e) => {
            log_console!("[pipeline] invalid plan JSON from 内阁: {}", e);
            return;
        }
    };

    log_console!(
        "[pipeline] 内阁 submitted plan: {} ({} steps) → supervisor",
        plan.summary,
        plan.steps.len()
    );

    let notify = PipelineNotifyContext {
        project_dir: ctx.project_dir.clone(),
        working_dir: ctx.working_dir.clone(),
        runtime_config: ctx.runtime_config.clone(),
        emperor_tx: ctx.emperor_tx.clone(),
        talk_history: ctx.talk_history.clone(),
    };

    let slot = ctx.actor_system_slot.clone();
    let supervisor = ctx.pipeline_supervisor.clone();
    let plan_owned = plan;

    tokio::spawn(async move {
        let guard = slot.lock().await;
        let Some(system) = guard.as_ref() else {
            log_console!("[pipeline-supervisor] actor system not ready");
            return;
        };
        supervisor.start_plan(plan_owned, system, notify).await;
    });
}

pub(super) async fn clear_paused_if_needed(_ctx: &ActorContext, paused_for_decision: bool) {
    if paused_for_decision {
        crate::agent::neige::NeigeAgent::clear_paused_session(&_ctx.working_dir).await;
    }
}

pub(super) fn append_neige_talk_and_build_context(
    ctx: &ActorContext,
    content: &str,
) -> Vec<crate::models::message::Message> {
    let mut context_msgs = Vec::new();
    if ctx.role != Role::Neige {
        return context_msgs;
    }

    if let Ok(talk) = ctx.talk_history.lock() {
        for line in talk.iter() {
            context_msgs.push(crate::models::message::Message::assistant(line));
        }
    }
    if let Ok(mut talk) = ctx.talk_history.lock() {
        talk.push(format!("皇帝: {}", content));
    }
    context_msgs
}
