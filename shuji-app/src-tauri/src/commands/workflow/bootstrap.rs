//! Actor 系统引导与初始化。
//!
//! 本文件是后端的**中枢**——负责实例化全部 9 个部门 agent，建立通信通道，
//! 并将所有 actor spawn 到 tokio 运行时。调用方（send.rs）只需拿到 ActorSystem
//! 句柄即可向任意部门投递消息。

// ============================================================================
// 依赖导入
// ============================================================================

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{Emitter, Manager};
use tokio::sync::mpsc;

use crate::actor::ActorSystem;
use crate::actor::{ActorContext, ActorMessage, DeptLogEntry, FastMessage};
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
use crate::commands::project::AppState;
use crate::commands::settings::AppConfig;
use crate::models::chat::ChatMessage;
use crate::models::dept_step::{DeptStepEntry, DeptStepSender};
use crate::models::role::Role;

// ============================================================================
// ContextStats — 前端上下文面板用统计信息
// ============================================================================

/// 单角色上下文使用统计，由 `get_context_stats` 命令消费展示在前端侧边栏。
///
/// 字段说明:
/// - `message_count`  — 当前上下文中保留的消息条数
/// - `token_count`    — 估算的 token 总量
/// - `token_threshold` — 该角色的压缩阈值（超过此值触发 compaction）
/// - `compressed`     — 是否已经过压缩
/// - `skill_count`    — 当前加载的技能摘要数目
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextStats {
    pub message_count: usize,
    pub token_count: usize,
    pub token_threshold: usize,
    pub compressed: bool,
    pub skill_count: usize,
}

// ============================================================================
// build_agents — 工厂：为每个部门创建 agent 实例
// ============================================================================

/// 创建全部 9 个部门的 agent 实例并注入各自专属的 API 客户端。
///
/// **设计要点**:
/// - 每个部门使用 `config.for_role(name)` 获取各自独立的 API key/url/model，
///   支持不同部门配置不同供应商（如内阁用 Claude，工部用 DeepSeek）
/// - 内阁 (`NeigeAgent`) 额外接收 `cancel_map` 和 `fast_txs`，
///   因为内阁需要能够取消其他部门（cancel_agent 工具）
/// - 尚书令在此函数中**不创建**——它需要等待其他部门信道就绪后才能构造
///   （见 `start_actor_system` 中的延迟构造逻辑）
///
/// **部门角色速查**:
/// ```
///  内阁(Neige) ──编排中枢
///   ├─ 中书令(Zhongshuling)   — 方案设计
///   ├─ 门下侍中(MenxiaShizhong) — 审查
///   └─ 尚书令(Shangshuling)   — 执行调度 → 六部
///       ├─ 吏部(LiBu)     — 详细设计
///       ├─ 兵部(Bingbu)   — 测试+接口契约
///       ├─ 工部(Gongbu)   — TDD 编码
///       ├─ 刑部(Xingbu)   — 运行测试验证
///       └─ 礼部(LiBuR)    — 规范检查+审计
/// ```
fn build_agents(
    config: &AppConfig,
    cancel: Arc<AtomicBool>,
    cancel_map: crate::CancelMap,
    fast_txs: crate::FastTxMap,
) -> HashMap<Role, Box<dyn Agent>> {
    let mut agents: HashMap<Role, Box<dyn Agent>> = HashMap::new();

    // ── 三省 ──

    // 门下侍中: 审查者 — 使用 review_overall / review_phase 两个技能
    let menxiashizhong_ep = config.for_role("menxiashizhong");
    agents.insert(
        Role::MenxiaShizhong,
        Box::new(MenxiaShizhongAgent::new(
            AnthropicClient::new(menxiashizhong_ep.api_key, menxiashizhong_ep.api_url),
            &menxiashizhong_ep.model,
            cancel.clone(),
        )),
    );

    // 中书令: 总设计师 — 7 个自管理技能（设计/分析/诊断）
    let zhongshuling_ep = config.for_role("zhongshuling");
    agents.insert(
        Role::Zhongshuling,
        Box::new(ZhongshulingAgent::new(
            AnthropicClient::new(zhongshuling_ep.api_key, zhongshuling_ep.api_url),
            &zhongshuling_ep.model,
            cancel.clone(),
        )),
    );

    // ── 六部（尚书省直辖） ──

    // 吏部尚书: 详细设计 — 仅使用文档工具
    let libushangshu_ep = config.for_role("libushangshu");
    agents.insert(
        Role::LiBuShangshu,
        Box::new(LibuShangshuAgent::new(
            AnthropicClient::new(libushangshu_ep.api_key, libushangshu_ep.api_url),
            &libushangshu_ep.model,
            cancel.clone(),
        )),
    );

    // 兵部尚书: 测试+接口契约 — 使用文件写入 + 文档工具
    let bingbushangshu_ep = config.for_role("bingbushangshu");
    agents.insert(
        Role::BingbuShangshu,
        Box::new(BingbuShangshuAgent::new(
            AnthropicClient::new(bingbushangshu_ep.api_key, bingbushangshu_ep.api_url),
            &bingbushangshu_ep.model,
            cancel.clone(),
        )),
    );

    // 工部尚书: TDD 编码 — 批次计划循环，推理开关分离
    let gongbushangshu_ep = config.for_role("gongbushangshu");
    agents.insert(
        Role::GongbuShangshu,
        Box::new(GongbuShangshuAgent::new(
            AnthropicClient::new(gongbushangshu_ep.api_key, gongbushangshu_ep.api_url),
            &gongbushangshu_ep.model,
            cancel.clone(),
        )),
    );

    // 刑部尚书: 测试验证 — 运行测试，file bug
    let xingbushangshu_ep = config.for_role("xingbushangshu");
    agents.insert(
        Role::XingbuShangshu,
        Box::new(XingbuShangshuAgent::new(
            AnthropicClient::new(xingbushangshu_ep.api_key, xingbushangshu_ep.api_url),
            &xingbushangshu_ep.model,
            cancel.clone(),
        )),
    );

    // 礼部尚书: 规范检查+审计 — 使用 audit checklist 工具
    let liburshangshu_ep = config.for_role("liburshangshu");
    agents.insert(
        Role::LiBuRShangshu,
        Box::new(LibuRShangshuAgent::new(
            AnthropicClient::new(liburshangshu_ep.api_key, liburshangshu_ep.api_url),
            &liburshangshu_ep.model,
            cancel.clone(),
        )),
    );

    // ── 内阁: 编排中枢 — 12 个技能、soul 系统、取消其他部门的能力 ──
    // 内阁是唯一能取消其他部门的角色，所以需要 cancel_map 和 fast_txs
    let neige_ep = config.for_role("neige");
    agents.insert(
        Role::Neige,
        Box::new(NeigeAgent::new(
            AnthropicClient::new(neige_ep.api_key, neige_ep.api_url),
            &neige_ep.model,
            cancel,           // 全局取消标志
            Some(cancel_map), // 各角色取消标志（内阁可用 cancel_agent 工具操作）
            Some(fast_txs),   // 快速中断通道（内阁可向任意部门发 FastMessage::Interrupt）
        )),
    );

    agents
}

// ============================================================================
// start_actor_system — Actor 系统启动主入口
// ============================================================================

/// 初始化并启动完整的 9 部门 actor 系统。
///
/// **执行流程**（按顺序）:
/// 1. 为所有 9 个角色创建 **fast channel**（容量 16，用于即时中断信号）
/// 2. 调用 `build_agents()` 创建 8 个部门 agent（尚书令除外）
/// 3. 为每个部门创建 **unbounded mailbox**（常规消息通道）
/// 4. **延迟构造尚书令**: 等待其他部门信道就绪后，将所有 peers 注入尚书令
/// 5. 为每个角色组装 `ActorContext`，通过 `tokio::spawn` 启动 `run_actor` 循环
/// 6. 返回 `ActorSystem` 句柄（供 `send_message` 等命令使用）
///
/// **为什么尚书令要延迟构造**?
/// 尚书令需要持有所有其他部门的 sender 引用（用于 `route_to` 路由），
/// 而其他部门的 sender 必须先创建好才能注入。
///
/// **通道体系**（三层）:
/// ```
///  常规消息:  UnboundedChannel  (无界 mpsc) — ActorMessage 队列
///  快速中断:  Bounded(16)       (有界 mpsc) — FastMessage (Interrupt)
///  前端转发:  Bounded(200/500)  (有界 mpsc) — ChatMessage / DeptLogEntry
/// ```
#[allow(clippy::too_many_arguments)]
pub async fn start_actor_system(
    config: &AppConfig,
    runtime_config: Arc<crate::config::RuntimeConfig>,
    project_dir: &Path,
    working_dir: &Path,
    cancel: Arc<AtomicBool>,                  // 全局取消标志（所有部门共享）
    emperor_tx: mpsc::Sender<ChatMessage>,    // 聊天消息 → 前端
    dept_log_tx: mpsc::Sender<DeptLogEntry>,  // 部门日志 → 前端
    dept_step_tx: Option<DeptStepSender>,     // 步骤事件 → 前端（可选）
    plan_tx: mpsc::Sender<serde_json::Value>, // 计划更新 → 前端
    milestone_tx: mpsc::Sender<String>,       // 里程碑事件 → 持久化
    pipeline_supervisor: Arc<crate::pipeline::supervisor::PipelineSupervisor>,
    actor_system_slot: Arc<tokio::sync::Mutex<Option<ActorSystem>>>,
) -> ActorSystem {
    // ── Step 1: 初始化 CancelMap（每个角色一个 AtomicBool） ──
    let cancel_map: crate::CancelMap = Arc::new(std::sync::Mutex::new(HashMap::new()));

    // 全部 9 个角色（尚书令虽延迟构造，但在这里预先纳入）
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

    // ── Step 2: 创建 fast channel（快速中断通道） ──
    // 容量 16：足够容纳连续多次 Interrupt 信号，又不至于无限增长。
    // FastMessage 只有 Interrupt 一种变体，极其轻量。
    let mut fast_txs: HashMap<Role, mpsc::Sender<FastMessage>> = HashMap::new();
    let mut fast_rxs: HashMap<Role, tokio::sync::Mutex<mpsc::Receiver<FastMessage>>> =
        HashMap::new();
    for role in &all_roles {
        let (fast_tx, fast_rx) = mpsc::channel(16);
        fast_txs.insert(*role, fast_tx);
        // rx 端用 Tokio Mutex 包裹，因为 run_actor 的 select! 需要 &mut receiver
        // 而多个 select! 分支可能同时借用（需要 Mutex 来同步访问）
        fast_rxs.insert(*role, tokio::sync::Mutex::new(fast_rx));
    }
    let fast_txs = Arc::new(fast_txs);

    // ── Step 3: 构建 agent 实例（尚书令除外） ──
    let agents = build_agents(config, cancel.clone(), cancel_map.clone(), fast_txs.clone());

    // ── Step 4: 为每个部门创建 unbounded mailbox ──
    // 为什么用无界通道？
    //   如果改用有界通道，所有调用方（ActorSystem::send()、forward_route、
    //   fallback_to_dispatcher 等）都必须改用 async send()，而且会引入
    //   背压风险——actor 可能在持有取消标志或锁的情况下因 send 阻塞而死锁。
    //   当前项目消息量可控，无界通道的内存风险极低。如果后续出现证据表明
    //   长时间运行任务导致内存增长，再考虑切换到 bounded + try_send 模式。
    let mut senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>> = HashMap::new();
    let mut contexts: Vec<(Role, Box<dyn Agent>, mpsc::UnboundedReceiver<ActorMessage>)> =
        Vec::new();

    for (role, mut agent) in agents {
        // 为每个角色创建独立的取消标志，注册到 cancel_map
        let actor_flag = Arc::new(AtomicBool::new(false));
        agent.set_interrupt_flag(actor_flag.clone());
        cancel_map.lock().unwrap().insert(role, actor_flag);

        // 创建该角色的 mailbox: (tx → senders 表, rx → contexts 表)
        let (tx, rx) = mpsc::unbounded_channel();
        senders.insert(role, tx);
        contexts.push((role, agent, rx));
    }

    // ── Step 5: 加载/创建工作流图谱（文移图） ──
    // WorkflowGraph 记录部门间路由关系，从磁盘加载以支持会话恢复。
    // 尚书省和六部 actor 共享同一个 Arc 引用。
    let workflow_graph = Arc::new(tokio::sync::Mutex::new(
        crate::workflow::WorkflowGraph::load_or_new(working_dir).await,
    ));

    // ── Step 6: 延迟构造尚书令 ──
    // 尚书令是执行调度中枢，需要：
    //   - 所有其他部门的 sender（用于 route_to 路由）
    //   - workflow_graph（读写工作流状态）
    //   - fast_txs（向六部发送快速中断）
    let shangshuling_ep = config.for_role("shangshuling");
    let shangshuling_agent = {
        // 收集所有已创建部门的 sender 作为尚书令的 peers
        // 注意：这里包含内阁和三省，尚书令可以向任意角色路由
        let mut shangshuling_peers: HashMap<Role, mpsc::UnboundedSender<ActorMessage>> =
            HashMap::new();
        for (other_role, tx) in &senders {
            shangshuling_peers.insert(*other_role, tx.clone());
        }

        // 创建尚书令自己的 mailbox 并注册到 senders 表
        let (shangshuling_tx, shangshuling_rx) = mpsc::unbounded_channel();
        senders.insert(Role::Shangshuling, shangshuling_tx);

        let agent = Box::new(ShangshulingAgent::new(
            AnthropicClient::new(shangshuling_ep.api_key, shangshuling_ep.api_url),
            &shangshuling_ep.model,
            cancel.clone(),
            shangshuling_peers, // 尚书令持有所有同级/下级部门的 sender
            Some(workflow_graph.clone()),
            Some(Arc::new((*fast_txs).clone())),
        ));
        (agent, shangshuling_rx)
    };

    // 将尚书令加入 contexts 表（与其他部门同样的注册流程）
    {
        let (mut agent, rx) = shangshuling_agent;
        let actor_flag = Arc::new(AtomicBool::new(false));
        agent.set_interrupt_flag(actor_flag.clone());
        cancel_map
            .lock()
            .unwrap()
            .insert(Role::Shangshuling, actor_flag);
        contexts.push((Role::Shangshuling, agent, rx));
    }

    // ── Step 7: 为每个角色组装 ActorContext 并 spawn ──
    let all_senders = senders.clone();

    let system = ActorSystem {
        senders: all_senders.clone(),
        fast_txs: (*fast_txs).clone(),
        emperor_tx: emperor_tx.clone(),
        dept_log_tx: dept_log_tx.clone(),
        dept_step_tx: dept_step_tx.clone(),
        cancel_map: cancel_map.clone(),
        cancel: cancel.clone(),
        workflow_graph: workflow_graph.clone(),
    };

    {
        let mut slot = actor_system_slot.lock().await;
        *slot = Some(system.duplicate_handles());
    }

    // 跨 actor 共享的状态
    let shared_context: Arc<std::sync::Mutex<HashMap<Role, String>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    let failure_retries: Arc<std::sync::Mutex<HashMap<Role, u32>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    let talk_history: Arc<std::sync::Mutex<Vec<String>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    for (role, agent, rx) in contexts {
        // 组装该角色的 peers 表：除自己之外的所有部门 sender
        let mut peers: HashMap<Role, mpsc::UnboundedSender<ActorMessage>> = HashMap::new();
        for (other_role, tx) in &all_senders {
            if *other_role != role {
                peers.insert(*other_role, tx.clone());
            }
        }

        let actor_flag = cancel_map.lock().unwrap().get(&role).unwrap().clone();
        // 每个角色的日志写入 .shuji/logs/{role}/ 目录
        let logger = crate::logging::logger::Logger::new(&working_dir.join(".shuji"));
        let is_neige = role == Role::Neige;
        // 从 fast_rxs 表中取出该角色的 fast channel 接收端（所有权转移）
        let fast_rx = fast_rxs.remove(&role).unwrap();

        // ActorContext 是 run_actor 所需的完整上下文包
        let ctx = ActorContext {
            role,
            agent,
            rx,                                                // 常规 mailbox 接收端
            fast_rx,                                           // 快速中断通道接收端
            peers,                                             // 其他部门的 sender（用于 route_to）
            emperor_tx: emperor_tx.clone(),                    // 聊天输出 → 前端
            dept_log_tx: dept_log_tx.clone(),                  // 日志输出 → 前端
            dept_step_tx: dept_step_tx.clone(),                // 步骤事件 → 前端
            plan_tx: plan_tx.clone(),                          // 计划更新 → 前端
            plan: Arc::new(std::sync::Mutex::new(Vec::new())), // 当前执行计划的快照
            milestone_tx: milestone_tx.clone(),                // 里程碑 → 持久化
            project_dir: project_dir.to_path_buf(),
            working_dir: working_dir.to_path_buf(),
            cancel: actor_flag, // 该部门专属取消标志
            cancel_map: if is_neige {
                Some(cancel_map.clone()) // 只有内阁可以取消别人
            } else {
                None
            },
            logger,
            shared_context: shared_context.clone(), // 跨 actor 共享上下文
            failure_retries: failure_retries.clone(), // 跨 actor 失败重试计数
            talk_history: talk_history.clone(),     // 跨 actor 对话历史
            current_skill: Arc::new(std::sync::Mutex::new(None)), // 当前激活的技能
            workflow_graph: Some(workflow_graph.clone()),
            runtime_config: runtime_config.clone(),
            pipeline_supervisor: pipeline_supervisor.clone(),
            actor_system_slot: actor_system_slot.clone(),
        };

        // spawn: 每个 actor 在自己的 tokio task 中独立运行
        // run_actor 是一个无限循环，监听 mailbox + fast channel，直到取消
        tokio::spawn(crate::actor::run_actor(ctx));
    }

    system
}

// ============================================================================
// ensure_actor_system — 延迟初始化 actor 系统（send_message 等入口复用）
// ============================================================================

/// 启动 5 条事件转发通道：actor → 前端 emit + 内存缓存 + 持久化。
///
/// 普通消息路径与 pipeline 恢复路径共用此逻辑，避免 send.rs 内重复初始化。
#[allow(clippy::too_many_arguments)]
pub fn spawn_event_forwarders(
    app: tauri::AppHandle,
    state: &AppState,
    working_dir: &str,
    mut emperor_rx: mpsc::Receiver<ChatMessage>,
    mut dept_log_rx: mpsc::Receiver<DeptLogEntry>,
    mut dept_step_rx: mpsc::UnboundedReceiver<DeptStepEntry>,
    mut plan_rx: mpsc::Receiver<serde_json::Value>,
    mut milestone_rx: mpsc::Receiver<String>,
) {
    let chat_hist = state.chat_history.clone();
    let dept_log_hist = state.dept_log_history.clone();
    let chat_persist_dir = working_dir.to_string();
    let app_handle = app.clone();

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

    let app2 = app.clone();
    let dept_log_dir = working_dir.to_string();
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

    let app_step = app.clone();
    tokio::spawn(async move {
        while let Some(entry) = dept_step_rx.recv().await {
            let _ = app_step.emit("dept-step", &entry);
        }
    });

    let app_plan = app.clone();
    tokio::spawn(async move {
        while let Some(plan_json) = plan_rx.recv().await {
            let _ = app_plan.emit("plan-update", &plan_json);
        }
    });

    let app3 = app.clone();
    let wd = working_dir.to_string();
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
}

/// 确保 actor 系统已初始化；若已存在则直接返回。
pub async fn ensure_actor_system(
    app: tauri::AppHandle,
    state: &AppState,
    config: &AppConfig,
    working_dir: &str,
) -> Result<(), String> {
    {
        let guard = state.actor_system.lock().await;
        if guard.is_some() {
            return Ok(());
        }
    }

    let (emperor_tx, emperor_rx) = mpsc::channel::<ChatMessage>(200);
    let (dept_log_tx, dept_log_rx) = mpsc::channel::<DeptLogEntry>(500);
    let (dept_step_tx, dept_step_rx) = mpsc::unbounded_channel::<DeptStepEntry>();
    let (plan_tx, plan_rx) = mpsc::channel::<serde_json::Value>(50);
    let (milestone_tx, milestone_rx) = mpsc::channel::<String>(50);

    spawn_event_forwarders(
        app,
        state,
        working_dir,
        emperor_rx,
        dept_log_rx,
        dept_step_rx,
        plan_rx,
        milestone_rx,
    );

    let system = start_actor_system(
        config,
        crate::commands::project::snapshot_runtime_config(&state.runtime_config),
        Path::new(working_dir),
        Path::new(working_dir),
        state.cancel_flag.clone(),
        emperor_tx,
        dept_log_tx,
        Some(dept_step_tx),
        plan_tx,
        milestone_tx,
        state.pipeline_supervisor.clone(),
        state.actor_system.clone(),
    )
    .await;

    *state.actor_system.lock().await = Some(system);
    Ok(())
}
