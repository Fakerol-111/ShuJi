//! Tool execution helpers for the tool-iteration loop.
//!
//! Extracted from `AgentController::run()`. Four free functions:
//!
//! - `execute_read_tools_parallel` — runs all read-only tools in a batch
//!   concurrently via `join_all`, returning a map idx → result. Writes are
//!   NOT executed here; they run serially in the main loop to avoid file
//!   contention.
//! - `feed_cancelled_remaining_tools` — feeds "cancelled" results for the
//!   tail of a batch when fast-cancel / route / error short-circuits the
//!   loop. The assistant message's tool_calls must be balanced with
//!   tool_results, otherwise the next API request returns 400.
//! - `emit_tool_call` / `emit_tool_result` — thin wrappers around the
//!   `DeptStepCallback` emitter for the two per-tool step events.
//!
//! Behavior is preserved bit-for-bit in this migration: same join_all
//! strategy, same "Cancelled: ..." strings, same 200-char summary truncation.

use std::collections::HashMap;

use futures::future::join_all;

use crate::api::session::{Session, ToolCallInfo};
use crate::models::dept_step::DeptStepKind;

use super::iterations::is_read_tool;
use super::{DeptStepCallback, ToolFuture};

/// Parallel-execute all read-only tools in a batch concurrently.
///
/// Returns a map from call index → result string. Call indices that don't
/// correspond to a read tool are simply absent from the map; the main loop
/// looks them up with `read_results.remove(&idx)` and falls back to serial
/// `tool_exec` for writes.
///
/// `tool_exec` is `Sync` (it's a function pointer wrapped in a closure),
/// so it's safe to call from multiple futures concurrently.
pub(super) async fn execute_read_tools_parallel(
    calls: &[ToolCallInfo],
    tool_exec: &(dyn for<'a> Fn(&'a str, &'a serde_json::Value) -> ToolFuture + Sync),
) -> HashMap<usize, String> {
    let mut read_results: HashMap<usize, String> = HashMap::new();
    let read_futures: Vec<_> = calls
        .iter()
        .enumerate()
        .filter(|(_, tc)| is_read_tool(&tc.name))
        .map(|(idx, tc)| {
            let name = tc.name.clone();
            let args = tc.args.clone();
            let fut = tool_exec(&name, &args);
            async move { (idx, fut.await) }
        })
        .collect();
    for (idx, result) in join_all(read_futures).await {
        read_results.insert(idx, result);
    }
    read_results
}

/// Feed "cancelled" results for all tool calls in the batch starting at
/// `from_idx` (inclusive). Used by:
///
/// - fast-cancel before a tool run → `from_idx = idx` (the current tool)
/// - route_to mid-batch → `from_idx = idx + 1` (the route tool already got
///   its real result fed by the caller)
/// - consecutive-error force-stop → `from_idx = idx + 1` (same)
///
/// The `reason` string is what the LLM sees as the tool result. It's phrased
/// for the model, not for the user.
pub(super) fn feed_cancelled_remaining_tools(
    session: &mut Session,
    calls: &[ToolCallInfo],
    from_idx: usize,
    reason: &str,
) {
    for remaining in &calls[from_idx..] {
        session.feed_tool_result(&remaining.id, &remaining.name, reason);
    }
}

/// Emit a `ToolCall` step event if an emitter is registered.
///
/// `emit` is typically `self.step_emit.as_ref()` from the controller.
pub(super) fn emit_tool_call(emit: Option<&DeptStepCallback>, tc: &ToolCallInfo) {
    if let Some(emit) = emit {
        emit(DeptStepKind::ToolCall {
            tool: tc.name.clone(),
            args: tc.args.clone(),
        });
    }
}

/// Emit a `ToolResult` step event if an emitter is registered.
///
/// `is_error` is computed the same way as the original inline code: parse
/// the result JSON, look for `ok: false`. If the result isn't valid JSON or
/// has no `ok` field, `is_error` defaults to `false` (matching the original
/// `.unwrap_or(false)`).
///
/// `summary` is the first 200 chars of the result string.
pub(super) fn emit_tool_result(emit: Option<&DeptStepCallback>, tool_name: &str, result: &str) {
    if let Some(emit) = emit {
        let is_error = serde_json::from_str::<serde_json::Value>(result)
            .ok()
            .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
            .map(|ok| !ok)
            .unwrap_or(false);
        let summary: String = result.chars().take(200).collect();
        emit(DeptStepKind::ToolResult {
            tool: tool_name.to_string(),
            ok: !is_error,
            summary,
        });
    }
}
