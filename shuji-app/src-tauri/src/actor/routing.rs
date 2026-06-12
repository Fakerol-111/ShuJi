use crate::api::control::RouteTo;
use crate::models::chat::ChatMessage;

use super::{ActorContext, ActorMessage};
use super::spawn::log_dept;

/// Forward a RouteTo instruction to the target actor.
pub async fn forward_route(ctx: &ActorContext, route: RouteTo) {
    let subject_for_graph = route.subject.clone();
    let mut actor_msg = ActorMessage::new(subject_for_graph.clone(), route.msg_type);
    actor_msg.payload = route.payload;

    let target_name = route.target.name();
    log_dept(ctx, ctx.role.name(), &format!("→ {}", subject_for_graph));

    // Log routing event to activity log
    ctx.logger
        .log_route(ctx.role.name(), target_name, &subject_for_graph)
        .await;

    match ctx.peers.get(&route.target) {
        Some(tx) => {
            let _ = tx.send(actor_msg);
            // ── 记录文移图边 ──
            if let Some(ref graph_lock) = ctx.workflow_graph {
                let mut g = graph_lock.lock().await;
                // 用 msg_type + subject 作为任务标识
                let task_id = format!("{:?}/{}", route.msg_type, subject_for_graph);
                g.add_edge(ctx.role.name(), target_name, &task_id, &subject_for_graph);
                let _ = g.save_to(&ctx.working_dir).await;
            }
        }
        None => {
            let _ = ctx.emperor_tx.try_send(ChatMessage::new(
                "系统",
                &format!("找不到目标部门: {}", target_name),
            ));
        }
    }
}
