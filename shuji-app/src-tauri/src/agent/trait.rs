//! Agent 抽象层：核心 trait 及输入/输出类型定义。
//!
//! 本文件是项目中**最重要的抽象接口**——9 个部门 agent 全部实现此 trait。
//! 理解这个文件等于理解了 agent 系统的"合同条款"。

use std::collections::HashMap;

use crate::config::{RoleContextConfig, RuntimeConfig};
use crate::models::dept_step::DeptStepSender;
use crate::models::message::Message;
use crate::models::role::Role;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

// ============================================================================
// AgentInput — 一次 agent 执行所需的全部输入
// ============================================================================

/// 每次调用 `Agent::execute()` 时的完整输入参数包。
///
/// 字段按用途可分为三组：
/// - **身份与任务**：role, task_description, current_skill
/// - **运行环境**：project_dir, working_dir, runtime_config, context_window_config
/// - **控制信号**：discuss_mode, fast_cancel, resume_paused, dept_step_tx
#[derive(Debug, Clone)]
pub struct AgentInput {
    /// 当前角色（内阁、工部尚书等），决定系统提示词和可用工具集
    pub role: Role,

    /// 本次执行的任务描述（用户消息或上游部门的指令）
    pub task_description: String,

    /// 上游部门/ Pipeline 传入的文档 ID，注入 context 而非 task 正文
    pub upstream_doc_ids: Vec<String>,

    /// 上下文历史消息
    pub context_messages: Vec<Message>,

    /// 项目根目录（绝对路径），文件工具在此范围内操作
    pub project_dir: PathBuf,

    /// 工作目录（绝对路径），通常与 project_dir 相同
    pub working_dir: PathBuf,

    /// 上一轮执行中激活的技能名（仅内阁使用）。
    /// 用于技能守卫：防止跨执行轮次的误判重试。
    pub current_skill: Option<String>,

    /// 为 true 时，agent 应恢复之前暂停的会话（内阁等待皇帝决策状态），
    /// 而非构建全新上下文。
    pub resume_paused: bool,

    /// 各角色的上下文窗口覆盖配置（来自 context_config.json）。
    /// key 为角色名，value 为该角色的压缩阈值和策略。
    pub context_window_config: Arc<HashMap<String, RoleContextConfig>>,

    /// 运行时配置（时间限制、重试次数、watchdog 阈值等）
    pub runtime_config: Arc<RuntimeConfig>,

    /// 为 true 时，agent 只能使用只读工具（讨论模式 — 禁止创建文档、路由、文件写入）。
    pub discuss_mode: bool,

    /// 快速中断标志：当 actor 的 fast mailbox 收到 Interrupt 信号时置 true。
    /// `AgentController::run()` 在每次工具执行前和迭代间隙检查此标志。
    pub fast_cancel: Arc<AtomicBool>,

    /// 实时步骤事件的发送端（思考、工具调用、结果等）。
    /// 为 Some 时，AgentController 在 run() 过程中发出步骤事件给前端 DeptInspector。
    /// 为 None 时（讨论模式），不发送步骤事件。
    pub dept_step_tx: Option<DeptStepSender>,

    /// 为 false 时，内阁不得提取或提交 pipeline plan（如 pipeline 完成后的 summary 回合）。
    pub allow_pipeline_plan: bool,
}

// ============================================================================
// AgentOutput — 一次 agent 执行的输出
// ============================================================================

/// 每次调用 `Agent::execute()` 的返回结果。
///
/// 这是一个"富文本"输出——不仅包含文本内容，还包含路由指令、
/// 技能延续、暂停信号和 pipeline 计划等结构化字段。
#[derive(Debug, Clone)]
pub struct AgentOutput {
    /// 输出的文本内容（markdown 格式），展示在前端聊天面板
    pub content: String,

    /// 路由指令：告诉 actor 系统将消息发送到哪个下游部门。
    /// 为 None 表示不需要路由（直接返回给皇帝）。
    pub route: Option<crate::api::control::RouteTo>,

    /// 当前激活的技能名，用于跨轮次持久化（仅内阁使用）。
    /// 下次调用时作为 `AgentInput::current_skill` 回传。
    pub skill: Option<String>,

    /// 为 true 时，agent 已向皇帝展示了 `<options>` 标签，正在等待决策。
    /// actor 应暂停执行循环，等待下次皇帝消息时以 `resume_paused=true` 恢复。
    pub paused: bool,

    /// 内阁调用 `submit_pipeline_plan` 时捕获的 JSON 计划字符串。
    /// PipelineEngine 消费此字段来初始化或更新 pipeline 运行时。
    pub plan_json: Option<String>,

    /// 内阁 `request_decision` 工具产生的待选项（供前端渲染按钮）
    pub decision_options: Vec<String>,

    /// 本次执行中创建/更新的文档，供聊天区展示卡片。
    pub documents: Vec<crate::models::chat::ChatDocument>,
}

impl AgentOutput {
    /// 创建仅含文本内容的基础输出。
    /// route、skill、paused、plan_json 均取默认值（None/false）。
    pub fn new(content: String) -> Self {
        Self {
            content,
            route: None,
            skill: None,
            paused: false,
            plan_json: None,
            decision_options: vec![],
            documents: vec![],
        }
    }

    /// Builder 方法：设置暂停标志。
    /// 内阁输出 `<options>` 后设置 paused=true，
    /// 让 actor 等待皇帝做出选择后再恢复执行。
    pub fn with_paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }
}

// ============================================================================
// LoopDecision — 控制 actor 主循环的走向
// ============================================================================

/// `after_execute` 的返回值：决定 actor 主循环的下一步动作。
///
/// 使用场景：
/// - 大部分部门返回 `Done`（一轮执行后停止，等待下一次消息触发）
/// - 工部返回 `Continue(...)`（批次计划循环：完成一批后自动进入下一批）
/// - 内阁在讨论模式下返回 `Done`（不循环）
pub enum LoopDecision {
    /// 停止循环，跳出 actor 的 run_actor 主循环。
    /// actor 此后进入等待状态，直到 mailbox 收到新消息。
    Done,

    /// 继续循环，携带一条上下文消息（作为下一轮 execute 的 task_description）。
    /// 用于工部的批次计划：完成一批后自动注入下一批给 LLM。
    Continue(String),
}

// ============================================================================
// Agent trait — 所有部门 agent 的公共契约
// ============================================================================

/// 所有部门 agent 必须实现的 trait。
///
/// 关键设计：
/// - `Send + Sync` 约束：agent 实例可以在线程间安全共享（用于 tokio::spawn）
/// - `execute()` 接收 `&AgentInput`，返回 `AgentOutput`——一次输入产生一次输出
/// - `after_execute()` 钩子：在 execute 之后调用，决定是否继续循环
/// - 其余方法（`set_interrupt_flag`、`reset_plan`、`plan_display`）有默认实现，
///   各 agent 按需覆盖
///
/// 9 个部门中，除内阁有独立的上下文管理逻辑外，其余 8 个共用
/// `agent/runner.rs` 中的 `build_compact_handler` / `build_checkpoint_handler` 等工具函数。
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    /// 返回该 agent 所扮演的角色标识
    fn role(&self) -> Role;

    /// 执行一次 agent 调用——构建 prompt → 调用 LLM API → 执行工具 → 返回输出。
    ///
    /// 内部通常调用 `AgentController::run()` 管理工具调用循环，
    /// 由 controller 负责将工具结果反馈给 LLM 直到 LLM 产出最终文本。
    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput>;

    /// 在每次 `execute()` 完成后调用的钩子。默认：停止循环。
    ///
    /// 覆盖此方法的典型场景：
    /// - **工部尚书**（批次计划循环）：返回 `Continue(下一批的prompt)`，
    ///   actor 会再次调用 `execute()` 并传入新的 task_description
    /// - **内阁**（暂停等待决策）：返回 `Done`，actor 等待皇帝消息并以
    ///   `resume_paused=true` 重新进入 execute
    fn after_execute(&self, _output: &AgentOutput) -> LoopDecision {
        LoopDecision::Done
    }

    /// 用 actor 系统的 per-actor 取消标志覆盖 agent 的取消标志。
    ///
    /// 在 actor 系统启动时调用（见 `bootstrap.rs`），
    /// 确保 `Interrupt` / `cancel_agent` 信号能到达 `AgentController::run()`。
    fn set_interrupt_flag(&mut self, _flag: Arc<AtomicBool>) {}

    /// 重置每个 agent 的计划状态（例如新任务到达时）。
    /// 工部覆盖此方法来清空 PlanState。
    fn reset_plan(&self) {}

    /// 返回给前端进度卡片用的计划 JSON 字符串。
    /// 默认返回 "null"（无计划展示）。
    fn plan_display(&self) -> String {
        "null".to_string()
    }
}
