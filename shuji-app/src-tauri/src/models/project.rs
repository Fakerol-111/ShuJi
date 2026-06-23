//! 项目数据模型。
//!
//! 定义了项目从创建到完成的完整状态模型，包括：
//! - `Project` — 持久化的项目元数据 + 进度状态
//! - `ProjectSummary` — 项目列表用的轻量摘要
//! - `ProjectSnapshot` / `PhaseSnapshot` — 前端仪表盘的进度快照
//! - 全套状态枚举 — 整体状态 + 各阶段的 design/execution 状态
//!
//! Project 持久化位置: `.shuji/state.json`

use serde::{Deserialize, Serialize};

// ============================================================================
// Project — 核心项目结构
// ============================================================================

/// 项目的完整状态，持久化到 `.shuji/state.json`。
///
/// 字段分为三组：
/// - **元数据**（创建时确定，不再变）：id, name, goal, working_dir, created_at
/// - **进度状态**（随工作流推进更新）：state, overall, phases, phase_count
/// - **运行时上下文**（内阁 + actor 更新）：talk, task, summary, resume, last_neige_msg, summary_prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// 项目唯一标识 (UUID)，创建时生成
    pub id: String,

    /// 项目显示名称（用户设定，可在 UI 修改）
    pub name: String,

    /// 项目目标描述（用户创建项目时输入的自然语言需求）
    pub goal: String,

    /// 项目工作目录（绝对路径），所有文件读写均在此范围内
    pub working_dir: String,

    /// 当前阶段标签，如 "Designing", "Implementing", "Completed"
    pub state: String,

    /// 整体进度状态（设计 → 审查 → 待批 → 批准/驳回）
    pub overall: OverallStatus,

    /// 各阶段的运行时状态（design + execution 进度）
    pub phases: Vec<PhaseRuntime>,

    /// 阶段总数
    pub phase_count: u32,

    /// 项目创建时间 (ISO 8601)
    pub created_at: String,

    /// 最后更新时间 (ISO 8601)，每次保存时刷新
    pub updated_at: String,

    /// 内阁最近一次输出的消息内容，用于跨 dispatcher 周期的上下文保持。
    /// `#[serde(default)]` — 旧版本 state.json 无此字段，反序列化时给空串。
    #[serde(default)]
    pub last_neige_msg: String,

    /// 紧凑的项目进度摘要，随工作推进持续更新。
    /// 由 milestone 通道自动更新（取前 120 字符）。
    #[serde(default)]
    pub summary: String,

    /// 项目里程碑记录（追加式，永不裁剪）。
    /// 格式示例:
    /// ```text
    /// [10:00] 整体方案审查通过
    /// [10:15] 皇帝批准整体方案
    /// ```
    #[serde(default)]
    pub task: String,

    /// 恢复上下文 — 在每次关键路由操作时更新。
    /// 项目重新加载时，内阁读取此字段作为"项目摘要"来了解中断前的状态。
    #[serde(default)]
    pub resume: String,

    /// 最近一次 summary 技能的完整输出。
    /// 注入到后续 summary 调用中作为上下文，实现增量更新。
    #[serde(default)]
    pub summary_prompt: String,

    /// 对话日志，智能保留策略:
    /// - 保留最近 ~12 条完整内容
    /// - 超过阈值时，旧内容压缩为摘要行（基于 task 里程碑生成）
    /// - 格式: `[摘要行]\n---\n[近期详细条目...]`
    ///
    /// 注意：与 `chat_history` (AppState) 不同——talk 是持久化的压缩日志，
    /// chat_history 是内存中的完整会话记录（含前端展示的所有 ChatMessage）。
    #[serde(default)]
    pub talk: String,
}

// ============================================================================
// ProjectSummary — 项目列表轻量摘要
// ============================================================================

/// 项目列表项，`list_projects` 命令的返回值。
/// 剥离了沉重的运行时上下文（talk/resume/summary_prompt），只保留列表展示所需字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub working_dir: String,
    pub created_at: String,

    /// 整体状态的 label 文本，如 "Overall: Designing"
    pub overall_status: String,

    /// 所有阶段状态的拼接文本，如 "Phase 1 Design: Approved | Phase 1 Execution: Coding"
    pub phases_status: String,
}

// ============================================================================
// ProjectSnapshot — 前端仪表盘进度快照
// ============================================================================

/// 项目进度的前端仪表盘快照。
/// `Project::snapshot()` 实时计算，不持久化——每次请求时从 Project.overall + phases 推导。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    /// 整体进度状态
    pub overall: OverallStatus,

    /// 各阶段快照
    pub phases: Vec<PhaseSnapshot>,

    /// 总体完成百分比 (0.0 ~ 100.0)
    pub overall_progress: f64,
}

/// 单个阶段的进度快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSnapshot {
    /// 阶段序号（0-based）
    pub index: u32,

    /// 设计阶段的状态 label
    pub design: String,

    /// 执行阶段的状态 label
    pub execution: String,
}

// ============================================================================
// Project impl — append_talk + snapshot
// ============================================================================

impl Project {
    /// 向对话日志追加一行，超出容量时自动压缩旧条目。
    ///
    /// **压缩机制**:
    /// 1. 新条目追加到末尾（格式 `[HH:MM] 内容`）
    /// 2. 若行数 > 12，取最近的 8 条保留
    /// 3. 旧内容被替换为 `[Previous Summary] <task 最后一行>`
    ///
    /// 这样 talk 字段永远不会无限膨胀，同时保留了最近上下文。
    pub fn append_talk(&mut self, line: &str) {
        let ts = chrono::Local::now().format("%H:%M").to_string();
        let entry = format!("[{}] {}", ts, line);
        if self.talk.is_empty() {
            self.talk = entry;
        } else {
            self.talk.push_str(&format!("\n{}", entry));
        }
        // 激进裁剪：超过 12 行时只保留近期 8 条
        let lines: Vec<&str> = self.talk.lines().collect();
        if lines.len() > 12 {
            // 从 task 最后一行提取摘要标签（去除时间戳和方括号）
            let summary = if self.task.is_empty() {
                "Project in progress"
            } else {
                let last = self.task.lines().last().unwrap_or("项目进行中");
                last.trim_start_matches(|c: char| {
                    c == '[' || c == ']' || c.is_numeric() || c == ':' || c == ' '
                })
            };
            let kept = lines[lines.len().saturating_sub(8)..].join("\n");
            self.talk = format!("[Previous Summary] {}\n---\n{}", summary, kept);
        }
    }

    /// 生成前端仪表盘进度快照。
    ///
    /// progress 计算: 每个阶段有 design + execution 两个检查点，
    /// 加上 overall 批准点，共 `phase_count * 2 + 1` 个总检查项。
    /// 完成率 = done / total * 100%。
    pub fn snapshot(&self) -> ProjectSnapshot {
        ProjectSnapshot {
            overall: self.overall.clone(),
            phases: self
                .phases
                .iter()
                .map(|p| PhaseSnapshot {
                    index: p.index,
                    design: p.design.label(p.index),
                    execution: p.execution.label(p.index),
                })
                .collect(),
            overall_progress: calc_progress(self.phase_count, &self.overall, &self.phases),
        }
    }
}

/// 计算项目总体完成百分比。
///
/// 检查项: `phase_count * 2`（每阶段 design + execution） + 1（overall 批准）
/// 每个检查项从状态枚举判断是否完成。
fn calc_progress(phase_count: u32, overall: &OverallStatus, phases: &[PhaseRuntime]) -> f64 {
    let total = phase_count as f64 * 2.0 + 1.0;
    let mut done = 0.0;
    if *overall == OverallStatus::Approved {
        done += 1.0;
    }
    for phase in phases {
        if phase.design == PhaseDesignStatus::Approved {
            done += 1.0;
        }
        if phase.execution == PhaseExecutionStatus::Completed {
            done += 1.0;
        }
    }
    done / total * 100.0
}

// ============================================================================
// 项目状态枚举
// ============================================================================

// ── OverallStatus — 整体项目状态 ──

/// 项目的整体生命周期状态。
///
/// 状态流转:
/// ```text
/// NotStarted → Designing → Reviewing → PendingApproval → Approved
///                                          ↓
///                                       Rejected → Designing (重新设计)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OverallStatus {
    NotStarted,      // 尚未开始
    Designing,       // 中书令正在设计方案
    Reviewing,       // 门下侍中正在审查
    PendingApproval, // 等待皇帝朱批
    Rejected,        // 审查未通过 / 皇帝驳回
    Approved,        // 已批准，可进入执行阶段
}

impl OverallStatus {
    /// 返回前端展示用的中文/英文标签。
    pub fn label(&self) -> &str {
        match self {
            OverallStatus::NotStarted => "Overall: Not Started",
            OverallStatus::Designing => "Overall: Designing",
            OverallStatus::Reviewing => "Overall: Reviewing",
            OverallStatus::PendingApproval => "Overall: Pending Emperor Approval",
            OverallStatus::Rejected => "Overall: Issues Found in Review",
            OverallStatus::Approved => "Overall: Approved",
        }
    }
}

// ── PhaseRuntime — 单阶段状态 ──

/// 单个阶段的运行时状态，包含设计和执行两个并行的进度维度。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseRuntime {
    /// 阶段序号（0-based）
    pub index: u32,

    /// 设计维度状态（中书令 + 门下侍中 负责）
    pub design: PhaseDesignStatus,

    /// 执行维度状态（吏部 → 兵部 → 工部 → 刑部 → 礼部 负责）
    pub execution: PhaseExecutionStatus,
}

// ── PhaseDesignStatus — 设计维度状态 ──

/// 单阶段设计进度状态（与 OverallStatus 类似但作用域为单个阶段）。
///
/// 负责方：中书令（设计） → 门下侍中（审查） → 皇帝（批准）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhaseDesignStatus {
    NotStarted,
    Designing,
    Reviewing,
    PendingApproval,
    Rejected,
    Approved,
}

impl PhaseDesignStatus {
    /// 返回带阶段编号的展示标签，如 "Phase 2 Design: Reviewing"
    pub fn label(&self, phase: u32) -> String {
        match self {
            PhaseDesignStatus::NotStarted => format!("Phase {} Design: Not Started", phase),
            PhaseDesignStatus::Designing => format!("Phase {} Design: Designing", phase),
            PhaseDesignStatus::Reviewing => format!("Phase {} Design: Reviewing", phase),
            PhaseDesignStatus::PendingApproval => {
                format!("Phase {} Design: Pending Emperor Approval", phase)
            }
            PhaseDesignStatus::Rejected => {
                format!("Phase {} Design: Issues Found in Review", phase)
            }
            PhaseDesignStatus::Approved => format!("Phase {} Design: Approved", phase),
        }
    }
}

// ── PhaseExecutionStatus — 执行维度状态 ──

/// 单阶段执行进度状态，按六部流水线推进。
///
/// 状态流转（TDD 批次循环）:
/// ```text
/// NotStarted → TaskBreakdown → Testing → Implementing → Checking
///                                                 ↓
///                                      Standards → Logging → Completed
/// ```
///
/// 异常分支：Blocked（阻塞）/ MinorIssue（小问题，不阻塞但需记录）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhaseExecutionStatus {
    NotStarted,                 // 尚未开始执行
    TaskBreakdown,              // 吏部：详细任务分解
    Testing,                    // 兵部：编写测试 + 接口契约
    Implementing,               // 工部：TDD 编码实现
    Checking,                   // 刑部：运行测试验证
    Standards,                  // 礼部：规范合规检查
    Logging,                    // 完成事项记录（交付前最后一步）
    Blocked { reason: String }, // 执行受阻（需人工排查）
    MinorIssue,                 // 发现小问题，不阻塞但需标记
    Completed,                  // 执行完成
}

impl PhaseExecutionStatus {
    /// 返回带阶段编号的展示标签，如 "Phase 1 Execution: Coding"
    pub fn label(&self, phase: u32) -> String {
        match self {
            PhaseExecutionStatus::NotStarted => format!("Phase {} Execution: Not Started", phase),
            PhaseExecutionStatus::TaskBreakdown => {
                format!("Phase {} Execution: Task Breakdown", phase)
            }
            PhaseExecutionStatus::Testing => format!("Phase {} Execution: Testing", phase),
            PhaseExecutionStatus::Implementing => format!("Phase {} Execution: Coding", phase),
            PhaseExecutionStatus::Checking => format!("Phase {} Execution: Checking", phase),
            PhaseExecutionStatus::Standards => {
                format!("Phase {} Execution: Standards Check", phase)
            }
            PhaseExecutionStatus::Logging => format!("Phase {} Execution: Logging", phase),
            PhaseExecutionStatus::Blocked { .. } => format!("Phase {} Execution: Blocked", phase),
            PhaseExecutionStatus::MinorIssue => format!("Phase {} Execution: Minor Issue", phase),
            PhaseExecutionStatus::Completed => format!("Phase {} Execution: Completed", phase),
        }
    }
}
