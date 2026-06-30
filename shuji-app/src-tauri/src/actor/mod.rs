//! Actor 系统核心定义。
//!
//! 本文件定义了 actor 系统的基础类型——消息格式、ActorContext、ActorSystem——
//! 以及 `run_actor` 主循环和消息路由逻辑（通过子模块 routing.rs 和 spawn.rs）。
//!
//! **Actor 模型概述**:
//! - 每个部门是一个独立 actor，通过 `tokio::spawn` 在各自 task 中运行
//! - Actor 间通信基于 mpsc channel（三层：常规 / fast / 前端转发）
//! - 内阁是唯一能取消其他部门的 actor（持有 cancel_map 和 fast_txs）
//!
//! **文件结构**:
//! - `mod.rs` — 类型定义（ActorMessage, ActorContext, ActorSystem, DeptLogEntry, FastMessage）
//! - `routing.rs` — 消息路由（forward_route）
//! - `spawn/` — run_actor 主循环（mailbox → exec → 输出分派）

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use std::fmt;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::agent::r#trait::Agent;
use crate::api::control::RouteMsgType;
use crate::config::RuntimeConfig;
use crate::logging::logger::Logger;
use crate::models::chat::ChatMessage;
use crate::models::dept_step::DeptStepSender;
use crate::models::role::Role;

// 子模块声明（模块代码在外部文件中，路由逻辑在 routing.rs，主循环在 spawn.rs）
mod routing;
mod spawn;
pub use routing::*;
pub use spawn::*;

// ============================================================================
// DeptLogEntry — 部门日志条目（前端状态面板）
// ============================================================================

/// 实时部门日志条目，emit 到前端 DeptStatusPanel。
///
/// 每条日志携带部门名、动作描述和时间戳，
/// 可选的 detail 字段用于长文本（如工具调用参数、错误详情）。
/// `#[serde(skip_serializing_if = "Option::is_none")]` — detail 为空时不序列化，
/// 减小 JSON 体积。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptLogEntry {
    /// 部门名，如 "内阁"、"工部尚书"
    pub dept: String,

    /// 动作描述，如 "→ 中书令"、"started processing"、"❌ execution error"
    pub action: String,

    /// 时间戳（HH:MM:SS 格式，人类可读）
    pub ts: String,

    /// 可选的详情文本，前端可展开查看
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl DeptLogEntry {
    /// 创建不含详情的日志条目。
    pub fn new(dept: &str, action: &str) -> Self {
        Self {
            dept: dept.to_string(),
            action: action.to_string(),
            ts: chrono::Local::now().format("%H:%M:%S").to_string(),
            detail: None,
        }
    }

    /// 创建含详情的日志条目。
    pub fn with_detail(dept: &str, action: &str, detail: &str) -> Self {
        Self {
            dept: dept.to_string(),
            action: action.to_string(),
            ts: chrono::Local::now().format("%H:%M:%S").to_string(),
            detail: Some(detail.to_string()),
        }
    }
}

// ============================================================================
// FastMessage — 高优先级快速通道消息
// ============================================================================

/// 通过 fast mailbox channel 发送的高优先级消息。
///
/// 每个 actor 有一个专用的有界 mpsc channel（容量 16），专门用于
/// 中断信号——绕过常规消息队列，直接到达 run_actor 的 select! 分支。
///
/// 当前只有一个变体 Interrupt，未来可扩展（如 Pause、Resume、PriorityTask）。
#[derive(Debug, Clone)]
pub enum FastMessage {
    /// 立即停止当前工具执行并返回。
    /// AgentController::run() 在每次工具调用前检查 fast_cancel 标志。
    Interrupt,
}

// ============================================================================
// ActorMessage — Actor 间常规消息
// ============================================================================

/// Actor 之间发送的常规消息。
///
/// 消息类型（`RouteMsgType`）决定处理方式：
/// - `Task` — 正常任务，执行 agent
/// - `Interrupt` — 中断信号，设置 cancel flag
/// - `Replace` — 替换当前任务内容（不中断执行，但用新内容替代原内容）
///
/// `reply_to` 字段用于 pipeline 引擎场景：pipeline step 等待 actor 返回结果时，
/// 通过此 channel 将 AgentOutput 发回给 pipeline engine。
#[derive(Debug, Clone)]
pub struct ActorMessage {
    /// 消息类型：Task | Interrupt | Replace
    pub msg_type: RouteMsgType,

    /// 消息主题/内容（任务描述或指令）
    pub subject: String,

    /// 可选的额外载荷（用于 RouteTo 携带短指令等）
    pub payload: Option<String>,

    /// 上游传递的文档 ID（与 task 分离，注入 agent 上下文而非任务正文）
    pub doc_ids: Vec<String>,

    /// Pipeline 引擎的回执通道
    pub reply_to: Option<mpsc::UnboundedSender<String>>,

    /// 为 false 时，内阁不得提取或提交 pipeline plan（如 pipeline 完成后的 summary 回合）。
    pub allow_pipeline_plan: bool,
}

impl ActorMessage {
    /// 创建一条新任务消息。
    pub fn new(subject: impl Into<String>, msg_type: RouteMsgType) -> Self {
        Self {
            msg_type,
            subject: subject.into(),
            payload: None,
            doc_ids: Vec::new(),
            reply_to: None,
            allow_pipeline_plan: true,
        }
    }

    /// Pipeline 完成后的总结任务：禁止再次提交 plan。
    pub fn pipeline_summary(subject: impl Into<String>) -> Self {
        Self {
            msg_type: RouteMsgType::Task,
            subject: subject.into(),
            payload: None,
            doc_ids: Vec::new(),
            reply_to: None,
            allow_pipeline_plan: false,
        }
    }

    /// 创建一条中断信号消息。
    /// 消息类型为 Interrupt，content 为空——run_actor 检测到 msg_type=Interrupt 即设置 cancel flag。
    pub fn interrupt() -> Self {
        Self {
            msg_type: RouteMsgType::Interrupt,
            subject: String::new(),
            payload: None,
            doc_ids: Vec::new(),
            reply_to: None,
            allow_pipeline_plan: true,
        }
    }
}

// ============================================================================
// ActorContext — run_actor 所需的完整上下文
// ============================================================================

/// 传递给 `run_actor` 的 per-actor 上下文。
///
/// 这是 actor 系统中最核心的数据结构——run_actor 主循环需要的所有
/// 通道端点、共享状态和配置全部打包在这里。
///
/// **所有权**: 每个 actor 独享一份 Context（move into tokio::spawn），
/// 但其中的 sender 和 Arc 字段被多个 actor 共享。
pub struct ActorContext {
    /// 该 actor 的角色标识
    pub role: Role,

    /// Agent 实例（实现了 Agent trait 的结构体）
    pub agent: Box<dyn Agent>,

    /// 常规 mailbox 接收端（无界 mpsc）
    pub rx: mpsc::UnboundedReceiver<ActorMessage>,

    /// 快速中断 mailbox 接收端（有界 mpsc，容量 16）。
    /// 用 Tokio Mutex 包裹——run_actor 的 select! 需要 &mut Receiver，
    /// 多个 select! 分支可能同时 poll，必须用 Mutex 同步。
    pub fast_rx: tokio::sync::Mutex<mpsc::Receiver<FastMessage>>,

    /// 同级部门的 sender 表：Key=Role, Value=发送端。
    /// 用于 `forward_route` 向目标部门投递消息。
    /// 注意：不包括自己——peer 是除自己外的所有部门。
    pub peers: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,

    /// 聊天消息 → 前端（emperor_tx channel）
    pub emperor_tx: mpsc::Sender<ChatMessage>,

    /// 部门日志 → 前端 DeptStatusPanel
    pub dept_log_tx: mpsc::Sender<DeptLogEntry>,

    /// 步骤事件 → 前端 DeptInspector（可选，讨论模式下为 None）
    pub dept_step_tx: Option<DeptStepSender>,

    /// 计划更新 → 前端工部进度卡片
    pub plan_tx: mpsc::Sender<serde_json::Value>,

    /// 里程碑事件 → 持久化 + 项目状态更新
    pub milestone_tx: mpsc::Sender<String>,

    /// 项目根目录（绝对路径）
    pub project_dir: PathBuf,

    /// 工作目录（绝对路径），通常与 project_dir 相同
    pub working_dir: PathBuf,

    /// 该 actor 的取消标志（由 cancel_map 管理）。
    /// AgentController::run() 每次迭代前检查此标志。
    pub cancel: Arc<AtomicBool>,

    /// 全部 agent 的取消标志映射表。**仅内阁**持有 Some。
    /// 内阁通过 `cancel_agent` 工具设置目标部门的 flag 来中断其执行。
    pub cancel_map: Option<crate::CancelMap>,

    /// 部门作用域日志写入器（写入 .shuji/logs/{role}/ 目录）
    pub logger: Logger,

    /// 跨 actor 共享上下文：Key=Role, Value=该角色最后一次输出内容。
    /// 用于失败回退时参考上游部门的输出。
    pub shared_context: Arc<Mutex<HashMap<Role, String>>>,

    /// 跨 actor 失败重试计数：Key=Role, Value=当前已重试次数。
    /// 超过 `MAX_FAILURE_RETRIES`(3) 后不再重试，汇报给皇帝。
    pub failure_retries: Arc<Mutex<HashMap<Role, u32>>>,

    /// 内阁与皇帝之间的完整对话历史。
    /// 内阁执行时注入为上下文消息（使内阁看到完整的对话脉络）。
    pub talk_history: Arc<Mutex<Vec<String>>>,

    /// Per-agent 任务计划（工部尚书用于多步骤执行）。
    /// 内容为工部的 PlanState 批次列表。
    pub plan: Arc<Mutex<Vec<String>>>,

    /// 当前激活的技能名，用于跨轮次持久化（仅内阁使用）。
    /// 下次执行时作为 AgentInput::current_skill 回传。
    pub current_skill: Arc<Mutex<Option<String>>>,

    /// 文移图 — 部门间任务流转 DAG。
    /// 共享引用：forward_route 写入边，send_message 归档读取。
    pub workflow_graph: Option<Arc<tokio::sync::Mutex<crate::workflow::WorkflowGraph>>>,

    /// 运行时配置（共享只读引用）
    pub runtime_config: Arc<RuntimeConfig>,

    /// Pipeline 后台执行 supervisor（内阁 submit plan 时非阻塞启动）
    pub pipeline_supervisor: Arc<crate::pipeline::supervisor::PipelineSupervisor>,

    /// 与 AppState.actor_system 同一槽位，pipeline 恢复时读取完整 ActorSystem
    pub actor_system_slot: Arc<tokio::sync::Mutex<Option<ActorSystem>>>,
}

// ============================================================================
// ActorSystem — 中央 actor 系统句柄
// ============================================================================

/// Actor 系统的中央句柄，启动时创建，注入到 Tauri 命令中。
///
/// **用途**: 任何需要向部门发送消息的地方（send_message、cancel_processing、
/// pipeline engine 等）都通过 ActorSystem 的 senders 和 fast_txs 进行。
///
/// **Drop 行为**: 当 ActorSystem 被释放时（例如项目切换），自动触发三层取消：
/// 1. 设置所有 cancel flag
/// 2. 发送 FastMessage::Interrupt
/// 3. 发送 ActorMessage::interrupt()
pub struct ActorSystem {
    /// 所有部门 actor 的常规 mailbox 发送端，按 Role 索引
    pub senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,

    /// 所有部门 actor 的 fast mailbox 发送端，用于高优先级中断信号
    pub fast_txs: HashMap<Role, mpsc::Sender<FastMessage>>,

    /// 聊天消息发送端 → 前端面板
    pub emperor_tx: mpsc::Sender<ChatMessage>,

    /// 部门日志发送端 → 前端 DeptStatusPanel
    pub dept_log_tx: mpsc::Sender<DeptLogEntry>,

    /// 步骤事件发送端 → 前端 DeptInspector
    pub dept_step_tx: Option<DeptStepSender>,

    /// Per-agent 取消标志，按 Role 索引。
    /// 内阁通过 cancel_agent 工具修改某个 role 的 flag 来中断之。
    pub cancel_map: crate::CancelMap,

    /// 全局取消标志（前端"停止"按钮触发）
    pub cancel: Arc<AtomicBool>,

    /// 文移图 — 共享引用，send_message 归档和 actor 写入都通过同一个 Arc。
    pub workflow_graph: Arc<tokio::sync::Mutex<crate::workflow::WorkflowGraph>>,
}

pub struct ActorSystemParts {
    pub senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    pub fast_txs: HashMap<Role, mpsc::Sender<FastMessage>>,
    pub emperor_tx: mpsc::Sender<ChatMessage>,
    pub dept_log_tx: mpsc::Sender<DeptLogEntry>,
    pub dept_step_tx: Option<DeptStepSender>,
    pub cancel_map: crate::CancelMap,
    pub cancel: Arc<AtomicBool>,
    pub workflow_graph: Arc<tokio::sync::Mutex<crate::workflow::WorkflowGraph>>,
}

impl ActorSystem {
    /// 构造 ActorSystem（仅供 bootstrap 使用）。
    pub fn new(parts: ActorSystemParts) -> Self {
        let ActorSystemParts {
            senders,
            fast_txs,
            emperor_tx,
            dept_log_tx,
            dept_step_tx,
            cancel_map,
            cancel,
            workflow_graph,
        } = parts;

        Self {
            senders,
            fast_txs,
            emperor_tx,
            dept_log_tx,
            dept_step_tx,
            cancel_map,
            cancel,
            workflow_graph,
        }
    }

    /// Duplicate channel handles for sharing with pipeline supervisor / AppState slot.
    pub fn duplicate_handles(&self) -> Self {
        Self {
            senders: self.senders.clone(),
            fast_txs: self.fast_txs.clone(),
            emperor_tx: self.emperor_tx.clone(),
            dept_log_tx: self.dept_log_tx.clone(),
            dept_step_tx: self.dept_step_tx.clone(),
            cancel_map: self.cancel_map.clone(),
            cancel: self.cancel.clone(),
            workflow_graph: self.workflow_graph.clone(),
        }
    }

    /// 向指定角色的 actor 发送消息。
    ///
    /// 错误情况：
    /// - 目标角色不存在于 senders 表中 → 返回 "找不到 X actor"
    /// - 目标 actor 已关闭（channel dropped） → 返回 "X actor 已关闭"
    pub fn send(&self, target: &Role, msg: ActorMessage) -> Result<(), String> {
        match self.senders.get(target) {
            Some(tx) => tx
                .send(msg)
                .map_err(|_| format!("{} actor 已关闭", target.name())),
            None => Err(format!("找不到 {} actor", target.name())),
        }
    }
}

/// ActorSystem 的 Drop 实现：释放时三层级联取消所有 actor。
///
/// ```text
/// 1. CancelMap → 所有 per-actor flag 置 true（AgentController 迭代检测）
/// 2. fast_txs → 向所有部门发送 FastMessage::Interrupt（即时信号）
/// 3. senders → 向所有部门发送 ActorMessage::interrupt()（兜底）
/// ```
impl Drop for ActorSystem {
    fn drop(&mut self) {
        // 第一层：设置所有 per-actor 取消标志
        if let Ok(map) = self.cancel_map.lock() {
            for flag in map.values() {
                flag.store(true, Ordering::SeqCst);
            }
        }

        // 第二层：通过 fast mailbox 向所有 actor 发送中断
        for tx in self.fast_txs.values() {
            let _ = tx.try_send(FastMessage::Interrupt);
        }

        // 第三层：通过常规 mailbox 向所有 actor 发送中断（兜底）
        for tx in self.senders.values() {
            let _ = tx.send(ActorMessage::interrupt());
        }

        log_console!(
            "[actor] ActorSystem dropped — all cancel flags set, FastMessage::Interrupt sent to all actors"
        );
    }
}

/// Debug 实现：只展示 actor 数量和取消状态，不展开所有 sender（避免日志爆炸）。
impl fmt::Debug for ActorSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorSystem")
            .field("actor_count", &self.senders.len())
            .field("cancel", &self.cancel)
            .finish()
    }
}
