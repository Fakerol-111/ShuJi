//! Actor 主循环：收消息 → 处理 → 产出。
//!
//! ```text
//! run_actor
//!   └─ dispatch_mailbox_message   (Interrupt / Replace / Task 分派)
//!        └─ run_exec_loop         (fast 中断、execute、输出分派)
//!             ├─ neige::*         (内阁 talk / pipeline / emit)
//!             ├─ output::*        (路由、暂停、工部批次 Continue)
//!             └─ fallback::*      (失败回退尚书令)
//! ```

mod emit;
mod exec_loop;
mod fallback;
mod mailbox;
mod neige;
mod output;

/// 向外部（`crate::actor` 内）暴露 `log_dept`，供 routing.rs 使用。
pub(in crate::actor) use emit::log_dept;

use mailbox::{dispatch_mailbox_message, ActorRunState, MailboxOutcome};

use super::ActorContext;

/// 运行单个 actor 的事件循环。
///
/// **结构**：
/// - 外层 `while let Some(msg) = rx.recv()` — 阻塞等待 mailbox 消息
/// - `dispatch_mailbox_message` — 根据消息类型（Interrupt/Replace/Task）做预处理
/// - `MailboxOutcome::Run(task)` — 进入 `run_exec_loop` 内层执行循环
/// - `MailboxOutcome::Skip` — Interrupt，直接回顶部等下一条
///
/// 当 mailbox 通道关闭（`rx.recv()` 返回 `None`），表示 actor 系统正在销毁，
/// 循环退出，actor 的 tokio task 结束。
pub async fn run_actor(mut ctx: ActorContext) {
    let role_name = ctx.role.name().to_string();
    log_console!("[actor] {}: started", role_name);

    // 跨多轮 mailbox 消息保持的状态
    let mut state = ActorRunState::default();

    // ── 外层循环：阻塞等待 mailbox 中的下一条消息 ──
    while let Some(msg) = ctx.rx.recv().await {
        // 处理消息：Interrupt → Skip，Task/Replace → Run(task)
        match dispatch_mailbox_message(&mut ctx, &role_name, msg, &mut state).await {
            MailboxOutcome::Skip => {
                // Interrupt：什么都不做，回顶部等下一条
            }
            MailboxOutcome::Run(task) => {
                // Task / Replace：进入内层 exec 循环
                exec_loop::run_exec_loop(
                    &mut ctx,
                    &role_name,
                    task,
                    &mut state.paused_for_decision,
                )
                .await;
            }
        }
    }

    // 通道关闭，标记角色为空闲
    crate::round_metrics::mark_idle(&role_name);
    log_console!("[actor] {}: stopped", role_name);
}
