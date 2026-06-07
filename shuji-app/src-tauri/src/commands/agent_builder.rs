use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::actor::{ActorContext, ActorMessage, ActorSystem, DeptLogEntry, FastMessage};
use crate::agent::bingbushangshu::BingbuShangshuAgent;
use crate::agent::gongbushangshu::GongbuShangshuAgent;
use crate::agent::liburshangshu::LibuRShangshuAgent;
use crate::agent::libushangshu::LibuShangshuAgent;
use crate::agent::menxiashizhong::MenxiaShizhongAgent;
use crate::agent::neige::NeigeAgent;
use crate::agent::shangshuling::ShangshulingAgent;
use crate::agent::xingbushangshu::XingbuShangshuAgent;
use crate::agent::zhongshuling::ZhongshulingAgent;
use crate::agent::r#trait::Agent;
use crate::api::client::AnthropicClient;
use crate::commands::settings::AppConfig;
use crate::models::chat::ChatMessage;
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


pub(crate) fn build_agents(
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

pub(crate) async fn start_actor_system(
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
    let cancel_map: crate::CancelMap =
        Arc::new(std::sync::Mutex::new(HashMap::new()));

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
    let failure_retries: Arc<std::sync::Mutex<HashMap<Role, u32>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    let talk_history: Arc<std::sync::Mutex<Vec<String>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

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
            failure_retries: failure_retries.clone(),
            talk_history: talk_history.clone(),
            current_skill: Arc::new(std::sync::Mutex::new(None)),
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
    }
}
