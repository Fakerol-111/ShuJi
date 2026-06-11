//! 尚书省特殊工具：向六部分派任务并等待回复。
//!
//! `assign_task` 是尚书省的核心调度工具。它向目标部门发送 ActorMessage，
//! 通过 reply_to 通道阻塞等待执行结果，并在 workflow graph 中记录边。

use crate::actor::ActorMessage;
use crate::api::control::RouteMsgType;
use crate::models::role::Role;
use crate::tool::ToolContext;
use tokio::sync::mpsc;

/// 尚书省特殊工具分发器。匹配到 assign_task 则处理，否则返回 None。
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

/// 向目标部门分派任务，阻塞等待完成，返回执行结果。
///
/// 流程：
/// 1. 验证目标部门和任务描述
/// 2. 从 ctx.peers 获取目标部门的 Actor 信道
/// 3. 创建 reply_to 通道等待回复
/// 4. 发送 ActorMessage，记录文移图边
/// 5. 阻塞等待部门完成
/// 6. 返回结构化结果（含部门产出内容）
async fn tool_assign_task(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let target_name = args["to"].as_str().unwrap_or("");
    let task = args["task"].as_str().unwrap_or("");

    if target_name.is_empty() {
        return serde_json::json!({
            "ok": false,
            "operation": "assign_task",
            "message": "缺少目标部门（to 参数)",
            "error_code": "missing_target"
        })
        .to_string();
    }
    if task.is_empty() {
        return serde_json::json!({
            "ok": false,
            "operation": "assign_task",
            "message": "缺少任务描述（task 参数)",
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
                "message": format!("未知部门: {}", target_name),
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
                "message": "调度系统未初始化（peers 不可用）",
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
                "message": format!("找不到 {} 的信道", target_name),
                "error_code": "channel_not_found"
            })
            .to_string();
        }
    };

    // 创建回复通道
    let (output_tx, mut output_rx) = mpsc::unbounded_channel::<String>();

    let msg = ActorMessage {
        msg_type: RouteMsgType::Task,
        subject: task.to_string(),
        payload: None,
        reply_to: Some(output_tx),
    };

    log_console!(
        "[shangshuling] assign_task: {} ← 分派任务: {}",
        target_name,
        task.chars().take(60).collect::<String>()
    );

    // ── 文移图：记录尚书省 → 目标部门边 ──
    if let Some(ref graph_lock) = ctx.workflow_graph {
        let mut g = graph_lock.lock().await;
        let step_id = format!("assign_{}", role.name());
        g.add_edge("尚书令", target_name, &step_id, task);
        g.mark_active(target_name);
        let _ = g.save_to(&ctx.working_dir).await;
    }

    if let Err(e) = tx.send(msg) {
        return serde_json::json!({
            "ok": false,
            "operation": "assign_task",
            "message": format!("发送消息到 {} 失败: {}", target_name, e),
            "error_code": "send_failed"
        })
        .to_string();
    }

    // 等待部门完成
    match output_rx.recv().await {
        Some(output) => {
            let preview = output.chars().take(200).collect::<String>();
            log_console!(
                "[shangshuling] assign_task: {} 完成: {}",
                target_name,
                preview
            );

            // ── 文移图：标记完成 ──
            if let Some(ref graph_lock) = ctx.workflow_graph {
                let mut g = graph_lock.lock().await;
                g.mark_completed(target_name);
                let _ = g.save_to(&ctx.working_dir).await;
            }

            serde_json::json!({
                "ok": true,
                "operation": "assign_task",
                "department": target_name,
                "message": format!("{} 任务完成", target_name),
                "result": output
            })
            .to_string()
        }
        None => {
            log_console!("[shangshuling] assign_task: {} 信道意外关闭", target_name);
            serde_json::json!({
                "ok": false,
                "operation": "assign_task",
                "message": format!("{} 执行过程中信道意外关闭", target_name),
                "error_code": "channel_closed"
            })
            .to_string()
        }
    }
}
