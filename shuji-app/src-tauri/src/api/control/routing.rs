//! Route detection for the tool-iteration loop.
//!
//! Extracted from `AgentController::run()` and the max-iter wrap-up path.
//! Both call sites used to inline their own copy of the "is this a route_to?
//! who is the target? build the RouteTo struct" dance. This module gives them
//! one shared parser.
//!
//! Behavior is preserved bit-for-bit in this migration:
//! - main loop: self-routing → feed error + continue; unknown target → feed
//!   error + continue; valid target → return `Routed`.
//! - wrap-up: self-routing is *not* checked (legacy quirk); unknown target
//!   falls through to "feed result for context".
//!
//! The split is expressed via the `my_role: Option<&str>` argument to
//! `detect_route`: main loop passes `Some(role)`, wrap-up passes `None`.

use serde_json::Value;

use crate::models::role::Role;

use super::role_from_name;
use super::types::{route_msg_type_from_str, RouteMsgType, RouteTo};

/// Outcome of inspecting a tool result for a `route_to` operation.
pub(super) enum RouteOutcome {
    /// The tool result is not a route_to operation.
    NotRoute,
    /// route_to to the *current* role. Only produced when the caller passed
    /// `Some(my_role)` to `detect_route`. The caller feeds an error message
    /// to the session and continues the loop.
    SelfRoute { my_role: String },
    /// route_to to a name that doesn't resolve to a known role. The caller
    /// feeds an error message and continues (main loop) or falls through
    /// to feed the raw result (wrap-up).
    UnknownTarget { to_name: String },
    /// route_to to a valid target role. The caller feeds the route result,
    /// feeds dummy results for any remaining tool calls in the batch, and
    /// returns `RunResult::Routed`.
    Route(RouteTo),
}

/// Inspect a tool result + args for a route_to operation.
///
/// `my_role`:
/// - `Some(role)` — enable self-routing detection (main loop).
/// - `None` — skip self-routing check (wrap-up preserves legacy behavior).
pub(super) fn detect_route(result: &str, args: &Value, my_role: Option<&str>) -> RouteOutcome {
    let Ok(v) = serde_json::from_str::<Value>(result) else {
        return RouteOutcome::NotRoute;
    };
    if v["operation"].as_str() != Some("route_to") {
        return RouteOutcome::NotRoute;
    }

    let to_name = args["to"].as_str().unwrap_or("");

    // Self-routing check — only when the caller asked for it.
    if let Some(my_role) = my_role {
        if role_from_name(to_name).is_some_and(|r| r.name() == my_role) {
            return RouteOutcome::SelfRoute {
                my_role: my_role.to_string(),
            };
        }
    }

    let Some(target) = role_from_name(to_name) else {
        return RouteOutcome::UnknownTarget {
            to_name: to_name.to_string(),
        };
    };

    RouteOutcome::Route(build_route(target, args))
}

/// Build a `RouteTo` from already-validated args.
///
/// Splits the `subject` into a task subject and any embedded doc IDs using
/// the same helper as the rest of the pipeline. `msg_type` defaults to
/// `Task` when the `type` field is missing or unrecognized (matches the
/// original inline `unwrap_or(RouteMsgType::Task)`).
fn build_route(target: Role, args: &Value) -> RouteTo {
    let msg_type = route_msg_type_from_str(args["type"].as_str().unwrap_or("task"))
        .unwrap_or(RouteMsgType::Task);
    let subject_raw = args["subject"].as_str().unwrap_or("").to_string();
    let payload = args
        .get("inline")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let (task_subject, doc_ids) =
        crate::pipeline::artifacts::split_route_task_and_doc_ids(&subject_raw, payload.as_deref());
    RouteTo {
        target,
        msg_type,
        subject: task_subject,
        payload,
        doc_ids,
    }
}

/// Human-readable one-line summary of a routed result.
///
/// Format: `Routed to {target} ({kind}): {subject} [docs: ...]`
/// Used by the main loop to build the `RunResult::Routed.text` payload.
/// The wrap-up path does not use this — it returns the raw `text` from the
/// assistant message.
pub(super) fn route_summary(route: &RouteTo) -> String {
    let kind = match route.msg_type {
        RouteMsgType::Task => "new task",
        RouteMsgType::Replace => "replace",
        RouteMsgType::Interrupt => "interrupt",
    };
    let docs = if route.doc_ids.is_empty() {
        String::new()
    } else {
        format!(" [docs: {}]", route.doc_ids.join(", "))
    };
    format!(
        "Routed to {} ({}): {}{}",
        route.target.name(),
        kind,
        route.subject,
        docs
    )
}
