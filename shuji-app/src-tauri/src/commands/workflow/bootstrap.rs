use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::actor::{ActorContext, ActorMessage, DeptLogEntry, FastMessage};
use crate::actor::ActorSystem;
use crate::agent::bingbushangshu::BingbuShangshuAgent;
use crate::agent::gongbushangshu::GongbuShangshuAgent;
use crate::agent::liburshangshu::LibuRShangshuAgent;
use crate::agent::libushangshu::LibuShangshuAgent;
use crate::agent::menxiashizhong::MenxiaShizhongAgent;
use crate::agent::neige::NeigeAgent;
use crate::agent::r#trait::Agent;
use crate::agent::shangshuling::ShangshulingAgent;
use crate::agent::xingbushangshu::XingbuShangshuAgent;
use crate::agent::zhongshuling::ZhongshulingAgent;
use crate::api::client::AnthropicClient;
use crate::commands::settings::AppConfig;
use crate::models::chat::ChatMessage;
use crate::models::role::Role;

/// Per-role context usage statistics exposed to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextStats {
    pub message_count: usize,
    pub token_count: usize,
    pub token_threshold: usize,
    pub compressed: bool,
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

    let shangshuling_ep = config.for_role("shangshuling");
    agents.insert(
        Role::Shangshuling,
        Box::new(ShangshulingAgent::new(
            AnthropicClient::new(shangshuling_ep.api_key, shangshuling_ep.api_url),
            &shangshuling_ep.model,
            cancel.clone(),
        )),
    );

    let libushangshu_ep = config.for_role("libushangshu");
    agents.insert(
        Role::LiBuShangshu,
        Box::new(LibuShangshuAgent::new(
            AnthropicClient::new(libushangshu_ep.api_key, libushangshu_ep.api_url),
            &libushangshu_ep.model,
            cancel.clone(),
        )),
    );

    let bingbushangshu_ep = config.for_role("bingbushangshu");
    agents.insert(
        Role::BingbuShangshu,
        Box::new(BingbuShangshuAgent::new(
            AnthropicClient::new(bingbushangshu_ep.api_key, bingbushangshu_ep.api_url),
            &bingbushangshu_ep.model,
            cancel.clone(),
        )),
    );

    let gongbushangshu_ep = config.for_role("gongbushangshu");
    agents.insert(
        Role::GongbuShangshu,
        Box::new(GongbuShangshuAgent::new(
            AnthropicClient::new(gongbushangshu_ep.api_key, gongbushangshu_ep.api_url),
            &gongbushangshu_ep.model,
            cancel.clone(),
        )),
    );

    let xingbushangshu_ep = config.for_role("xingbushangshu");
    agents.insert(
        Role::XingbuShangshu,
        Box::new(XingbuShangshuAgent::new(
            AnthropicClient::new(xingbushangshu_ep.api_key, xingbushangshu_ep.api_url),
            &xingbushangshu_ep.model,
            cancel.clone(),
        )),
    );

    let liburshangshu_ep = config.for_role("liburshangshu");
    agents.insert(
        Role::LiBuRShangshu,
        Box::new(LibuRShangshuAgent::new(
            AnthropicClient::new(liburshangshu_ep.api_key, liburshangshu_ep.api_url),
            &liburshangshu_ep.model,
            cancel.clone(),
        )),
    );

    let neige_ep = config.for_role("neige");
    agents.insert(
        Role::Neige,
        Box::new(NeigeAgent::new(
            AnthropicClient::new(neige_ep.api_key, neige_ep.api_url),
            &neige_ep.model,
            cancel,
            Some(cancel_map),
            Some(fast_txs),
        )),
    );

    agents
}

// ── Actor system startup ──────────────────────────────────

pub async fn start_actor_system(
    config: &AppConfig,
    runtime_config: Arc<crate::config::RuntimeConfig>,
    project_dir: &Path,
    working_dir: &Path,
    cancel: Arc<AtomicBool>,
    emperor_tx: mpsc::Sender<ChatMessage>,
    dept_log_tx: mpsc::Sender<DeptLogEntry>,
    plan_tx: mpsc::Sender<serde_json::Value>,
    milestone_tx: mpsc::Sender<String>,
) -> ActorSystem {
    let cancel_map: crate::CancelMap = Arc::new(std::sync::Mutex::new(HashMap::new()));
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
    // Fast mailbox with capacity 16 per actor — enough for Interrupt signals
    // without unbounded memory growth.
    let mut fast_txs: HashMap<Role, mpsc::Sender<FastMessage>> = HashMap::new();
    let mut fast_rxs: HashMap<Role, tokio::sync::Mutex<mpsc::Receiver<FastMessage>>> =
        HashMap::new();
    for role in &all_roles {
        let (fast_tx, fast_rx) = mpsc::channel(16);
        fast_txs.insert(*role, fast_tx);
        fast_rxs.insert(*role, tokio::sync::Mutex::new(fast_rx));
    }
    let fast_txs = Arc::new(fast_txs);

    let agents = build_agents(config, cancel.clone(), cancel_map.clone(), fast_txs.clone());
    let mut senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>> = HashMap::new();
    let mut contexts: Vec<(Role, Box<dyn Agent>, mpsc::UnboundedReceiver<ActorMessage>)> =
        Vec::new();

    for (role, mut agent) in agents {
        let actor_flag = Arc::new(AtomicBool::new(false));
        agent.set_interrupt_flag(actor_flag.clone());
        cancel_map.lock().unwrap().insert(role, actor_flag);

        // Actor main message channel — kept unbounded intentionally.
        // Switching to bounded would require async send() in all callers
        // (ActorSystem::send(), forward_route, fallback_to_dispatcher, etc.)
        // and introduce backpressure risk (actor blocked on send while holding
        // cancel flag or lock). Monitor for memory growth in long-running tasks;
        // revisit with a bounded channel + try_send pattern if evidence appears.
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
            cancel_map: if is_neige { Some(cancel_map.clone()) } else { None },
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
