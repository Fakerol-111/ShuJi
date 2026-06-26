//! 消息发送与取消命令。
//!
//! 本文件定义了前端可直接调用的核心交互接口：
//! - `send_message`: 向内阁发送消息，自动判断走 pipeline 恢复还是常规工作流
//! - `discuss_with_cabinet`: 独立讨论模式（不修改项目状态）
//! - `cancel_discuss` / `cancel_processing`: 取消机制

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::State;

use crate::actor::{ActorMessage, ActorSystem, FastMessage};
use crate::agent::neige::NeigeAgent;
use crate::agent::r#trait::{Agent, AgentInput};
use crate::api::client::AnthropicClient;
use crate::api::control::RouteMsgType;
use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::commands::workflow::bootstrap::ensure_actor_system;
use crate::models::chat::ChatMessage;
use crate::models::role::Role;
use crate::pipeline::engine::PipelineEngine;
use crate::pipeline::{PipelineResult, PlanRuntime};

// ============================================================================
// send_message helpers
// ============================================================================

fn interrupt_active_departments_if_needed(system: &ActorSystem) {
    if let Some(current_role) = crate::round_metrics::current_role_name() {
        if current_role != Role::Neige.name() {
            if let Some(role) = Role::from_name(&current_role) {
                if let Some(tx) = system.fast_txs.get(&role) {
                    let _ = tx.send(FastMessage::Interrupt);
                    log_console!("[commands] send_message: fast-interrupted {}", current_role);
                }
            }
        }
    }
}

fn format_pipeline_result(result: &PipelineResult) -> String {
    match result {
        PipelineResult::Complete { runtime } => {
            format!("✅ Pipeline execution complete: {}", runtime.plan.summary)
        }
        PipelineResult::AwaitingUserInput {
            step_id, question, ..
        } => {
            format!(
                "⏳ Pipeline waiting for user input (step {}): {}",
                step_id, question
            )
        }
        PipelineResult::AwaitingApproval {
            doc_id, step_id, ..
        } => {
            format!(
                "⏳ Pipeline waiting for approval (step {}, doc {})",
                step_id, doc_id
            )
        }
        PipelineResult::StepFailed {
            step_id, reason, ..
        } => {
            format!("❌ Pipeline step {} failed: {}", step_id, reason)
        }
        PipelineResult::Aborted { .. } => "🛑 Pipeline execution aborted".to_string(),
        PipelineResult::Deadlock { .. } => {
            "❌ Pipeline deadlock: remaining steps have unmet dependencies.".to_string()
        }
    }
}

async fn resume_active_pipeline(
    system: &ActorSystem,
    project_dir: &Path,
    message: &str,
    runtime_config: Arc<crate::config::RuntimeConfig>,
) -> Result<String, String> {
    let Some(engine) = PipelineEngine::load_from_disk(project_dir, system, runtime_config).await
    else {
        return Err(friendly_error("pipeline runtime not found on disk"));
    };

    let result = engine.resume_with_input(Some(message)).await;

    if matches!(result, PipelineResult::Aborted { .. }) {
        PlanRuntime::cleanup(project_dir).await;
    }

    let msg = format_pipeline_result(&result);
    let _ = system.emperor_tx.try_send(ChatMessage::new("System", &msg));
    log_console!("[pipeline] result: {}", msg);
    Ok(msg)
}

// ============================================================================
// send_message — 核心消息入口
// ============================================================================

/// 向内阁（或 pipeline）发送用户消息。
///
/// **双路径分发**:
/// 1. 磁盘上存在活跃 `PlanRuntime` → 恢复 pipeline（`AwaitingUserInput` / `AwaitingApproval`）
/// 2. 否则 → 消息路由到内阁 actor
#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    message: String,
) -> Result<String, String> {
    crate::round_metrics::start_round();

    let config = crate::commands::settings::get_config()
        .await
        .map_err(friendly_error)?;

    let p_working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("no open project"))?;
        p.working_dir.clone()
    };

    let project_dir = Path::new(&p_working_dir);
    let runtime_config = crate::commands::project::snapshot_runtime_config(&state.runtime_config);

    ensure_actor_system(app.clone(), &state, &config, &p_working_dir).await?;

    let sys_lock = state.actor_system.lock().await;
    let system = sys_lock
        .as_ref()
        .ok_or_else(|| friendly_error("actor system not initialized"))?;

    if PlanRuntime::load_from(project_dir).await.is_some() {
        log_console!("[pipeline] found active runtime on disk, resuming pipeline");
        return resume_active_pipeline(system, project_dir, &message, runtime_config).await;
    }

    drop(sys_lock);

    let sys_lock = state.actor_system.lock().await;
    let system = sys_lock
        .as_ref()
        .ok_or_else(|| friendly_error("actor system not initialized"))?;

    {
        let mut graph = system.workflow_graph.lock().await;
        let label: String = message.chars().take(60).collect();
        graph.archive_and_new(project_dir, label.trim()).await;
    }

    interrupt_active_departments_if_needed(system);

    system
        .send(&Role::Neige, ActorMessage::new(message, RouteMsgType::Task))
        .map_err(friendly_error)?;

    Ok("received".to_string())
}

// ============================================================================
// discuss_with_cabinet — 独立讨论模式
// ============================================================================

/// 与内阁进行独立讨论 — 不修改项目状态、不使用工具。
#[tauri::command]
pub async fn discuss_with_cabinet(
    state: State<'_, AppState>,
    message: String,
) -> Result<ChatMessage, String> {
    let config = crate::commands::settings::get_config()
        .await
        .map_err(friendly_error)?;

    let (working_dir, project_context) = {
        let project_opt = state.current_project.lock().await;
        let p = match project_opt.as_ref() {
            Some(p) => p,
            None => return Err(friendly_error("no open project")),
        };
        (
            p.working_dir.clone(),
            format!(
                r#"━━ Project Goal ━━
{}

━━ Current Phase ━━
{}

━━ Milestones ━━
{}

━━ Recent Conversation ━━
{}"#,
                p.goal, p.summary, p.task, p.talk,
            ),
        )
    };

    let ep = config.for_role("neige");
    let client = AnthropicClient::new(ep.api_key, ep.api_url);
    let neige = NeigeAgent::new(
        client,
        &ep.model,
        Arc::new(AtomicBool::new(false)),
        None,
        None,
    );

    let input = AgentInput {
        role: Role::Neige,
        task_description: format!(
            "(Current project state for reference)\n{}\n\n━━ Emperor Discussion ━━\n{}",
            project_context, message,
        ),
        upstream_doc_ids: vec![],
        context_messages: vec![],
        project_dir: std::path::PathBuf::from(&working_dir),
        working_dir: std::path::PathBuf::from(&working_dir),
        current_skill: None,
        resume_paused: false,
        context_window_config: Arc::new(HashMap::new()),
        runtime_config: crate::commands::project::snapshot_runtime_config(&state.runtime_config),
        discuss_mode: true,
        fast_cancel: state.discuss_cancel.clone(),
        dept_step_tx: None,
    };

    let output = neige.execute(&input).await.map_err(|e| {
        state
            .discuss_cancel
            .store(false, std::sync::atomic::Ordering::SeqCst);
        friendly_error(&e.to_string())
    })?;
    state
        .discuss_cancel
        .store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(ChatMessage::new("内阁", &output.content))
}

// ============================================================================
// 取消命令
// ============================================================================

#[tauri::command]
pub async fn cancel_discuss(state: State<'_, AppState>) -> Result<(), String> {
    state
        .discuss_cancel
        .store(true, std::sync::atomic::Ordering::SeqCst);
    log_console!("[commands] cancel_discuss: flag set");
    Ok(())
}

#[tauri::command]
pub async fn cancel_processing(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(sys) = state.actor_system.lock().await.as_ref() {
        if let Ok(map) = sys.cancel_map.lock() {
            for flag in map.values() {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            log_console!("[commands] cancel_processing: all per-actor flags set");
        }
        for tx in sys.fast_txs.values() {
            let _ = tx.send(FastMessage::Interrupt);
        }
        log_console!("[commands] cancel_processing: FastMessage::Interrupt sent to all actors");
        for tx in sys.senders.values() {
            let _ = tx.send(crate::actor::ActorMessage::interrupt());
        }
        log_console!("[commands] cancel_processing: Interrupt sent to all actors");
    }
    Ok(())
}
