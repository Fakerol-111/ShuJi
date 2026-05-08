use crate::state_machine::states::ProjectState;

pub struct StateMachine;

impl StateMachine {
    /// Returns true if a transition from `from` to `to` is valid.
    pub fn can_transition(from: &ProjectState, to: &ProjectState) -> bool {
        use ProjectState::*;
        matches!(
            (from, to),
            // Goal → Clarify
            (GoalReceived, Clarifying)
            | (GoalReceived, OverallDesign) // skip clarify

            // Clarify cycle
            | (Clarifying, RequirementsClear)
            | (Clarifying, GoalReceived)

            // Requirements → Design
            | (RequirementsClear, OverallDesign)

            // Overall design → Review
            | (OverallDesign, OverallReview)

            // Review → Pending / Rejected
            | (OverallReview, OverallPending)
            | (OverallReview, OverallRejected)

            // Rejected → redesign
            | (OverallRejected, OverallDesign)
            // Rejected → Escalated (3 strikes)
            | (OverallRejected, OverallEscalated)

            // Escalated → Pending (emperor rules)
            | (OverallEscalated, OverallPending)

            // Pending → Approved / Rejected
            | (OverallPending, OverallApproved)
            | (OverallPending, OverallRejected)

            // Approved → Phase detail design
            | (OverallApproved, PhaseDetailDesign)

            // Phase design → Review
            | (PhaseDetailDesign, PhaseDesignReview)

            // Review → Pending / Rejected
            | (PhaseDesignReview, PhaseDesignPending)
            | (PhaseDesignReview, PhaseDesignRejected)

            // Rejected → redesign
            | (PhaseDesignRejected, PhaseDetailDesign)
            | (PhaseDesignRejected, PhaseDesignEscalated)

            // Escalated → Pending
            | (PhaseDesignEscalated, PhaseDesignPending)

            // Pending → Approved / Rejected
            | (PhaseDesignPending, PhaseDesignApproved)
            | (PhaseDesignPending, PhaseDesignRejected)

            // Approved → Execute
            | (PhaseDesignApproved, PhaseExecuting)

            // Execute → feedback / minor / blocked
            | (PhaseExecuting, ExecutionFeedback)
            | (PhaseExecuting, MinorIssue)
            | (PhaseExecuting, ExecutionBlocked)
            | (PhaseExecuting, PhaseDetailDesign) // feedback → redesign
            | (PhaseExecuting, Delivered)

            // Feedback → emperor / self-fix
            | (ExecutionFeedback, IssueAwaitingEmperor)
            | (ExecutionFeedback, MinorIssue)

            // Minor → continue or escalate
            | (MinorIssue, PhaseExecuting)
            | (MinorIssue, IssueAwaitingEmperor)

            // Emperor issue → redesign / continue
            | (IssueAwaitingEmperor, IssueDesignChange)
            | (IssueAwaitingEmperor, PhaseExecuting)

            // Redesign → back to review
            | (IssueDesignChange, PhaseDesignReview)

            // Blocked → emperor / redesign / continue
            | (ExecutionBlocked, IssueAwaitingEmperor)
            | (ExecutionBlocked, PhaseDetailDesign)

            // Multi-phase: after phase design approved, can execute previous phase
            // while designing next phase — handled by orchestrator logic

            // Pause / Resume / Terminate (any → Paused, any → Terminated)
            | (_, Paused) | (_, Terminated)

            // Resume
            | (Paused, GoalReceived)
            | (Paused, OverallDesign)
            | (Paused, PhaseDetailDesign)
            | (Paused, PhaseExecuting)
        )
    }

    /// Returns all valid target states from `state`.
    pub fn valid_transitions(state: &ProjectState) -> Vec<ProjectState> {
        use ProjectState::*;
        let all = vec![
            // Normal flow states
            GoalReceived, Clarifying, RequirementsClear,
            OverallDesign, OverallReview, OverallPending,
            OverallRejected, OverallEscalated, OverallApproved,
            PhaseDetailDesign, PhaseDesignReview, PhaseDesignPending,
            PhaseDesignRejected, PhaseDesignEscalated, PhaseDesignApproved,
            PhaseExecuting, ExecutionFeedback, MinorIssue,
            IssueAwaitingEmperor, IssueDesignChange, ExecutionBlocked,
            Delivered,
            // Terminal/control
            Paused, Terminated,
        ];
        all.into_iter()
            .filter(|to| Self::can_transition(state, to))
            .collect()
    }
}
