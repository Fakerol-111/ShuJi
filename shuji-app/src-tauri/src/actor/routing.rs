//! Actor 间消息路由。
//!
//! 本文件实现了 `forward_route`——当某个 agent 输出包含 `RouteTo` 指令时，
//! 将消息从当前 actor 转发到目标 actor 的 mailbox。
//!
//! **路由流程**:
//! 1. 当前 actor 执行完毕，`AgentOutput.route` 包含 `RouteTo { target, ... }`
//! 2. `run_actor` 调用 `forward_route(ctx, route)`
//! 3. 查 `ctx.peers` 找到目标部门的 sender
//! 4. 发送 ActorMessage → 写入文移图边 → 记录路由日志

use crate::api::control::RouteTo;
use crate::models::chat::ChatMessage;

use super::spawn::log_dept;
use super::{ActorContext, ActorMessage};

/// 将 `RouteTo` 指令从当前 actor 转发到目标 actor。
///
/// **执行步骤**:
/// 1. 构造 ActorMessage：以 route.subject 为主题，route.msg_type 为类型，
///    route.payload 为附加载荷
/// 2. 记录部门日志：`→ 目标部门名`
/// 3. 写路由事件到 Logger（.shuji/logs/ 目录）
/// 4. 查 `ctx.peers` 找到目标部门的 mailbox sender：
///    - **找到** → 发送消息 → 同时在文移图（WorkflowGraph）中添加一条有向边
///      （src=当前部门, dst=目标部门, task_id="{msg_type}/{subject}"）
///    - **未找到** → 向皇帝发送错误消息："找不到目标部门: X"
pub async fn forward_route(ctx: &ActorContext, route: RouteTo) {
    let subject_for_graph = route.subject.clone();
    let mut actor_msg = ActorMessage::new(subject_for_graph.clone(), route.msg_type);
    actor_msg.payload = route.payload.clone();

    let target_name = route.target.name();
    log_dept(ctx, ctx.role.name(), &format!("→ {}", subject_for_graph));

    // 写路由事件到 activity log（供审计和调试）
    ctx.logger
        .log_route(ctx.role.name(), target_name, &subject_for_graph)
        .await;

    match ctx.peers.get(&route.target) {
        Some(tx) => {
            let _ = tx.send(actor_msg);

            // 记录文移图边：DAG 中 `当前部门 → 目标部门` 的有向边
            if let Some(ref graph_lock) = ctx.workflow_graph {
                let mut g = graph_lock.lock().await;
                // 用 msg_type + subject 作为任务标识，确保同一任务多次路由算同一条边
                let task_id = format!("{:?}/{}", route.msg_type, subject_for_graph);
                g.add_edge(ctx.role.name(), target_name, &task_id, &subject_for_graph);
                let _ = g.save_to(&ctx.working_dir).await;
            }
        }
        None => {
            // P0-5: 路由失败自动 fallback 到尚书令
            log_console!(
                "[routing] route target {} not found in peers, attempting fallback",
                target_name
            );
            ctx.logger
                .log_route(ctx.role.name(), "FALLBACK", &subject_for_graph)
                .await;

            if ctx.role == crate::models::role::Role::Shangshuling {
                let _ = ctx.emperor_tx.try_send(ChatMessage::new(
                    "系统",
                    &format!(
                        "路由失败：找不到目标部门 {}（当前角色为尚书令，无法自回退）",
                        target_name,
                    ),
                ));
                log_dept(
                    ctx,
                    ctx.role.name(),
                    &format!(
                        "⇢ route failure: target {} not found (Shangshuling, no self-fallback)",
                        target_name
                    ),
                );
            } else {
                match ctx.peers.get(&crate::models::role::Role::Shangshuling) {
                    Some(tx) => {
                        let fallback_msg = format!(
                        "[route failure fallback]\nOriginal target: {}\nSubject: {}\nThe target department was not found in the routing table. Please re-route to the correct department.",
                        target_name, subject_for_graph
                    );
                        let mut fb_msg = ActorMessage::new(
                            fallback_msg,
                            crate::api::control::RouteMsgType::Task,
                        );
                        fb_msg.payload = route.payload.clone();
                        let _ = tx.send(fb_msg);
                        log_dept(
                            ctx,
                            ctx.role.name(),
                            &format!("⇢ fallback to 尚书令 (target {} not found)", target_name),
                        );
                    }
                    None => {
                        let _ = ctx.emperor_tx.try_send(ChatMessage::new(
                            "系统",
                            &format!(
                                "路由失败：找不到目标部门 {}，且尚书令也不在 peers 中",
                                target_name
                            ),
                        ));
                        log_dept(
                            ctx,
                            ctx.role.name(),
                            &format!(
                                "⇢ route failure: target {} not found, 尚书令 also not in peers",
                                target_name
                            ),
                        );
                    }
                }
            }
        }
    }
}
