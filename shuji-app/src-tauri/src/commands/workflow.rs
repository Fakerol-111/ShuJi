use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tauri::{Emitter, Manager, State};
use tokio::sync::mpsc;

use crate::actor::{ActorContext, ActorMessage, ActorSystem, DeptLogEntry};
use crate::agent::r#trait::{Agent, AgentInput};
use crate::agent::zhongshuling::ZhongshulingAgent;
use crate::agent::bingbushangshu::BingbuShangshuAgent;
use crate::agent::gongbushangshu::GongbuShangshuAgent;
use crate::agent::libushangshu::LibuShangshuAgent;
use crate::agent::liburshangshu::LibuRShangshuAgent;
use crate::agent::menxiashizhong::MenxiaShizhongAgent;
use crate::agent::neige::NeigeAgent;
use crate::agent::shangshuling::ShangshulingAgent;
use crate::agent::xingbushangshu::XingbuShangshuAgent;
use crate::agent::zhisi::ZhisiAgent;
use crate::api::client::AnthropicClient;
use crate::commands::project::AppState;
use crate::commands::settings::AppConfig;
use crate::models::chat::ChatMessage;
use crate::models::project::ProjectSnapshot;
use crate::models::role::Role;

// ── Build agents (used by actor system startup) ─────────────

fn build_agents(
    config: &AppConfig,
    cancel: Arc<AtomicBool>,
    cancel_map: Arc<std::sync::Mutex<HashMap<Role, Arc<AtomicBool>>>>,
) -> HashMap<Role, Box<dyn Agent>> {
    let mut agents: HashMap<Role, Box<dyn Agent>> = HashMap::new();

    let menxiashizhong_ep = config.for_role("menxiashizhong");
    agents.insert(Role::MenxiaShizhong, Box::new(
        MenxiaShizhongAgent::new(
            AnthropicClient::new(menxiashizhong_ep.api_key, menxiashizhong_ep.api_url),
            &menxiashizhong_ep.model,
            cancel.clone(),
        )
    ));

    let zhongshuling_ep = config.for_role("zhongshuling");
    agents.insert(Role::Zhongshuling, Box::new(
        ZhongshulingAgent::new(
            AnthropicClient::new(zhongshuling_ep.api_key, zhongshuling_ep.api_url),
            &zhongshuling_ep.model,
            cancel.clone(),
        )
    ));

    let neige_ep = config.for_role("neige");
    agents.insert(Role::Neige, Box::new(
        NeigeAgent::new(
            AnthropicClient::new(neige_ep.api_key, neige_ep.api_url),
            &neige_ep.model,
            cancel.clone(),
            Some(cancel_map),
        )
    ));

    let ministry_configs: Vec<(Role, &str)> = vec![
        (Role::LiBuShangshu, "libushangshu"),
        (Role::BingbuShangshu, "bingbushangshu"),
        (Role::GongbuShangshu, "gongbushangshu"),
        (Role::XingbuShangshu, "xingbushangshu"),
        (Role::LiBuRShangshu, "liburshangshu"),
        (Role::Zhisi, "zhisi"),
        (Role::Shangshuling, "shangshuling"),
    ];

    for (role, name) in ministry_configs {
        let ep = config.for_role(name);
        let client = AnthropicClient::new(ep.api_key, ep.api_url);
        let agent: Box<dyn Agent> = match role {
            Role::LiBuShangshu => Box::new(LibuShangshuAgent::new(client, &ep.model, cancel.clone())),
            Role::BingbuShangshu => Box::new(BingbuShangshuAgent::new(client, &ep.model, cancel.clone())),
            Role::GongbuShangshu => Box::new(GongbuShangshuAgent::new(client, &ep.model, cancel.clone())),
            Role::XingbuShangshu => Box::new(XingbuShangshuAgent::new(client, &ep.model, cancel.clone())),
            Role::LiBuRShangshu => Box::new(LibuRShangshuAgent::new(client, &ep.model, cancel.clone())),
            Role::Zhisi => Box::new(ZhisiAgent::new(client, &ep.model)),
            Role::Shangshuling => Box::new(ShangshulingAgent::new(client, &ep.model, cancel.clone())),
            _ => continue,
        };
        agents.insert(role, agent);
    }

    agents
}

// ── Actor system startup ──────────────────────────────────

/// Build the actor system: create all agents, spawn one actor
/// per role, return the ActorSystem with all channel senders.
async fn start_actor_system(
    config: &AppConfig,
    project_dir: &Path,
    working_dir: &Path,
    cancel: Arc<AtomicBool>,
    emperor_tx: mpsc::UnboundedSender<ChatMessage>,
    dept_log_tx: mpsc::UnboundedSender<DeptLogEntry>,
    plan_tx: mpsc::UnboundedSender<serde_json::Value>,
    milestone_tx: mpsc::UnboundedSender<String>,
) -> ActorSystem {
    // Per-agent cancel flags — 内阁 gets access to cancel any agent
    let cancel_map: Arc<std::sync::Mutex<HashMap<Role, Arc<AtomicBool>>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));

    let agents = build_agents(config, cancel.clone(), cancel_map.clone());
    let mut senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>> = HashMap::new();
    let mut contexts: Vec<(Role, Box<dyn Agent>, mpsc::UnboundedReceiver<ActorMessage>)> = Vec::new();

    for (role, agent) in agents {
        let (tx, rx) = mpsc::unbounded_channel();
        senders.insert(role, tx);
        contexts.push((role, agent, rx));
    }

    let all_senders = senders.clone();
    let shared_context: Arc<std::sync::Mutex<HashMap<Role, String>>> = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let talk_history: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    for (role, agent, rx) in contexts {
        let mut peers: HashMap<Role, mpsc::UnboundedSender<ActorMessage>> = HashMap::new();
        for (other_role, tx) in &all_senders {
            if *other_role != role {
                peers.insert(*other_role, tx.clone());
            }
        }

        // Each agent gets its own cancel flag
        let agent_cancel = Arc::new(AtomicBool::new(false));
        cancel_map.lock().unwrap().insert(role, agent_cancel.clone());

        let logger = crate::logging::logger::Logger::new(
            &working_dir.join(".shuji"),
        );

        let is_neige = role == Role::Neige;
        let ctx = ActorContext {
            role,
            agent,
            rx,
            peers,
            emperor_tx: emperor_tx.clone(),
            dept_log_tx: dept_log_tx.clone(),
            plan_tx: plan_tx.clone(),
            plan: Arc::new(std::sync::Mutex::new(Vec::new())),
            milestone_tx: milestone_tx.clone(),
            project_dir: project_dir.to_path_buf(),
            working_dir: working_dir.to_path_buf(),
            cancel: agent_cancel,
            cancel_map: if is_neige { Some(cancel_map.clone()) } else { None },
            logger,
            shared_context: shared_context.clone(),
            talk_history: talk_history.clone(),
            current_skill: Arc::new(std::sync::Mutex::new(None)),
        };

        tokio::spawn(crate::actor::run_actor(ctx));
    }

    ActorSystem {
        senders: all_senders,
        emperor_tx,
        dept_log_tx,
        cancel_map,
        cancel,
    }
}

// ── Tauri commands ────────────────────────────────────────

/// Send a message to the 内阁 actor.  The actor system is
/// lazily created on the first message.
#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    message: String,
) -> Result<String, String> {
    let config = crate::commands::settings::get_config().await.map_err(|e| e.to_string())?;

    let p_working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt.as_ref().ok_or("没有加载项目")?;
        p.working_dir.clone()
    };

    // Lazy init: create actor system on first message
    {
        let mut sys_lock = state.actor_system.lock().await;
        if sys_lock.is_none() {
            let (emperor_tx, mut emperor_rx) = mpsc::unbounded_channel::<ChatMessage>();
            let (dept_log_tx, mut dept_log_rx) = mpsc::unbounded_channel::<DeptLogEntry>();
            let (plan_tx, mut plan_rx) = mpsc::unbounded_channel::<serde_json::Value>();
            let (milestone_tx, mut milestone_rx): (mpsc::UnboundedSender<String>, _) = mpsc::unbounded_channel();
            let app_handle = app.clone();

            // Buffer references for missed-event recovery across navigation
            let chat_hist = state.chat_history.clone();
            let dept_log_hist = state.dept_log_history.clone();

            // Persist chat messages to `.shuji/chat.jsonl`
            let chat_persist_dir = p_working_dir.clone();

            // Forward actor output to frontend + buffer + persist
            tokio::spawn(async move {
                while let Some(msg) = emperor_rx.recv().await {
                    let _ = app_handle.emit("chat-message", &msg);
                    let mut hist = chat_hist.lock().await;
                    hist.push(msg.clone());
                    // Append to persistent chat log
                    let log_dir = std::path::Path::new(&chat_persist_dir).join(".shuji");
                    let _ = std::fs::create_dir_all(&log_dir);
                    let chat_path = log_dir.join("chat.jsonl");
                    if let Ok(json) = serde_json::to_string(&msg) {
                        use std::io::Write;
                        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&chat_path) {
                            let _ = writeln!(f, "{}", json);
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
                    // Persist to .shuji/dept-log.jsonl
                    let log_dir = std::path::Path::new(&dept_log_dir).join(".shuji");
                    let _ = std::fs::create_dir_all(&log_dir);
                    let log_path = log_dir.join("dept-log.jsonl");
                    if let Ok(json) = serde_json::to_string(&entry) {
                        use std::io::Write;
                        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
                            let _ = writeln!(f, "{}", json);
                        }
                    }
                }
            });

            // Forward plan updates to frontend (工部 batch progress)
            let app_plan = app.clone();
            tokio::spawn(async move {
                while let Some(plan_json) = plan_rx.recv().await {
                    // Emit as raw JSON string; frontend parses it
                    let _ = app_plan.emit("plan-update", &plan_json);
                }
            });

            // Update project state on milestones
            let app3 = app.clone();
            let wd = p_working_dir.clone();
            tokio::spawn(async move {
                let s = crate::storage::shuji_dir::ShujiDir::new(&wd);
                while let Some(milestone) = milestone_rx.recv().await {
                    // Read from AppState, modify, persist
                    let st = app3.state::<AppState>();
                    let mut p_opt = st.current_project.lock().await;
                    if let Some(ref mut p) = *p_opt {
                        p.append_talk(&milestone);
                        p.summary = milestone.chars().take(120).collect();
                    }
                    let snapshot = p_opt.clone();
                    drop(p_opt);

                    // Save to disk (silently ignore errors)
                    if let Some(ref project) = snapshot {
                        let _ = s.save_project(project).await;
                    }
                }
            });

            let system = start_actor_system(
                &config,
                Path::new(&p_working_dir),
                Path::new(&p_working_dir),
                state.cancel_flag.clone(),
                emperor_tx,
                dept_log_tx,
                plan_tx,
                milestone_tx,
            ).await;

            *sys_lock = Some(system);
        }
    }

    // Send message to 内阁 actor
    let sys_lock = state.actor_system.lock().await;
    let system = sys_lock.as_ref().ok_or("Actor 系统未初始化")?;
    system.send(&Role::Neige, ActorMessage::Task { content: message })?;

    Ok("已接收".to_string())
}

/// Independent discussion with Cabinet — does NOT modify project state.
#[tauri::command]
pub async fn discuss_with_cabinet(
    state: State<'_, AppState>,
    message: String,
) -> Result<ChatMessage, String> {
    let config = crate::commands::settings::get_config().await.map_err(|e| e.to_string())?;

    let (working_dir, project_context) = {
        let project_opt = state.current_project.lock().await;
        let p = match project_opt.as_ref() {
            Some(p) => p,
            None => return Err("没有加载项目".to_string()),
        };
        (p.working_dir.clone(), format!(
            r#"━━ 项目目标 ━━
{}

━━ 当前阶段 ━━
{}

━━ 项目里程碑 ━━
{}

━━ 对话记录(近期) ━━
{}"#,
            p.goal, p.summary, p.task, p.talk,
        ))
    };

    let ep = config.for_role("neige");
    let client = AnthropicClient::new(ep.api_key, ep.api_url);
    let neige = NeigeAgent::new(client, &ep.model, Arc::new(AtomicBool::new(false)), None);

    let input = AgentInput {
        role: Role::Neige,
        task_description: format!(
            "（以下为当前项目状态，供参考）\n{}\n\n━━ 皇帝与你讨论 ━━\n{}",
            project_context, message,
        ),
        context_messages: vec![],
        project_dir: std::path::PathBuf::from(&working_dir),
        working_dir: std::path::PathBuf::from(&working_dir),
        skill_prompts: vec![],
        current_skill: None,
    };

    let output = neige.execute(&input).await.map_err(|e| e.to_string())?;
    Ok(ChatMessage::new("内阁", &output.content))
}

/// Get current snapshot (for UI refresh).
#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<ProjectSnapshot, String> {
    let project_opt = state.current_project.lock().await;
    let project = project_opt.as_ref().ok_or("没有加载项目")?;
    Ok(project.snapshot())
}

// ── Document and log file commands ────────────────────────

#[tauri::command]
pub async fn read_document(
    state: State<'_, AppState>,
    subdir: String,
    filename: String,
) -> Result<Option<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt.as_ref().ok_or("没有加载项目")?.working_dir.clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir.read_document(&subdir, &filename).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_documents(
    state: State<'_, AppState>,
    subdir: String,
) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt.as_ref().ok_or("没有加载项目")?.working_dir.clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir.list_documents(&subdir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_log_files(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt.as_ref().ok_or("没有加载项目")?.working_dir.clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir.list_log_files().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_log_file(state: State<'_, AppState>, filename: String) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt.as_ref().ok_or("没有加载项目")?.working_dir.clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir.read_log_file(&filename).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recent_dirs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let dir = state.current_dir.lock().await;
    Ok(match dir.as_ref() {
        Some(d) => vec![d.clone()],
        None => vec![],
    })
}

/// Get token usage statistics for all roles (dashboard data).
#[tauri::command]
pub async fn get_token_stats() -> Result<std::collections::HashMap<String, std::collections::HashMap<String, crate::token_tracker::TokenUsage>>, String> {
    Ok(crate::token_tracker::snapshot_grouped())
}

/// Get buffered chat message history (for re-sync after page navigation).
#[tauri::command]
pub async fn get_chat_history(state: State<'_, AppState>) -> Result<Vec<ChatMessage>, String> {
    let hist = state.chat_history.lock().await;
    Ok(hist.clone())
}

/// Get buffered department log history (for re-sync after page navigation).
#[tauri::command]
pub async fn get_dept_logs(state: State<'_, AppState>) -> Result<Vec<crate::actor::DeptLogEntry>, String> {
    let hist = state.dept_log_history.lock().await;
    Ok(hist.clone())
}

/// Cancel all running actor processing.  Sets the shared AtomicBool flag;
/// each actor's AgentController checks it between tool iterations.
#[tauri::command]
pub async fn cancel_processing(state: State<'_, AppState>) -> Result<(), String> {
    state.cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
    log_console!("[commands] cancel_processing: flag set, actors will stop at next check point");
    Ok(())
}
