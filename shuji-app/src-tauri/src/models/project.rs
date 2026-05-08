use serde::{Deserialize, Serialize};

use crate::state_machine::states::ProjectState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub working_dir: String,
    pub state: ProjectState,
    pub overall: OverallStatus,
    pub phases: Vec<PhaseRuntime>,
    pub phase_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub working_dir: String,
    pub created_at: String,
    pub overall_status: String,
    pub phases_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OverallStatus {
    NotStarted,
    Designing,
    Reviewing,
    PendingApproval,
    Rejected(u32),
    Escalated,
    Approved,
}

impl OverallStatus {
    pub fn label(&self) -> &str {
        match self {
            OverallStatus::NotStarted => "整体方案：未开始",
            OverallStatus::Designing => "整体方案：设计中",
            OverallStatus::Reviewing => "整体方案：审查中",
            OverallStatus::PendingApproval => "整体方案：待皇帝审批",
            OverallStatus::Rejected(n) => {
                if *n >= 3 {
                    "整体方案：驳回升级"
                } else {
                    "整体方案：驳回待改"
                }
            }
            OverallStatus::Escalated => "整体方案：驳回升级",
            OverallStatus::Approved => "整体方案：已批准",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseRuntime {
    pub index: u32,
    pub design: PhaseDesignStatus,
    pub execution: PhaseExecutionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhaseDesignStatus {
    NotStarted,
    Designing,
    Reviewing,
    PendingApproval,
    Rejected(u32),
    Escalated,
    Approved,
}

impl PhaseDesignStatus {
    pub fn label(&self, phase: u32) -> String {
        match self {
            PhaseDesignStatus::NotStarted => format!("阶段{}设计：未开始", phase),
            PhaseDesignStatus::Designing => format!("阶段{}设计：设计中", phase),
            PhaseDesignStatus::Reviewing => format!("阶段{}设计：审查中", phase),
            PhaseDesignStatus::PendingApproval => format!("阶段{}设计：待皇帝审批", phase),
            PhaseDesignStatus::Rejected(n) => {
                if *n >= 3 {
                    format!("阶段{}设计：驳回升级", phase)
                } else {
                    format!("阶段{}设计：驳回待改", phase)
                }
            }
            PhaseDesignStatus::Escalated => format!("阶段{}设计：驳回升级", phase),
            PhaseDesignStatus::Approved => format!("阶段{}设计：已批准", phase),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhaseExecutionStatus {
    NotStarted,
    TaskBreakdown,
    Testing,
    Implementing,
    Checking,
    Standards,
    Logging,
    Blocked { reason: String },
    MinorIssue,
    Completed,
}

impl PhaseExecutionStatus {
    pub fn label(&self, phase: u32) -> String {
        match self {
            PhaseExecutionStatus::NotStarted => format!("阶段{}执行：未开始", phase),
            PhaseExecutionStatus::TaskBreakdown => format!("阶段{}执行：吏部拆解任务", phase),
            PhaseExecutionStatus::Testing => format!("阶段{}执行：兵部测试", phase),
            PhaseExecutionStatus::Implementing => format!("阶段{}执行：工部编码", phase),
            PhaseExecutionStatus::Checking => format!("阶段{}执行：刑部检查", phase),
            PhaseExecutionStatus::Standards => format!("阶段{}执行：礼部检查", phase),
            PhaseExecutionStatus::Logging => format!("阶段{}执行：户部记录", phase),
            PhaseExecutionStatus::Blocked { .. } => format!("阶段{}执行：阻塞", phase),
            PhaseExecutionStatus::MinorIssue => format!("阶段{}执行：轻微问题", phase),
            PhaseExecutionStatus::Completed => format!("阶段{}执行：已完成", phase),
        }
    }
}
