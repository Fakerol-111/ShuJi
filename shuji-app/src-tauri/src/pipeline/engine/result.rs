//! Internal step result (not yet converted to PipelineResult).

pub(crate) enum StepResultInner {
    Success {
        artifact_id: Option<String>,
        target_role: Option<String>,
    },
    ApprovalRequired {
        doc_id: String,
    },
    AwaitingUserInput {
        question: String,
    },
    Failed {
        reason: String,
    },
}
