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

use tauri::{AppHandle, State};

use crate::actor::{ActorMessage, ActorSystem, FastMessage};
use crate::agent::neige::NeigeAgent;
use crate::agent::r#trait::{Agent, AgentInput};
use crate::api::client::AnthropicClient;
use crate::api::control::RouteMsgType;
use crate::api::stream::ChatDeltaEvent;
use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::commands::workflow::bootstrap::ensure_actor_system;
use crate::models::chat::ChatMessage;
use crate::models::message::Message;
use crate::models::role::Role;
use crate::pipeline::supervisor::PipelineNotifyContext;
use crate::pipeline::{should_resume_from_disk, PlanRuntime};

// ============================================================================
// send_message helpers
// ============================================================================

fn interrupt_active_departments_if_needed(system: &ActorSystem) {
    if let Some(current_role) = crate::round_metrics::current_role_name() {
        if current_role != Role::Neige.name() {
            if let Some(role) = Role::from_name(&current_role) {
                if let Some(tx) = system.fast_txs.get(&role) {
                    let _ = tx.try_send(FastMessage::Interrupt);
                    log_console!("[commands] send_message: fast-interrupted {}", current_role);
                }
            }
        }
    }
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

    let has_paused_runtime = PlanRuntime::load_from(project_dir).await.is_some();

    if should_resume_from_disk(has_paused_runtime, state.pipeline_supervisor.is_running()) {
        log_console!("[pipeline] paused runtime on disk, resuming via supervisor");
        let notify = PipelineNotifyContext {
            project_dir: project_dir.to_path_buf(),
            working_dir: project_dir.to_path_buf(),
            runtime_config: runtime_config.clone(),
            emperor_tx: system.emperor_tx.clone(),
            talk_history: Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        return state
            .pipeline_supervisor
            .resume_with_input(project_dir, system, notify, Some(&message))
            .await
            .map_err(friendly_error);
    }

    if has_paused_runtime {
        log_console!(
            "[pipeline] runtime exists but pipeline still running — message goes to neige"
        );
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

    state.pipeline_supervisor.clear_submission_guards();

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
        allow_pipeline_plan: true,
    };

    let output = neige.execute(&input).await.map_err(|e| {
        state
            .discuss_cancel
            .store(false, std::sync::atomic::Ordering::SeqCst);
        friendly_error(e.to_string())
    })?;
    state
        .discuss_cancel
        .store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(ChatMessage::new("内阁", &output.content))
}

/// Build system prompt + user content for discuss streaming (text-only, no tools).
async fn build_discuss_stream_context(
    state: &AppState,
    message: &str,
) -> Result<(String, String), String> {
    let project_opt = state.current_project.lock().await;
    let p = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("no open project"))?;

    let working_dir = std::path::PathBuf::from(&p.working_dir);
    let project_context = format!(
        r#"━━ Project Goal ━━
{}

━━ Current Phase ━━
{}

━━ Milestones ━━
{}

━━ Recent Conversation ━━
{}"#,
        p.goal, p.summary, p.task, p.talk,
    );

    let user_content = format!(
        "(Current project state for reference)\n{}\n\n━━ Emperor Discussion ━━\n{}",
        project_context, message
    );

    let mut system_prompt = include_str!("../../agent/neige/prompt.md").to_string();

    let soul_content = crate::learning::load_role_soul(&working_dir, "Neige").await;
    if !soul_content.trim().is_empty() {
        system_prompt.push_str("\n\n[soul: Neige]\n");
        system_prompt.push_str(&soul_content);
    }

    let mut skill_content = NeigeAgent::load_skill("discuss", &working_dir).await;
    if skill_content.is_empty() {
        let fallback = if message.contains("bug") || message.contains("修复") {
            "workflow_bugfix"
        } else if message.contains("重构") {
            "workflow_refactor"
        } else if message.contains("优化") {
            "workflow_optimize"
        } else {
            "clarify"
        };
        skill_content = NeigeAgent::load_skill(fallback, &working_dir).await;
    }
    if !skill_content.is_empty() {
        system_prompt.push_str("\n\n[skill: discuss]\n");
        system_prompt.push_str(&skill_content);
    }

    Ok((system_prompt, user_content))
}

/// Stream discuss-mode reply via SSE — emits `chat-delta` / `chat-complete` events.
#[tauri::command]
pub async fn discuss_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    message: String,
    message_id: String,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    state.discuss_cancel.store(false, Ordering::SeqCst);

    let config = crate::commands::settings::get_config()
        .await
        .map_err(friendly_error)?;

    let (system_prompt, user_content) = build_discuss_stream_context(&state, &message).await?;

    let ep = config.for_role("neige");
    let client = AnthropicClient::new(ep.api_key, ep.api_url);
    let cancel = state.discuss_cancel.clone();
    let msgs = vec![Message::user(&user_content)];

    let app_for_delta = app.clone();
    let delta_id = message_id.clone();
    let mut full_text = String::new();

    let stream_result = client
        .stream_message_with_reasoning(
            &system_prompt,
            &msgs,
            &ep.model,
            cancel.clone(),
            crate::config::ResolvedReasoningPolicy {
                enabled: true,
                effort: crate::config::ReasoningEffort::Low,
                budget_tokens: 0,
            },
            |delta| {
                if cancel.load(Ordering::SeqCst) {
                    return Ok(());
                }
                full_text.push_str(delta);
                let _ = crate::events::emit_chat_delta(
                    &app_for_delta,
                    &ChatDeltaEvent {
                        message_id: delta_id.clone(),
                        role: "内阁".into(),
                        delta: delta.to_string(),
                    },
                );
                Ok(())
            },
        )
        .await;

    state.discuss_cancel.store(false, Ordering::SeqCst);

    if cancel.load(Ordering::SeqCst) {
        log_console!("[commands] discuss_stream: cancelled");
        return Ok(());
    }

    let text = stream_result.map_err(|e| friendly_error(e.to_string()))?;
    let final_content = if text.is_empty() { full_text } else { text };

    let mut msg = ChatMessage::new("内阁", &final_content);
    msg.id = message_id;
    crate::events::emit_chat_complete(&app, &msg).map_err(|e| friendly_error(e.to_string()))?;

    Ok(())
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
        state.pipeline_supervisor.abort_current(sys).await;
        log_console!("[commands] cancel_processing: pipeline supervisor aborted");
        if let Ok(map) = sys.cancel_map.lock() {
            for flag in map.values() {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            log_console!("[commands] cancel_processing: all per-actor flags set");
        }
        for tx in sys.fast_txs.values() {
            let _ = tx.try_send(FastMessage::Interrupt);
        }
        log_console!("[commands] cancel_processing: FastMessage::Interrupt sent to all actors");
        for tx in sys.senders.values() {
            let _ = tx.send(crate::actor::ActorMessage::interrupt());
        }
        log_console!("[commands] cancel_processing: Interrupt sent to all actors");
    }
    Ok(())
}
