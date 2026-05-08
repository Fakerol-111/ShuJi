use serde::{Deserialize, Serialize};

/// Legacy linear state — used for project history display.
/// Runtime logic uses the parallel state model in models::project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProjectState {
    GoalReceived,
    Clarifying,
    RequirementsClear,
    OverallDesign,
    OverallReview,
    OverallPending,
    OverallRejected,
    OverallEscalated,
    OverallApproved,
    PhaseDetailDesign,
    PhaseDesignReview,
    PhaseDesignPending,
    PhaseDesignRejected,
    PhaseDesignEscalated,
    PhaseDesignApproved,
    PhaseExecuting,
    ExecutionFeedback,
    MinorIssue,
    IssueAwaitingEmperor,
    IssueDesignChange,
    ExecutionBlocked,
    Delivered,
    Paused,
    Terminated,
}

impl ProjectState {
    pub fn label(&self) -> &str {
        match self {
            ProjectState::GoalReceived => "目标已接收",
            ProjectState::Clarifying => "需求澄清中",
            ProjectState::RequirementsClear => "需求已明确",
            ProjectState::OverallDesign => "整体方案设计中",
            ProjectState::OverallReview => "整体方案审查中",
            ProjectState::OverallPending => "整体方案待批",
            ProjectState::OverallRejected => "整体方案驳回待改",
            ProjectState::OverallEscalated => "整体方案驳回升级",
            ProjectState::OverallApproved => "整体方案已批准",
            ProjectState::PhaseDetailDesign => "阶段详细设计中",
            ProjectState::PhaseDesignReview => "阶段设计审查中",
            ProjectState::PhaseDesignPending => "阶段设计待批",
            ProjectState::PhaseDesignRejected => "阶段设计驳回待改",
            ProjectState::PhaseDesignEscalated => "阶段设计驳回升级",
            ProjectState::PhaseDesignApproved => "阶段设计已批准",
            ProjectState::PhaseExecuting => "阶段执行中",
            ProjectState::ExecutionFeedback => "执行反馈中",
            ProjectState::MinorIssue => "执行轻微问题",
            ProjectState::IssueAwaitingEmperor => "问题待皇帝决策",
            ProjectState::IssueDesignChange => "问题修改设计中",
            ProjectState::ExecutionBlocked => "执行阻塞",
            ProjectState::Delivered => "已交付",
            ProjectState::Paused => "已暂停",
            ProjectState::Terminated => "已终止",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, ProjectState::Delivered | ProjectState::Terminated)
    }
}
