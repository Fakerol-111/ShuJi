//! Shangshuling special tools: assign tasks to the six ministries and wait for replies.
//!
//! `assign_task` is Shangshuling's core dispatch tool. It sends an ActorMessage to the target department,
//! blocks via the reply_to channel for the execution result, and records edges in the workflow graph.

use crate::actor::ActorMessage;
use crate::api::control::RouteMsgType;
use crate::models::role::Role;
use crate::tool::ToolContext;
use tokio::sync::mpsc;

/// Shangshuling special tool dispatcher. Processes assign_task if matched, otherwise returns None.
pub async fn tool_handle_shangshuling_special(
    name: &str,
    args: &serde_json::Value,
    ctx: &ToolContext,
) -> Option<String> {
    match name {
        "assign_task" => Some(tool_assign_task(args, ctx).await),
        _ => None,
    }
}

/// Assign a task to the target department, block until completion, return execution result.
///
/// Flow:
/// 1. Validate target department and task description
/// 2. Get target department's Actor channel from ctx.peers
/// 3. Create reply_to channel to wait for reply
/// 4. Send ActorMessage, record workflow graph edge
/// 5. Block until department completes
/// 6. Return structured result (including department output)
async fn tool_assign_task(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let target_name = args["to"].as_str().unwrap_or("");
    let task = args["task"].as_str().unwrap_or("");

    if target_name.is_empty() {
        return serde_json::json!({
            "ok": false,
            "operation": "assign_task",
            "message": "Missing target department (to parameter)",
            "error_code": "missing_target"
        })
        .to_string();
    }
    if task.is_empty() {
        return serde_json::json!({
            "ok": false,
            "operation": "assign_task",
            "message": "Missing task description (task parameter)",
            "error_code": "missing_task"
        })
        .to_string();
    }

    let role = match Role::from_name(target_name) {
        Some(r) => r,
        None => {
            return serde_json::json!({
                "ok": false,
                "operation": "assign_task",
                "message": format!("Unknown department: {}", target_name),
                "error_code": "unknown_department"
            })
            .to_string();
        }
    };

    let peers = match ctx.peers.as_ref() {
        Some(p) => p,
        None => {
            return serde_json::json!({
                "ok": false,
                "operation": "assign_task",
                "message": "Dispatch system not initialized (peers unavailable)",
                "error_code": "no_peers"
            })
            .to_string();
        }
    };

    let tx = match peers.get(&role) {
        Some(tx) => tx,
        None => {
            return serde_json::json!({
                "ok": false,
                "operation": "assign_task",
                "message": format!("Cannot find channel for {}", target_name),
                "error_code": "channel_not_found"
            })
            .to_string();
        }
    };

    // Create reply channel
    let (output_tx, mut output_rx) = mpsc::unbounded_channel::<String>();

    let msg = ActorMessage {
        msg_type: RouteMsgType::Task,
        subject: task.to_string(),
        payload: None,
        doc_ids: Vec::new(),
        reply_to: Some(output_tx),
    };

    log_console!(
        "[shangshuling] assign_task: {} <- assigned task: {}",
        target_name,
        task.chars().take(60).collect::<String>()
    );

    // Workflow graph: record edge from Shangshuling -> target department
    if let Some(ref graph_lock) = ctx.workflow_graph {
        let mut g = graph_lock.lock().await;
        let step_id = format!("assign_{}", role.name());
        g.add_edge("Shangshuling", target_name, &step_id, task);
        g.mark_active(target_name);
        let _ = g.save_to(&ctx.working_dir).await;
    }

    if let Err(e) = tx.send(msg) {
        return serde_json::json!({
            "ok": false,
            "operation": "assign_task",
            "message": format!("Failed to send message to {}: {}", target_name, e),
            "error_code": "send_failed"
        })
        .to_string();
    }

    // Wait for department to complete
    match output_rx.recv().await {
        Some(output) => {
            let preview = output.chars().take(200).collect::<String>();
            log_console!(
                "[shangshuling] assign_task: {} completed: {}",
                target_name,
                preview
            );

            // Workflow graph: mark completed
            if let Some(ref graph_lock) = ctx.workflow_graph {
                let mut g = graph_lock.lock().await;
                g.mark_completed(target_name);
                let _ = g.save_to(&ctx.working_dir).await;
            }

            serde_json::json!({
                "ok": true,
                "operation": "assign_task",
                "department": target_name,
                "message": format!("{} task completed", target_name),
                "result": output
            })
            .to_string()
        }
        None => {
            log_console!(
                "[shangshuling] assign_task: {} channel unexpectedly closed",
                target_name
            );
            serde_json::json!({
                "ok": false,
                "operation": "assign_task",
                "message": format!("{} channel unexpectedly closed during execution", target_name),
                "error_code": "channel_closed"
            })
            .to_string()
        }
    }
}
