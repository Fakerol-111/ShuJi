//! 消息发送与取消命令。
//!
//! 本文件定义了前端可直接调用的核心交互接口：
//! - `send_message`: 向内阁发送消息，自动判断走 pipeline 恢复还是常规工作流
//! - `discuss_with_cabinet`: 独立讨论模式（不修改项目状态）
//! - `cancel_discuss` / `cancel_processing`: 取消机制

// ============================================================================
// 依赖导入
// ============================================================================

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
use crate::models::dept_step::DeptStepEntry;
use crate::models::role::Role;

// ============================================================================
// send_message — 核心消息入口
// ============================================================================

/// 向内阁（或 pipeline）发送用户消息。
///
/// **双路径分发**:
/// 1. 如果磁盘上存在活跃的 `PlanRuntime` → 消息交给 pipeline 引擎，
///    用于恢复暂停状态（`AwaitingUserInput` / `AwaitingApproval`）
/// 2. 否则 → 消息路由到内阁 actor，走常规工作流通路
///
/// **延迟初始化**: 首次调用时自动创建完整的 actor 系统
///   - 5 个转发通道（emperor / dept_log / dept_step / plan / milestone）
///   - 每个通道 spawn 一个后台任务：转发到前端 + 写入缓存 + 持久化 JSONL
///   - 调用 `start_actor_system` 启动全部 9 部门 actor
///
/// **运行中的流程打断**: 如果当前有非内阁角色正在执行，
///   先发送 `FastMessage::Interrupt` 中断之，再投递新消息。
///
/// **工作流图谱归档**: 每次新消息都会将当前 WorkflowGraph 归档并重置。
#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    message: String,
) -> Result<String, String> {
    // 开始新一轮指标追踪（记录本轮开始时间、角色等）
    crate::round_metrics::start_round();

    // 加载完整配置（含 api_config.json + .env 合并结果）
    let config = crate::commands::settings::get_config()
        .await
        .map_err(friendly_error)?;

    // 获取当前打开项目的工作目录
    let p_working_dir = {
        let project_opt = state.current_project.lock().await;
        let p = project_opt
            .as_ref()
            .ok_or_else(|| friendly_error("no open project"))?;
        p.working_dir.clone()
    };

    // =======================================================================
    // 路径 A: Pipeline 恢复 — 磁盘上存在活跃的 PlanRuntime
    // =======================================================================
    let project_dir = std::path::Path::new(&p_working_dir);
    if let Some(runtime) = crate::pipeline::PlanRuntime::load_from(project_dir).await {
        log_console!("[pipeline] found active runtime on disk, resuming pipeline");

        // 确保 actor 系统已初始化（pipeline 需要部门路由能力）
        {
            let mut sys_lock = state.actor_system.lock().await;
            if sys_lock.is_none() {
                // 与下方常规路径相同的延迟初始化逻辑 — 创建 5 个转发通道
                let (emperor_tx, mut emperor_rx) = tokio::sync::mpsc::channel::<ChatMessage>(200);
                let (dept_log_tx, mut dept_log_rx) =
                    tokio::sync::mpsc::channel::<DeptLogEntry>(500);
                let (dept_step_tx, mut dept_step_rx) =
                    tokio::sync::mpsc::unbounded_channel::<DeptStepEntry>();
                let (plan_tx, mut plan_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(50);
                let (milestone_tx, mut milestone_rx) = tokio::sync::mpsc::channel::<String>(50);
                let app_handle = app.clone();

                let chat_hist = state.chat_history.clone();
                let dept_log_hist = state.dept_log_history.clone();
                let chat_persist_dir = p_working_dir.clone();

                // 通道 1: emperor → 前端 chat-message 事件
                // 工作流: emit 到前端 → 写入内存缓存 → 持久化到 .shuji/chat.jsonl
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

                // 通道 2: dept_log → 前端 dept-log 事件（仅写入内存缓存）
                let app2 = app.clone();
                let _dept_log_dir = p_working_dir.clone();
                tokio::spawn(async move {
                    while let Some(entry) = dept_log_rx.recv().await {
                        let _ = app2.emit("dept-log", &entry);
                        let mut hist = dept_log_hist.lock().await;
                        hist.push(entry.clone());
                    }
                });

                // 通道 3: dept_step → 前端 dept-step 事件
                let app_step = app.clone();
                tokio::spawn(async move {
                    while let Some(entry) = dept_step_rx.recv().await {
                        let _ = app_step.emit("dept-step", &entry);
                    }
                });

                // 通道 4: plan → 前端 plan-update 事件
                let app_plan = app.clone();
                tokio::spawn(async move {
                    while let Some(plan_json) = plan_rx.recv().await {
                        let _ = app_plan.emit("plan-update", &plan_json);
                    }
                });

                // 通道 5: milestone → 更新项目状态 + 持久化
                // 收到里程碑时：更新 Project.talk/summary → 保存到磁盘 → emit project-update → 写审计日志
                let app3 = app.clone();
                let wd = p_working_dir.clone();
                tokio::spawn(async move {
                    let _s = crate::storage::shuji_dir::ShujiDir::new(&wd);
                    while let Some(milestone) = milestone_rx.recv().await {
                        let st = app3.state::<AppState>();
                        let mut p_opt = st.current_project.lock().await;
                        if let Some(ref mut p) = *p_opt {
                            p.append_talk(&milestone);
                            p.summary = milestone.chars().take(120).collect();
                        }
                    }
                });

                // 启动完整的 9 部门 actor 系统
                let system = start_actor_system(
                    &config,
                    state.runtime_config.clone(),
                    Path::new(&p_working_dir),
                    Path::new(&p_working_dir),
                    state.cancel_flag.clone(),
                    emperor_tx,
                    dept_log_tx,
                    Some(dept_step_tx),
                    plan_tx,
                    milestone_tx,
                )
                .await;

                *sys_lock = Some(system);
            }
        }

        // 从 runtime 构建 pipeline 引擎，注入用户消息恢复执行
        let sys_lock = state.actor_system.lock().await;
        if let Some(system) = sys_lock.as_ref() {
            let engine = crate::pipeline::engine::PipelineEngine::from_runtime(
                runtime,
                system.senders.clone(),
                project_dir.to_path_buf(),
            );

            // 将用户消息作为输入恢复 pipeline 执行
            let result = engine.resume_with_input(Some(&message)).await;

            // 根据 pipeline 执行结果构造用户可读的状态消息
            let msg = match &result {
                crate::pipeline::PipelineResult::Complete { ref runtime } => {
                    format!("✅ Pipeline execution complete: {}", runtime.plan.summary)
                }
                crate::pipeline::PipelineResult::AwaitingUserInput {
                    step_id, question, ..
                } => {
                    format!(
                        "⏳ Pipeline waiting for user input (step {}): {}",
                        step_id, question
                    )
                }
                crate::pipeline::PipelineResult::AwaitingApproval {
                    doc_id, step_id, ..
                } => {
                    format!(
                        "⏳ Pipeline waiting for approval (step {}, doc {})",
                        step_id, doc_id
                    )
                }
                crate::pipeline::PipelineResult::StepFailed {
                    step_id, reason, ..
                } => {
                    format!("❌ Pipeline step {} failed: {}", step_id, reason)
                }
                crate::pipeline::PipelineResult::Aborted { .. } => {
                    // 中止时清理磁盘上的 runtime 文件
                    crate::pipeline::PlanRuntime::cleanup(project_dir).await;
                    "🛑 Pipeline execution aborted".to_string()
                }
                crate::pipeline::PipelineResult::Deadlock { .. } => {
                    "❌ Pipeline deadlock: remaining steps have unmet dependencies.".to_string()
                }
            };

            // 将 pipeline 结果发送到前端聊天面板
            let _ = system.emperor_tx.try_send(ChatMessage::new("System", &msg));
            log_console!("[pipeline] result: {}", msg);

            return Ok(msg);
        }
    }

    // =======================================================================
    // 路径 B: 常规流程 — 发送消息到内阁（无活跃 pipeline）
    // =======================================================================

    // 延迟初始化: 第一次发消息时创建完整的 actor 系统
    {
        let mut sys_lock = state.actor_system.lock().await;
        if sys_lock.is_none() {
            // 创建 5 个 mpsc 通道，连接 actor 系统与前端
            // 通道容量设计:
            //   emperor: 200  — 聊天消息（高频，需缓冲）
            //   dept_log: 500 — 部门日志（高频，批量写入）
            //   dept_step: 无界 — 步骤事件（允许瞬时尖峰）
            //   plan: 50 — 计划更新（低频）
            //   milestone: 50 — 里程碑（极低频）
            let (emperor_tx, mut emperor_rx) = mpsc::channel::<ChatMessage>(200);
            let (dept_log_tx, mut dept_log_rx) = mpsc::channel::<DeptLogEntry>(500);
            let (dept_step_tx, mut dept_step_rx) = mpsc::unbounded_channel::<DeptStepEntry>();
            let (plan_tx, mut plan_rx) = mpsc::channel::<serde_json::Value>(50);
            let (milestone_tx, mut milestone_rx) = mpsc::channel::<String>(50);
            let app_handle = app.clone();

            let chat_hist = state.chat_history.clone();
            let dept_log_hist = state.dept_log_history.clone();
            let chat_persist_dir = p_working_dir.clone();

            // 通道 1: emperor → 前端 chat-message 事件
            // 三步联动: Tauri emit(实时) → 内存缓存(上下文面板) → .shuji/chat.jsonl(持久化)
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

            // 通道 2: dept_log → 前端 dept-log 事件
            // 两步联动: Tauri emit → 内存缓存 + .shuji/dept-log.jsonl 持久化
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

            // 通道 3: dept_step → 前端 dept-step 事件（实时步骤流，无需持久化）
            let app_step = app.clone();
            tokio::spawn(async move {
                while let Some(entry) = dept_step_rx.recv().await {
                    let _ = app_step.emit("dept-step", &entry);
                }
            });

            // 通道 4: plan → 前端 plan-update 事件（实时计划进度，无需持久化）
            let app_plan = app.clone();
            tokio::spawn(async move {
                while let Some(plan_json) = plan_rx.recv().await {
                    let _ = app_plan.emit("plan-update", &plan_json);
                }
            });

            // 通道 5: milestone → 项目状态更新
            // 收到里程碑时执行:
            //   1. 更新 Project.talk（追加）和 Project.summary（取前120字符）
            //   2. 持久化项目状态到磁盘（ShujiDir::save_project）
            //   3. emit project-update 事件通知前端刷新
            //   4. 写审计日志（audit::append）
            let app3 = app.clone();
            let wd = p_working_dir.clone();
            tokio::spawn(async move {
                let s = crate::storage::shuji_dir::ShujiDir::new(&wd);
                while let Some(milestone) = milestone_rx.recv().await {
                    // 更新内存中的项目状态
                    let st = app3.state::<AppState>();
                    let mut p_opt = st.current_project.lock().await;
                    if let Some(ref mut p) = *p_opt {
                        p.append_talk(&milestone);
                        p.summary = milestone.chars().take(120).collect();
                    }
                    // 快照克隆以在锁释放后使用
                    let snapshot = p_opt.clone();
                    drop(p_opt);
                    // 持久化项目状态到 .shuji/state.json
                    if let Some(ref project) = snapshot {
                        let _ = s.save_project(project).await;
                    }
                    // 通知前端项目状态已更新
                    if let Some(ref project) = snapshot {
                        let _ = app3.emit("project-update", project);
                    }
                    // 写审计日志: 事件= milestone, role= 里程碑来源角色, detail= 前120字符
                    let event = "milestone";
                    let role = milestone.split('|').next().unwrap_or("").trim();
                    let detail = milestone.chars().take(120).collect::<String>();
                    crate::audit::append(Path::new(&wd), event, role, "", &detail).await;
                }
            });

            // 启动完整的 9 部门 actor 系统
            // 参数: 配置、项目/工作目录（此处相同）、取消标志、5 个通道的发送端
            let system = start_actor_system(
                &config,
                state.runtime_config.clone(),
                Path::new(&p_working_dir),
                Path::new(&p_working_dir),
                state.cancel_flag.clone(),
                emperor_tx,
                dept_log_tx,
                Some(dept_step_tx),
                plan_tx,
                milestone_tx,
            )
            .await;

            *sys_lock = Some(system);
        }
    }

    // 获取已初始化的 actor 系统句柄
    let sys_lock = state.actor_system.lock().await;
    let system = sys_lock
        .as_ref()
        .ok_or_else(|| friendly_error("actor system not initialized"))?;

    // 每次新消息前归档当前 WorkflowGraph 并创建新的
    {
        let wd = Path::new(&p_working_dir);
        let mut graph = system.workflow_graph.lock().await;
        let label: String = message.chars().take(60).collect();
        graph.archive_and_new(wd, label.trim()).await;
    }

    // 如果有其他部门正在执行，先发 FastMessage::Interrupt 中断之
    // 这确保新消息到达时没有旧流程在运行
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

    // 将用户消息投递到内阁邮箱，触发工作流
    system
        .send(&Role::Neige, ActorMessage::new(message, RouteMsgType::Task))
        .map_err(friendly_error)?;

    Ok("received".to_string())
}

// ============================================================================
// discuss_with_cabinet — 独立讨论模式
// ============================================================================

/// 与内阁进行独立讨论 — 不修改项目状态、不使用工具。
///
/// **与 send_message 的关键区别**:
/// - 不经过 actor 系统 — 直接创建 NeigeAgent 实例同步调用
/// - `discuss_mode: true` — 内阁不会调用任何文档/文件工具
/// - `fast_cancel: state.discuss_cancel` — 可通过 `cancel_discuss` 中断
/// - 返回 `ChatMessage` 给前端（而非 "received" 确认）
///
/// **上下文注入**: 将当前项目的 goal / summary / task / talk 作为参考上下文
///   注入到 prompt 中，让内阁了解项目现状但不做修改。
#[tauri::command]
pub async fn discuss_with_cabinet(
    state: State<'_, AppState>,
    message: String,
) -> Result<ChatMessage, String> {
    let config = crate::commands::settings::get_config()
        .await
        .map_err(friendly_error)?;

    // 获取项目上下文（只读引用，不做修改）
    let (working_dir, project_context) = {
        let project_opt = state.current_project.lock().await;
        let p = match project_opt.as_ref() {
            Some(p) => p,
            None => return Err(friendly_error("no open project")),
        };
        (
            p.working_dir.clone(),
            // 构造结构化的项目状态摘要，作为讨论的参考背景
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

    // 从配置中获取内阁专属的 API endpoint（支持 per-role key）
    let ep = config.for_role("neige");
    let client = AnthropicClient::new(ep.api_key, ep.api_url);
    // 独立模式不使用 agent runner，直接实例化 NeigeAgent
    let neige = NeigeAgent::new(
        client,
        &ep.model,
        Arc::new(AtomicBool::new(false)),
        None,
        None,
    );

    // 构造 AgentInput: discuss_mode=true 确保不调用任何修改工具
    let input = AgentInput {
        role: Role::Neige,
        task_description: format!(
            "(Current project state for reference)\n{}\n\n━━ Emperor Discussion ━━\n{}",
            project_context, message,
        ),
        context_messages: vec![],
        project_dir: std::path::PathBuf::from(&working_dir),
        working_dir: std::path::PathBuf::from(&working_dir),
        current_skill: None,
        resume_paused: false,
        context_window_config: Arc::new(HashMap::new()),
        runtime_config: state.runtime_config.clone(),
        discuss_mode: true, // 关键标志：讨论模式，不修改项目状态
        fast_cancel: state.discuss_cancel.clone(), // 复用 discuss_cancel 作为取消信号
        dept_step_tx: None, // 讨论模式不发送步骤事件
    };

    // 执行内阁 agent（同步等待结果）
    let output = neige.execute(&input).await.map_err(|e| {
        // 出错时重置取消标志，防止后续调用被误取消
        state
            .discuss_cancel
            .store(false, std::sync::atomic::Ordering::SeqCst);
        friendly_error(&e.to_string())
    })?;
    // 正常结束后重置取消标志
    state
        .discuss_cancel
        .store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(ChatMessage::new("内阁", &output.content))
}

// ============================================================================
// 取消命令
// ============================================================================

/// 取消活跃的讨论模式调用。
///
/// 原理: 设置 `discuss_cancel` AtomicBool 为 true，
/// `AgentController` 在每次工具调用迭代时检查该标志，发现为 true 则终止执行。
/// 这种方式不杀线程，而是让 agent 自行在安全点退出。
#[tauri::command]
pub async fn cancel_discuss(state: State<'_, AppState>) -> Result<(), String> {
    state
        .discuss_cancel
        .store(true, std::sync::atomic::Ordering::SeqCst);
    log_console!("[commands] cancel_discuss: flag set");
    Ok(())
}

/// 取消所有正在运行的 actor 处理流程。
///
/// **三层取消机制**（递进式，从最轻到最重）:
/// 1. **CancelMap**: 遍历所有部门的 `Arc<AtomicBool>`，全部置 true
///    → 各部门在 AgentController 迭代中自行检测并退出
/// 2. **FastMessage::Interrupt**: 通过专用 fast channel 发送即时中断信号
///    → 比 mailbox 消息优先级更高，可中断正在执行的工具调用
/// 3. **ActorMessage::interrupt()**: 向常规 mailbox 发送中断消息
///    → 作为兜底，清理 mailbox 中可能堆积的后续任务
///
/// 三层设计确保即使某一层失效，其他层仍能取消执行。
#[tauri::command]
pub async fn cancel_processing(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(sys) = state.actor_system.lock().await.as_ref() {
        // 第一层: 设置 CancelMap — 各部门在下一次 AgentController 迭代时检测并退出
        if let Ok(map) = sys.cancel_map.lock() {
            for flag in map.values() {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            log_console!("[commands] cancel_processing: all per-actor flags set");
        }
        // 第二层: 发送 FastMessage::Interrupt — 通过高优先级通道即时通知
        for tx in sys.fast_txs.values() {
            let _ = tx.send(FastMessage::Interrupt);
        }
        log_console!("[commands] cancel_processing: FastMessage::Interrupt sent to all actors");
        // 第三层: 发送 ActorMessage::interrupt() — 兜底常规 mailbox 中断
        for tx in sys.senders.values() {
            let _ = tx.send(crate::actor::ActorMessage::interrupt());
        }
        log_console!("[commands] cancel_processing: Interrupt sent to all actors");
    }
    Ok(())
}
