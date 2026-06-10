use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{Emitter, Manager, State};
use tokio::sync::mpsc;

use crate::actor::{ActorMessage, DeptLogEntry, FastMessage};
use crate::agent::neige::NeigeAgent;
use crate::agent::r#trait::{Agent, AgentInput};
use crate::api::client::AnthropicClient;
use crate::api::control::RouteMsgType;
use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::commands::workflow::bootstrap::start_actor_system;
use crate::models::chat::ChatMessage;
use crate::models::role::Role;

/// Send a message to the 内阁 actor.
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
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };

    // Lazy init: create actor system on first message
    {
        let mut sys_lock = state.actor_system.lock().await;
        if sys_lock.is_none() {
            let (emperor_tx, mut emperor_rx) = mpsc::channel::<ChatMessage>(200);
            let (dept_log_tx, mut dept_log_rx) = mpsc::channel::<DeptLogEntry>(500);
            let (plan_tx, mut plan_rx) = mpsc::channel::<serde_json::Value>(50);
            let (milestone_tx, mut milestone_rx) = mpsc::channel::<String>(50);
            let app_handle = app.clone();

            let chat_hist = state.chat_history.clone();
            let dept_log_hist = state.dept_log_history.clone();
            let chat_persist_dir = p_working_dir.clone();

            // Forward actor output to frontend + buffer + persist
            tokio::spawn(async move {
                while let Some(msg) = emperor_rx.recv().await {
                    let _ = app_handle.emit("chat-message", &msg);
                    let mut hist = chat_hist.lock().await;
                    hist.push(msg.clone());
                    let log_dir = std::path::Path::new(&chat_persist_dir).join(".shuji");
                    let _ = tokio::fs::create_dir_all(&log_dir).await;
                    let chat_path = log_dir.join("chat.jsonl");
                    if let Ok(json) = serde_json::to_string(&msg) {
                        use tokio::io::AsyncWriteExt;
                        if let Ok(mut f) = tokio::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&chat_path)
                            .await
                        {
                            let _ = f.write_all(format!("{}\n", json).as_bytes()).await;
                        }
                    }
                }
            });

            // Forward department logs to frontend + buffer + persist
            let app2 = app.clone();
            let dept_log_dir = p_working_dir.clone();
            tokio::spawn(async move {
                while let Some(entry) = dept_log_rx.recv().await {
                    let _ = app2.emit("dept-log", &entry);
                    let mut hist = dept_log_hist.lock().await;
                    hist.push(entry.clone());
                    let log_dir = std::path::Path::new(&dept_log_dir).join(".shuji");
                    let _ = tokio::fs::create_dir_all(&log_dir).await;
                    let log_path = log_dir.join("dept-log.jsonl");
                    if let Ok(json) = serde_json::to_string(&entry) {
                        use tokio::io::AsyncWriteExt;
                        if let Ok(mut f) = tokio::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&log_path)
                            .await
                        {
                            let _ = f.write_all(format!("{}\n", json).as_bytes()).await;
                        }
                    }
                }
            });

            // Forward plan updates to frontend
            let app_plan = app.clone();
            tokio::spawn(async move {
                while let Some(plan_json) = plan_rx.recv().await {
                    let _ = app_plan.emit("plan-update", &plan_json);
                }
            });

            // Update project state on milestones
            let app3 = app.clone();
            let wd = p_working_dir.clone();
            tokio::spawn(async move {
                let s = crate::storage::shuji_dir::ShujiDir::new(&wd);
                while let Some(milestone) = milestone_rx.recv().await {
                    let st = app3.state::<AppState>();
                    let mut p_opt = st.current_project.lock().await;
                    if let Some(ref mut p) = *p_opt {
                        p.append_talk(&milestone);
                        p.summary = milestone.chars().take(120).collect();
                    }
                    let snapshot = p_opt.clone();
                    drop(p_opt);
                    if let Some(ref project) = snapshot {
                        let _ = s.save_project(project).await;
                    }
                    if let Some(ref project) = snapshot {
                        let _ = app3.emit("project-update", project);
                    }
                    let event = "milestone";
                    let role = milestone.split('|').next().unwrap_or("").trim();
                    let detail = milestone.chars().take(120).collect::<String>();
                    crate::audit::append(Path::new(&wd), event, role, "", &detail).await;
                }
            });

            let system = start_actor_system(
                &config,
                state.runtime_config.clone(),
                Path::new(&p_working_dir),
                Path::new(&p_working_dir),
                state.cancel_flag.clone(),
                emperor_tx,
                dept_log_tx,
                plan_tx,
                milestone_tx,
            )
            .await;

            *sys_lock = Some(system);
        }
    }

    let sys_lock = state.actor_system.lock().await;
    let system = sys_lock
        .as_ref()
        .ok_or_else(|| friendly_error("Actor 系统未初始化"))?;

    {
        let wd = Path::new(&p_working_dir);
        let mut graph = system.workflow_graph.lock().await;
        let label: String = message.chars().take(60).collect();
        graph.archive_and_new(wd, label.trim()).await;
    }

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

    system
        .send(&Role::Neige, ActorMessage::new(message, RouteMsgType::Task))
        .map_err(friendly_error)?;

    Ok("已接收".to_string())
}

/// Independent discussion with Cabinet — does NOT modify project state.
/// Uses `state.discuss_cancel` as the fast_cancel flag so cancel_discuss can
/// interrupt mid-execution (checked by AgentController on each tool iteration).
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
            None => return Err(friendly_error("没有加载项目")),
        };
        (
            p.working_dir.clone(),
            format!(
                r#"━━ 项目目标 ━━
{}

━━ 当前阶段 ━━
{}

━━ 项目里程碑 ━━
{}

━━ 对话记录(近期) ━━
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
            "（以下为当前项目状态，供参考）\n{}\n\n━━ 皇帝与你讨论 ━━\n{}",
            project_context, message,
        ),
        context_messages: vec![],
        project_dir: std::path::PathBuf::from(&working_dir),
        working_dir: std::path::PathBuf::from(&working_dir),
        current_skill: None,
        resume_paused: false,
        context_window_config: Arc::new(HashMap::new()),
        runtime_config: state.runtime_config.clone(),
        discuss_mode: true,
        fast_cancel: state.discuss_cancel.clone(),
    };

    let output = neige.execute(&input).await.map_err(|e| {
        state.discuss_cancel.store(false, std::sync::atomic::Ordering::SeqCst);
        friendly_error(&e.to_string())
    })?;
    state.discuss_cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(ChatMessage::new("内阁", &output.content))
}

/// Cancel an active discuss_with_cabinet call by setting the discuss_cancel flag.
/// The flag is checked by AgentController on every tool-call iteration.
#[tauri::command]
pub async fn cancel_discuss(state: State<'_, AppState>) -> Result<(), String> {
    state.discuss_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    log_console!("[commands] cancel_discuss: flag set");
    Ok(())
}

/// Cancel all running actor processing.
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
