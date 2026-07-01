//! Max-iteration wrap-up for the tool-iteration loop.
//!
//! Extracted from `AgentController::run()`. When the loop exhausts its
//! iteration budget without the model returning a text-only response, this
//! path gives the model one extra API round-trip to produce a coherent
//! summary (and optionally route to 尚书令 for further authorization).
//!
//! Behavior is preserved bit-for-bit in this migration:
//! - `session.set_reasoning(true)` is called before the extra step.
//! - The injected wrap-up prompt is the same string (mojibake and all —
//!   fixing that is a separate cleanup, not part of this refactor).
//! - Route detection reuses `routing::detect_route` with `my_role = None`
//!   (the wrap-up path skips self-routing detection — legacy quirk).
//! - The final text is composed differently depending on whether
//!   `last_text` is empty.

use crate::api::session::{Session, StepResult};

use super::routing::{detect_route, RouteOutcome};
use super::watchdog::WatchdogState;
use super::{RunResult, ToolFuture};

impl super::AgentController {
    /// Max-iteration wrap-up: inject a summary request, do one extra API
    /// step, and either return `Routed` (if the model called `route_to`)
    /// or `Done` (with the work summary appended to `last_text`).
    ///
    /// `wd` is borrowed read-only to build the human-readable reason string
    /// (`WatchdogState::max_iter_reason`). `last_text` is the accumulated
    /// assistant text from the main loop. `tool_exec` is the same tool
    /// executor used in the main loop.
    pub(super) async fn wrap_up_max_iterations(
        &mut self,
        session: &mut Session,
        tool_exec: &(dyn for<'a> Fn(&'a str, &'a serde_json::Value) -> ToolFuture + Sync),
        wd: &WatchdogState,
        last_text: &str,
        max_iter: usize,
    ) -> anyhow::Result<RunResult> {
        log_console!("[control] tool-call limit ({}) reached", max_iter);
        let reason = wd.max_iter_reason(max_iter);

        // ── Graceful wrap-up: inject summary request, do one extra step ────
        // Instead of silently cutting off, give the LLM a chance to produce
        // a coherent wrap-up and route to its superior for authorization.
        session.set_reasoning(true);
        let wrap_up = format!(
            "\n\n---\n[System] Tool call limit reached ({} calls). {}. \
             \nPlease immediately summarize the work completed and any difficulties encountered. If you need to continue, call route_to to route to 灏氫功浠? explaining the reason and requesting further authorization. \
             \nIf no routing is needed, directly output the work summary.\n---",
            max_iter, reason,
        );
        session.inject(&wrap_up);

        let final_text = match session.step().await? {
            StepResult::Text(t) => t,
            StepResult::ToolCalls { calls, text } => {
                let combined = text;
                // Execute first tool call — check if it's a route_to.
                if let Some(tc) = calls.first() {
                    let result = tool_exec(&tc.name, &tc.args).await;
                    self.collect_document_from_tool(&tc.name, &result);

                    // ── Route detection (wrap-up variant) ─────────────────
                    // Same parser as the main loop, but `my_role` is None:
                    // the wrap-up path skips self-routing detection (legacy
                    // quirk) and lets unknown targets fall through to feed
                    // the raw result for context.
                    match detect_route(&result, &tc.args, None) {
                        RouteOutcome::Route(route) => {
                            log_console!(
                                "[control] max-iter wrap-up: routed to {} ({})",
                                route.target.name(),
                                route.subject,
                            );
                            return Ok(RunResult::Routed {
                                text: combined,
                                route,
                            });
                        }
                        // SelfRoute can't happen here (my_role is None).
                        // UnknownTarget + NotRoute both fall through to feed
                        // the raw result for context.
                        RouteOutcome::SelfRoute { .. }
                        | RouteOutcome::UnknownTarget { .. }
                        | RouteOutcome::NotRoute => {
                            session.feed_tool_result(&tc.id, &tc.name, &result);
                        }
                    }
                }
                combined
            }
        };

        let result = if last_text.is_empty() {
            format!(
                "Tool call limit reached ({}). Last response:\n{}",
                max_iter, final_text
            )
        } else {
            format!(
                "{}\n\n---\nTool call limit reached ({}). Work summary:\n{}",
                last_text, max_iter, final_text
            )
        };
        Ok(RunResult::Done(result))
    }
}
