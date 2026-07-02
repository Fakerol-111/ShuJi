//! PipelineEngine resume logic.
//!
//! Extracted from `mod.rs`. `resume_with_input()` is the main entry point;
//! `verify_manual_approval_before_resume()` is an extracted private helper
//! for the manual-approval gate check.

use crate::pipeline::{PipelineResult, StepStatus};

use super::PipelineEngine;

/// Check whether the upstream revw document for the given step has been
/// approved by the emperor. Returns `Ok(None)` if approved, or
/// `Ok(Some(doc_id))` if still awaiting approval, or `Err(...)` if the
/// document is missing/empty.
async fn verify_manual_approval_before_resume(
    engine: &PipelineEngine,
    step_id: &str,
) -> Result<Option<String>, PipelineResult> {
    let step = engine
        .runtime
        .plan
        .steps
        .iter()
        .find(|s| s.step_id == step_id);
    let step = match step {
        Some(s) if s.action == "approval_gate" => s,
        _ => return Ok(None), // Not an approval gate step — nothing to verify
    };

    let doc_id = crate::pipeline::artifacts::approval_doc_from_upstream(
        &engine.runtime.artifacts,
        &step.depends_on,
    );
    let Some(doc_id) = doc_id.filter(|id| !id.is_empty()) else {
        log_console!("[pipeline] approval_gate failed: no upstream revw document artifact");
        return Err(PipelineResult::StepFailed {
            step_id: step_id.to_string(),
            reason: "approval_gate requires an upstream revw document, but none was found.".into(),
            runtime: engine.runtime.clone(),
        });
    };

    // Reject empty revw on resume path as well
    if doc_id.starts_with("revw_") {
        if let Some(body) =
            crate::tool::documents::get_document_body(&engine.project_dir, &doc_id).await
        {
            if body.trim().is_empty() {
                log_console!(
                    "[pipeline] approval_gate blocked: doc {} body is empty",
                    doc_id
                );
                return Err(PipelineResult::StepFailed {
                    step_id: step_id.to_string(),
                    reason: format!(
                        "approval_gate requires a non-empty revw document body, but {} is empty.",
                        doc_id
                    ),
                    runtime: engine.runtime.clone(),
                });
            }
        }
    }

    let status = crate::tool::documents::get_document_status(&engine.project_dir, &doc_id).await;
    if status.as_deref() != Some("approved") {
        log_console!(
            "[pipeline] approval_gate blocked: doc {} status={:?}",
            doc_id,
            status
        );
        return Ok(Some(doc_id));
    }

    Ok(None) // Approved
}

impl PipelineEngine {
    /// Resume pipeline after user input or approval decision.
    ///
    /// For `AwaitingUserInput`: marks the current `ask_user` step as Done
    /// and records user_input as an artifact, then continues remaining steps.
    ///
    /// For `AwaitingApproval`: marks the current `approval_gate` step as Done,
    /// then continues.
    pub async fn resume_with_input(mut self, user_input: Option<&str>) -> PipelineResult {
        // Get the current step that was waiting
        let current = self.runtime.current_step.clone();
        let current = match current {
            Some(ref id) => id.clone(),
            None => return self.run().await, // No pending step, just continue
        };

        // approval_gate: in manual mode, verify document is actually approved before proceeding
        if self.runtime_config.approval.mode == crate::config::ApprovalMode::Manual {
            match verify_manual_approval_before_resume(&self, &current).await {
                Err(result) => return result,
                Ok(Some(doc_id)) => {
                    return PipelineResult::AwaitingApproval {
                        doc_id,
                        step_id: current,
                        runtime: self.runtime.clone(),
                    };
                }
                Ok(None) => {} // Approved — continue
            }
        }

        // Mark current step Done so find_executable_step can proceed
        self.set_status(&current, StepStatus::Done);

        // Record user input as artifact if provided
        if let Some(input) = user_input {
            self.runtime
                .artifacts
                .insert(format!("{}.user_input", current), input.to_string());
            log_console!("[pipeline] resume step {} with user input", current);
        }

        self.save().await.ok();
        self.run().await
    }
}
