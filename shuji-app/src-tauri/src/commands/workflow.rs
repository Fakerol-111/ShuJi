use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{Emitter, Manager, State};
use tokio::sync::mpsc;

use crate::actor::{ActorContext, ActorMessage, ActorSystem, DeptLogEntry, FastMessage};
use crate::agent::bingbushangshu::BingbuShangshuAgent;
use crate::agent::gongbushangshu::GongbuShangshuAgent;
use crate::agent::liburshangshu::LibuRShangshuAgent;
use crate::agent::libushangshu::LibuShangshuAgent;
use crate::agent::menxiashizhong::MenxiaShizhongAgent;
use crate::agent::neige::NeigeAgent;
use crate::agent::r#trait::{Agent, AgentInput};
use crate::agent::shangshuling::ShangshulingAgent;
use crate::agent::xingbushangshu::XingbuShangshuAgent;
use crate::agent::zhongshuling::ZhongshulingAgent;
use crate::api::client::AnthropicClient;
use crate::api::control::RouteMsgType;
use crate::api::session::PersistedContext;
use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::commands::settings::{AppConfig, ContextWindowConfig};
use crate::models::chat::ChatMessage;
use crate::models::project::ProjectSnapshot;
use crate::models::role::Role;

/// Per-role context usage statistics exposed to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextStats {
    /// Number of conversation messages in current context.
    pub message_count: usize,
    /// Total tokens across context_messages (cl100k).
    pub token_count: usize,
    /// Compression threshold in tokens.
    pub token_threshold: usize,
    /// Whether context has been compacted (contains `[对话摘要]` summary).
    pub compressed: bool,
    /// Number of active skill prompts.
    pub skill_count: usize,
}

// ── Build agents (used by actor system startup) ─────────────

fn build_agents(
    config: &AppConfig,
    cancel: Arc<AtomicBool>,
    cancel_map: crate::CancelMap,
    fast_txs: crate::FastTxMap,
) -> HashMap<Role, Box<dyn Agent>> {
    let mut agents: HashMap<Role, Box<dyn Agent>> = HashMap::new();

    let menxiashizhong_ep = config.for_role("menxiashizhong");
    agents.insert(
        Role::MenxiaShizhong,
        Box::new(MenxiaShizhongAgent::new(
            AnthropicClient::new(menxiashizhong_ep.api_key, menxiashizhong_ep.api_url),
            &menxiashizhong_ep.model,
            cancel.clone(),
        )),
    );

    let zhongshuling_ep = config.for_role("zhongshuling");
    agents.insert(
        Role::Zhongshuling,
        Box::new(ZhongshulingAgent::new(
            AnthropicClient::new(zhongshuling_ep.api_key, zhongshuling_ep.api_url),
            &zhongshuling_ep.model,
            cancel.clone(),
        )),
    );

    let neige_ep = config.for_role("neige");
    agents.insert(
        Role::Neige,
        Box::new(NeigeAgent::new(
            AnthropicClient::new(neige_ep.api_key, neige_ep.api_url),
            &neige_ep.model,
            cancel.clone(),
            Some(cancel_map),
            Some(fast_txs),
        )),
    );

    let ministry_configs: Vec<(Role, &str)> = vec![
        (Role::LiBuShangshu, "libushangshu"),
        (Role::BingbuShangshu, "bingbushangshu"),
        (Role::GongbuShangshu, "gongbushangshu"),
        (Role::XingbuShangshu, "xingbushangshu"),
        (Role::LiBuRShangshu, "liburshangshu"),
        (Role::Shangshuling, "shangshuling"),
    ];

    for (role, name) in ministry_configs {
        let ep = config.for_role(name);
        let client = AnthropicClient::new(ep.api_key, ep.api_url);
        let agent: Box<dyn Agent> = match role {
            Role::LiBuShangshu => {
                Box::new(LibuShangshuAgent::new(client, &ep.model, cancel.clone()))
            }
            Role::BingbuShangshu => {
                Box::new(BingbuShangshuAgent::new(client, &ep.model, cancel.clone()))
            }
            Role::GongbuShangshu => {
                Box::new(GongbuShangshuAgent::new(client, &ep.model, cancel.clone()))
            }
            Role::XingbuShangshu => {
                Box::new(XingbuShangshuAgent::new(client, &ep.model, cancel.clone()))
            }
            Role::LiBuRShangshu => {
                Box::new(LibuRShangshuAgent::new(client, &ep.model, cancel.clone()))
            }
            Role::Shangshuling => {
                Box::new(ShangshulingAgent::new(client, &ep.model, cancel.clone()))
            }
            _ => continue,
        };
        agents.insert(role, agent);
    }

    agents
}

// ── Actor system startup ──────────────────────────────────

/// Build the actor system: create all agents, spawn one actor
/// per role, return the ActorSystem with all channel senders.
#[allow(clippy::too_many_arguments)]
async fn start_actor_system(
    config: &AppConfig,
    runtime_config: Arc<crate::config::RuntimeConfig>,
    project_dir: &Path,
    working_dir: &Path,
    cancel: Arc<AtomicBool>,
    emperor_tx: mpsc::UnboundedSender<ChatMessage>,
    dept_log_tx: mpsc::UnboundedSender<DeptLogEntry>,
    plan_tx: mpsc::UnboundedSender<serde_json::Value>,
    milestone_tx: mpsc::UnboundedSender<String>,
) -> ActorSystem {
    // Per-agent cancel flags — 内阁 gets access to cancel any agent
    let cancel_map: crate::CancelMap = Arc::new(std::sync::Mutex::new(HashMap::new()));

    // Create fast mailboxes for all roles before building agents,
    // so NeigeAgent can reference fast_txs from its constructor.
    let all_roles = vec![
        Role::Neige,
        Role::Zhongshuling,
        Role::MenxiaShizhong,
        Role::Shangshuling,
        Role::LiBuShangshu,
        Role::BingbuShangshu,
        Role::GongbuShangshu,
        Role::XingbuShangshu,
        Role::LiBuRShangshu,
    ];
    let mut fast_txs: HashMap<Role, mpsc::UnboundedSender<FastMessage>> = HashMap::new();
    let mut fast_rxs: HashMap<Role, tokio::sync::Mutex<mpsc::UnboundedReceiver<FastMessage>>> =
        HashMap::new();
    for role in &all_roles {
        let (fast_tx, fast_rx) = mpsc::unbounded_channel();
        fast_txs.insert(*role, fast_tx);
        fast_rxs.insert(*role, tokio::sync::Mutex::new(fast_rx));
    }
    let fast_txs = Arc::new(fast_txs);

    let agents = build_agents(config, cancel.clone(), cancel_map.clone(), fast_txs.clone());
    let mut senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>> = HashMap::new();
    let mut contexts: Vec<(Role, Box<dyn Agent>, mpsc::UnboundedReceiver<ActorMessage>)> =
        Vec::new();

    for (role, mut agent) in agents {
        // Create per-actor cancel flag and wire it into the agent so
        // AgentController.run() checks the same flag that Interrupt
        // messages and cancel_agent tool set.
        let actor_flag = Arc::new(AtomicBool::new(false));
        agent.set_interrupt_flag(actor_flag.clone());
        cancel_map.lock().unwrap().insert(role, actor_flag);

        let (tx, rx) = mpsc::unbounded_channel();
        senders.insert(role, tx);
        contexts.push((role, agent, rx));
    }

    let all_senders = senders.clone();
    let shared_context: Arc<std::sync::Mutex<HashMap<Role, String>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    let failure_retries: Arc<std::sync::Mutex<HashMap<Role, u32>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    let talk_history: Arc<std::sync::Mutex<Vec<String>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    // ── 文移图初始化 ──
    let workflow_graph = Arc::new(tokio::sync::Mutex::new(
        crate::workflow::WorkflowGraph::load_or_new(working_dir).await,
    ));

    for (role, agent, rx) in contexts {
        let mut peers: HashMap<Role, mpsc::UnboundedSender<ActorMessage>> = HashMap::new();
        for (other_role, tx) in &all_senders {
            if *other_role != role {
                peers.insert(*other_role, tx.clone());
            }
        }

        // Reuse the per-actor cancel flag created in the agents loop above
        let actor_flag = cancel_map.lock().unwrap().get(&role).unwrap().clone();

        let logger = crate::logging::logger::Logger::new(&working_dir.join(".shuji"));

        let is_neige = role == Role::Neige;
        let fast_rx = fast_rxs.remove(&role).unwrap();
        let ctx = ActorContext {
            role,
            agent,
            rx,
            fast_rx,
            peers,
            emperor_tx: emperor_tx.clone(),
            dept_log_tx: dept_log_tx.clone(),
            plan_tx: plan_tx.clone(),
            plan: Arc::new(std::sync::Mutex::new(Vec::new())),
            milestone_tx: milestone_tx.clone(),
            project_dir: project_dir.to_path_buf(),
            working_dir: working_dir.to_path_buf(),
            cancel: actor_flag,
            cancel_map: if is_neige {
                Some(cancel_map.clone())
            } else {
                None
            },
            logger,
            shared_context: shared_context.clone(),
            failure_retries: failure_retries.clone(),
            talk_history: talk_history.clone(),
            current_skill: Arc::new(std::sync::Mutex::new(None)),
            workflow_graph: Some(workflow_graph.clone()),
            runtime_config: runtime_config.clone(),
        };

        tokio::spawn(crate::actor::run_actor(ctx));
    }

    ActorSystem {
        senders: all_senders,
        fast_txs: (*fast_txs).clone(),
        emperor_tx,
        dept_log_tx,
        cancel_map,
        cancel,
        workflow_graph: workflow_graph.clone(),
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
    // Start a new round metrics cycle
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
            let (emperor_tx, mut emperor_rx) = mpsc::unbounded_channel::<ChatMessage>();
            let (dept_log_tx, mut dept_log_rx) = mpsc::unbounded_channel::<DeptLogEntry>();
            let (plan_tx, mut plan_rx) = mpsc::unbounded_channel::<serde_json::Value>();
            let (milestone_tx, mut milestone_rx): (mpsc::UnboundedSender<String>, _) =
                mpsc::unbounded_channel();
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
                    // Persist to .shuji/dept-log.jsonl
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

                    // Emit project-update event for frontend
                    if let Some(ref project) = snapshot {
                        let _ = app3.emit("project-update", project);
                    }

                    // Audit log milestone
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

    // Send message to 内阁 actor
    let sys_lock = state.actor_system.lock().await;
    let system = sys_lock
        .as_ref()
        .ok_or_else(|| friendly_error("Actor 系统未初始化"))?;

    // ── 归档上次命令的文移图，开始新会话 ──
    // 使用 actor system 共享的 workflow_graph，而非从磁盘独立加载，避免
    // 与 actor 内 forward_route 写入的内存图不同步。
    {
        let wd = Path::new(&p_working_dir);
        let mut graph = system.workflow_graph.lock().await;
        let label: String = message.chars().take(60).collect();
        graph.archive_and_new(wd, label.trim()).await;
    }

    // Interrupt any currently active non-cabinet department via fast mailbox,
    // so the emperor's new instruction is handled promptly instead of waiting
    // for the current tool loop to finish.
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
        fast_cancel: Arc::new(AtomicBool::new(false)),
    };

    let output = neige.execute(&input).await.map_err(friendly_error)?;
    Ok(ChatMessage::new("内阁", &output.content))
}

/// Get current snapshot (for UI refresh).
#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<ProjectSnapshot, String> {
    let project_opt = state.current_project.lock().await;
    let project = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?;
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
    let working_dir = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?
        .working_dir
        .clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir
        .read_document(&subdir, &filename)
        .await
        .map_err(friendly_error)
}

#[tauri::command]
pub async fn list_documents(
    state: State<'_, AppState>,
    subdir: String,
) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?
        .working_dir
        .clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir
        .list_documents(&subdir)
        .await
        .map_err(friendly_error)
}

#[tauri::command]
pub async fn list_log_files(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?
        .working_dir
        .clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir.list_log_files().await.map_err(friendly_error)
}

#[tauri::command]
pub async fn read_log_file(
    state: State<'_, AppState>,
    filename: String,
) -> Result<Vec<String>, String> {
    let project_opt = state.current_project.lock().await;
    let working_dir = project_opt
        .as_ref()
        .ok_or_else(|| friendly_error("没有加载项目"))?
        .working_dir
        .clone();
    drop(project_opt);
    let shuji_dir = crate::storage::shuji_dir::ShujiDir::new(&working_dir);
    shuji_dir
        .read_log_file(&filename)
        .await
        .map_err(friendly_error)
}

/// Persist and retrieve recent project directories.
/// Stored at `~/.shuji/recent_dirs.json` with cap of 20 entries.
fn recent_dirs_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".shuji")
        .join("recent_dirs.json")
}

fn load_recent_dirs() -> Vec<String> {
    let path = recent_dirs_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_recent_dirs(dirs: &[String]) {
    let path = recent_dirs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(dirs) {
        let _ = std::fs::write(&path, json);
    }
}

pub fn add_recent_dir(working_dir: &str) {
    let mut dirs = load_recent_dirs();
    // Remove existing entry if present (move to front)
    dirs.retain(|d| d != working_dir);
    dirs.insert(0, working_dir.to_string());
    // Cap at 20
    dirs.truncate(20);
    save_recent_dirs(&dirs);
}

#[tauri::command]
pub async fn get_recent_dirs() -> Result<Vec<String>, String> {
    Ok(load_recent_dirs())
}

/// Get token usage statistics for all roles (dashboard data).
#[tauri::command]
pub async fn get_token_stats() -> Result<
    std::collections::HashMap<
        String,
        std::collections::HashMap<String, crate::token_tracker::TokenUsage>,
    >,
    String,
> {
    Ok(crate::token_tracker::snapshot_grouped())
}

/// Get per-role context usage statistics.
#[tauri::command]
pub async fn get_context_stats(
    state: State<'_, AppState>,
) -> Result<HashMap<String, ContextStats>, String> {
    let dir = match state.current_dir.lock().await.as_ref() {
        Some(d) => d.clone(),
        None => return Ok(HashMap::new()),
    };
    let config = &state.runtime_config;

    // Load per-role context window overrides
    let role_overrides: HashMap<String, crate::config::RoleContextConfig> = {
        let path = std::path::Path::new(&dir).join("context_config.json");
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => match serde_json::from_str::<ContextWindowConfig>(&content) {
                Ok(cfg) => cfg.roles,
                Err(_) => HashMap::new(),
            },
            Err(_) => HashMap::new(),
        }
    };

    let ctx_dir = std::path::Path::new(&dir).join(".shuji/context");
    let mut entries = match tokio::fs::read_dir(&ctx_dir).await {
        Ok(e) => e,
        Err(_) => return Ok(HashMap::new()),
    };

    let mut result = HashMap::new();

    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let role = match path.file_stem().and_then(|s| s.to_str()) {
            Some(r) if !r.is_empty() => r.to_string(),
            _ => continue,
        };

        let data = match tokio::fs::read_to_string(&path).await {
            Ok(d) => d,
            _ => continue,
        };
        let ctx: PersistedContext = match serde_json::from_str(&data) {
            Ok(c) => c,
            _ => continue,
        };

        let token_count = crate::api::token_count::count_messages_tokens(&ctx.context_messages);

        // Resolve per-role thresholds
        let thresholds = config.resolve_compact_thresholds(&role, role_overrides.get(&role));

        result.insert(
            role,
            ContextStats {
                message_count: ctx.context_messages.len(),
                token_count,
                token_threshold: thresholds.token_threshold,
                compressed: ctx.context_messages.iter().any(|m| {
                    m["role"].as_str() == Some("system")
                        && m["content"]
                            .as_str()
                            .is_some_and(|c| c.starts_with("[对话摘要]"))
                }),
                skill_count: crate::api::session::count_skill_messages(&ctx.context_messages),
            },
        );
    }

    Ok(result)
}

/// Manually trigger context compaction for a specific role.
///
/// Reads the persisted context from `.shuji/context/{role}.json`, runs the
/// iterative compaction loop, and saves the result back to disk.
/// Works independently of the actor system — safe to call while actors run.
#[tauri::command]
pub async fn compact_context(state: State<'_, AppState>, role: String) -> Result<String, String> {
    let dir = state
        .current_dir
        .lock()
        .await
        .clone()
        .ok_or_else(|| friendly_error("没有加载项目"))?;
    let working_dir = std::path::Path::new(&dir);

    // ── Concurrency guard: agent is actively executing ──
    if crate::round_metrics::is_active(&role) {
        return Err(friendly_error(format!(
            "角色 {} 正在执行中，请等待完成后再压缩",
            role
        )));
    }

    // ── Concurrency guard: already being compacted ──
    {
        let mut compacting = state.compacting_roles.lock().await;
        if !compacting.insert(role.clone()) {
            return Err(friendly_error(format!(
                "角色 {} 正在被压缩中，请勿重复操作",
                role
            )));
        }
    }

    // Run compaction and always release the guard when done.
    let result = compact_impl(working_dir, &role, &state).await;
    state.compacting_roles.lock().await.remove(&role);
    result
}

async fn compact_impl(
    working_dir: &Path,
    role: &str,
    state: &State<'_, AppState>,
) -> Result<String, String> {
    // Load context window overrides from context_config.json
    let role_overrides: HashMap<String, crate::config::RoleContextConfig> = {
        let path = working_dir.join("context_config.json");
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str::<ContextWindowConfig>(&content)
                .ok()
                .map(|c| c.roles)
                .unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    };

    // Load persisted context
    let mut ctx = PersistedContext::load_from(working_dir, role)
        .await
        .ok_or_else(|| friendly_error(format!("角色 {} 没有找到上下文文件", role)))?;

    // Resolve thresholds
    let thresholds = state
        .runtime_config
        .resolve_compact_thresholds(role, role_overrides.get(role));

    // Pre-check: skip if neither threshold is exceeded (no API config needed)
    let total_tokens = crate::api::token_count::count_messages_tokens(&ctx.context_messages);

    // Force context compaction threshold to 0 so it always compresses.
    let force_thresholds = crate::config::CompactThresholds {
        token_threshold: 0,
        keep_recent_count: thresholds.keep_recent_count,
        mid_run_compact: false,
    };

    // Load API config for this role
    let config = crate::commands::settings::get_config()
        .await
        .map_err(friendly_error)?;
    let ep = config.for_role(role);

    if ep.api_key.is_empty() {
        return Err(friendly_error(format!(
            "角色 {} 未配置 API 密钥，请在设置中配置",
            role
        )));
    }
    if ep.api_url.is_empty() {
        return Err(friendly_error(format!("角色 {} 未配置 API URL", role)));
    }

    let client = AnthropicClient::new(ep.api_key, ep.api_url);
    let model = ep.model;
    let is_cabinet = role == "neige";

    log_console!(
        "[compact:manual] starting compaction for {} (cabinet={}, tokens={})",
        role,
        is_cabinet,
        total_tokens,
    );

    let performed = crate::api::compact::run_compaction_loop(
        &client,
        &model,
        &mut ctx,
        &force_thresholds,
        is_cabinet,
        working_dir,
        role,
    )
    .await;

    if performed {
        log_console!("[compact:manual] compaction completed for {}", role);
        Ok(format!(
            "压缩完成（角色: {}，原始 {} tokens → 摘要 + {} 条最近消息）",
            role, total_tokens, thresholds.keep_recent_count,
        ))
    } else {
        // With force_thresholds=0, "not performed" means summarization failed.
        Err(friendly_error(format!(
            "角色 {} 压缩失败——API 调用未返回有效摘要。请检查 API 配置后重试",
            role
        )))
    }
}

/// Get buffered chat message history (for re-sync after page navigation).
#[tauri::command]
pub async fn get_chat_history(state: State<'_, AppState>) -> Result<Vec<ChatMessage>, String> {
    let hist = state.chat_history.lock().await;
    Ok(hist.clone())
}

/// Get buffered department log history (for re-sync after page navigation).
#[tauri::command]
pub async fn get_dept_logs(
    state: State<'_, AppState>,
) -> Result<Vec<crate::actor::DeptLogEntry>, String> {
    let hist = state.dept_log_history.lock().await;
    Ok(hist.clone())
}

/// Cancel all running actor processing.  Sets all per-actor cancel flags
/// (checked by AgentController.run() between tool iterations) and sends
/// Interrupt messages so idle actors don't start new work.
#[tauri::command]
pub async fn cancel_processing(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(sys) = state.actor_system.lock().await.as_ref() {
        // Set all per-actor cancel flags
        if let Ok(map) = sys.cancel_map.lock() {
            for flag in map.values() {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            log_console!("[commands] cancel_processing: all per-actor flags set");
        }

        // Send fast interrupt to all actors for immediate tool-level preemption
        for tx in sys.fast_txs.values() {
            let _ = tx.send(FastMessage::Interrupt);
        }
        log_console!("[commands] cancel_processing: FastMessage::Interrupt sent to all actors");

        // Wake idle actors so they don't start new work
        for tx in sys.senders.values() {
            let _ = tx.send(crate::actor::ActorMessage::interrupt());
        }
        log_console!("[commands] cancel_processing: Interrupt sent to all actors");
    }

    Ok(())
}

/// Get the list of document IDs pending emperor approval (朱批).
#[tauri::command]
pub async fn get_pending_approvals(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?
            .working_dir
            .clone()
    };
    let path = std::path::Path::new(&working_dir).join(".shuji/pending_approvals.json");
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => serde_json::from_str(&content).map_err(|e| e.to_string()),
        Err(_) => Ok(vec![]),
    }
}

/// Get the current round metrics (live workflow state).
/// Doesn't need AppState — reads from the global static.
#[tauri::command]
pub async fn get_round_metrics() -> Result<Option<crate::round_metrics::RoundMetricState>, String> {
    Ok(crate::round_metrics::snapshot())
}

/// Get the list of currently active (executing) departments.
#[tauri::command]
pub fn get_active_roles() -> Vec<String> {
    crate::round_metrics::get_active_roles()
}

/// Get the document lineage tree for a given doc ID.
#[tauri::command]
pub async fn get_document_lineage(
    state: State<'_, AppState>,
    doc_id: String,
) -> Result<Option<crate::audit::LineageNode>, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    Ok(crate::audit::build_lineage(std::path::Path::new(&working_dir), &doc_id).await)
}

/// Get the aggregated audit timeline.
#[tauri::command]
pub async fn get_audit_timeline(
    state: State<'_, AppState>,
) -> Result<crate::audit::TimelineData, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    Ok(crate::audit::build_timeline(std::path::Path::new(&working_dir)).await)
}

/// Generate a delivery report for the current project.
#[tauri::command]
pub async fn generate_delivery_report(state: State<'_, AppState>) -> Result<String, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    Ok(crate::audit::generate_report(std::path::Path::new(&working_dir)).await)
}

/// List available diff files for a document.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DocDiffFile {
    pub filename: String,
    pub event: String,
    pub ts: String,
}

#[tauri::command]
pub async fn get_document_diffs(
    state: State<'_, AppState>,
    doc_id: String,
) -> Result<Vec<DocDiffFile>, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    let diff_dir = std::path::Path::new(&working_dir)
        .join(".shuji")
        .join("audit")
        .join("diffs");
    let mut diffs = Vec::new();

    if let Ok(mut rd) = tokio::fs::read_dir(&diff_dir).await {
        let mut entries = Vec::new();
        while let Ok(Some(entry)) = rd.next_entry().await {
            entries.push(entry);
        }
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&format!("{}_", doc_id)) {
                let stripped = name.strip_suffix(".patch").unwrap_or(&name);
                let parts: Vec<&str> = stripped.splitn(3, '_').collect();
                let event = if parts.len() > 1 {
                    parts[1].to_string()
                } else {
                    String::new()
                };
                let ts = if parts.len() > 2 {
                    parts[2].to_string()
                } else {
                    String::new()
                };
                diffs.push(DocDiffFile {
                    filename: name,
                    event,
                    ts,
                });
            }
        }
    }
    diffs.sort_by(|a, b| b.ts.cmp(&a.ts));
    Ok(diffs)
}

/// Read the content of a specific diff file.
#[tauri::command]
pub async fn read_document_diff(
    state: State<'_, AppState>,
    filename: String,
) -> Result<String, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    let path = std::path::Path::new(&working_dir)
        .join(".shuji")
        .join("audit")
        .join("diffs")
        .join(&filename);
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取 diff 失败: {}", e))
}

/// Set a document's approval status (approved/rejected).
/// Frontend calls this from the DocPreview "待陛下朱批" banner.
#[tauri::command]
pub async fn set_document_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
    emperor_note: Option<String>,
) -> Result<String, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };

    let mut args = serde_json::json!({
        "id": id,
        "status": status,
    });
    if let Some(note) = emperor_note {
        args["emperor_note"] = serde_json::Value::String(note);
    }

    let result =
        crate::tool::documents::tool_set_document_status(std::path::Path::new(&working_dir), &args)
            .await;

    let v: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| "解析结果失败".to_string())?;
    if v["ok"].as_bool().unwrap_or(false) {
        Ok(v["message"].as_str().unwrap_or("ok").to_string())
    } else {
        Err(v["message"].as_str().unwrap_or("未知错误").to_string())
    }
}

// ── Workflow state command ────────────────────────────────

/// Read the current workflow state (profile id, governance, stage, execution chain).
#[tauri::command]
pub async fn get_workflow_state(
    state: State<'_, AppState>,
) -> Result<Option<crate::workflow::WorkflowState>, String> {
    let dir = {
        let d = state.current_dir.lock().await;
        d.clone().ok_or("没有打开的项目")?
    };
    Ok(crate::workflow::WorkflowState::load_from(std::path::Path::new(&dir)).await)
}

// ── 文移图 ──────────────────────────────────────────────

#[tauri::command]
pub async fn get_workflow_graph(
    state: State<'_, AppState>,
) -> Result<Option<crate::workflow::WorkflowGraph>, String> {
    let dir = {
        let d = state.current_dir.lock().await;
        d.clone().ok_or("没有打开的项目")?
    };
    Ok(crate::workflow::WorkflowGraph::load_from(std::path::Path::new(&dir)).await)
}

#[tauri::command]
pub async fn list_workflow_archives(
    state: State<'_, AppState>,
) -> Result<Vec<Vec<String>>, String> {
    let dir = {
        let d = state.current_dir.lock().await;
        d.clone().ok_or("没有打开的项目")?
    };
    let archives = crate::workflow::WorkflowGraph::list_archives(std::path::Path::new(&dir)).await;
    // Return as Vec<[filename, label]> for frontend
    Ok(archives.into_iter().map(|(f, l)| vec![f, l]).collect())
}

#[tauri::command]
pub async fn load_workflow_archive(
    state: State<'_, AppState>,
    filename: String,
) -> Result<Option<crate::workflow::WorkflowGraph>, String> {
    let dir = {
        let d = state.current_dir.lock().await;
        d.clone().ok_or("没有打开的项目")?
    };
    Ok(crate::workflow::WorkflowGraph::load_archive(std::path::Path::new(&dir), &filename).await)
}

// ── Traceability commands ───────────────────────────────────

#[tauri::command]
pub async fn trace_document(
    state: State<'_, AppState>,
    doc_id: String,
) -> Result<crate::audit::TraceResult, String> {
    let working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("没有加载项目"))?;
        p.working_dir.clone()
    };
    Ok(crate::audit::trace_document(std::path::Path::new(&working_dir), &doc_id).await)
}
