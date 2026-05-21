#![allow(dead_code)]
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::agent::r#trait::{Agent, AgentInput};
use crate::api::control::{RouteMsgType, RouteTo, role_from_name};
use crate::config::RuntimeConfig;
use crate::logging::logger::Logger;
use crate::models::chat::ChatMessage;
use crate::models::role::Role;

/// A real-time department log entry emitted to the frontend status panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptLogEntry {
    pub dept: String,
    pub action: String,
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl DeptLogEntry {
    pub fn new(dept: &str, action: &str) -> Self {
        Self {
            dept: dept.to_string(),
            action: action.to_string(),
            ts: chrono::Local::now().format("%H:%M:%S").to_string(),
            detail: None,
        }
    }

    pub fn with_detail(dept: &str, action: &str, detail: &str) -> Self {
        Self {
            dept: dept.to_string(),
            action: action.to_string(),
            ts: chrono::Local::now().format("%H:%M:%S").to_string(),
            detail: Some(detail.to_string()),
        }
    }
}

/// Messages that actors send to each other.
#[derive(Debug, Clone)]
pub enum ActorMessage {
    /// Start a new task.
    Task { content: String },
    /// Interrupt current task and start this instead.
    Replace { content: String },
    /// Only interrupt, don't replace.
    Interrupt,
}

impl ActorMessage {
    fn subject(&self) -> &str {
        match self {
            ActorMessage::Task { content } => content,
            ActorMessage::Replace { content } => content,
            ActorMessage::Interrupt => "",
        }
    }
}

/// Per-actor context passed to `run_actor`.
pub struct ActorContext {
    pub role: Role,
    pub agent: Box<dyn Agent>,
    pub rx: mpsc::UnboundedReceiver<ActorMessage>,
    pub peers: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    pub emperor_tx: mpsc::UnboundedSender<ChatMessage>,
    pub dept_log_tx: mpsc::UnboundedSender<DeptLogEntry>,
    pub plan_tx: mpsc::UnboundedSender<serde_json::Value>,
    pub milestone_tx: mpsc::UnboundedSender<String>,
    pub project_dir: PathBuf,
    pub working_dir: PathBuf,
    pub cancel: Arc<AtomicBool>,
    /// Cancel flags for ALL agents. Only populated for 内阁.
    /// 内阁 uses this to interrupt other agents via the `cancel_agent` tool.
    pub cancel_map: Option<Arc<std::sync::Mutex<HashMap<Role, Arc<AtomicBool>>>>>,
    pub logger: Logger,
    /// Shared context across all actors — stores last output per role.
    pub shared_context: Arc<Mutex<HashMap<Role, String>>>,
    /// Full conversation history between 内阁 and emperor.
    pub talk_history: Arc<Mutex<Vec<String>>>,
    /// Per-agent task plan (populated by 工部尚书 actor for multi-step execution).
    pub plan: Arc<Mutex<Vec<String>>>,
    /// Current skill name for cross-turn persistence (内阁 only).
    pub current_skill: Arc<Mutex<Option<String>>>,
    /// Runtime configuration
    pub runtime_config: Arc<RuntimeConfig>,
}

/// The central actor system, holding all senders.
/// Created at startup, injected into commands.
pub struct ActorSystem {
    /// Senders for all department actors, keyed by Role.
    pub senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    /// Sender for emperor-facing chat messages.
    pub emperor_tx: mpsc::UnboundedSender<ChatMessage>,
    /// Sender for department log entries (→ frontend DeptStatusPanel).
    pub dept_log_tx: mpsc::UnboundedSender<DeptLogEntry>,
    /// Per-agent cancel flags, indexed by Role.
    pub cancel_map: Arc<std::sync::Mutex<HashMap<Role, Arc<AtomicBool>>>>,
    /// Global cancel flag for the frontend cancel button.
    pub cancel: Arc<AtomicBool>,
}

impl ActorSystem {
    pub fn new(
        senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
        emperor_tx: mpsc::UnboundedSender<ChatMessage>,
        dept_log_tx: mpsc::UnboundedSender<DeptLogEntry>,
        cancel_map: Arc<std::sync::Mutex<HashMap<Role, Arc<AtomicBool>>>>,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self { senders, emperor_tx, dept_log_tx, cancel_map, cancel }
    }

    /// Send a message to a role's actor.
    pub fn send(&self, target: &Role, msg: ActorMessage) -> Result<(), String> {
        match self.senders.get(target) {
            Some(tx) => tx.send(msg).map_err(|_| format!("{} actor 已关闭", target.name())),
            None => Err(format!("找不到 {} actor", target.name())),
        }
    }
}

/// Run a single actor's event loop.
///
/// Each actor:
/// 1. Receives messages from its mailbox (mpsc channel)
/// 2. Executes tasks via its agent
/// 3. Routes results (text → emperor, RouteTo → peer actor)
/// 4. Handles Interrupt/Replace via cancel flag
pub async fn run_actor(mut ctx: ActorContext) {
    let role_name = ctx.role.name().to_string();
    log_console!("[actor] {}: started", role_name);

    let mut pending_replace: Option<String> = None;

    while let Some(msg) = ctx.rx.recv().await {
        // ── Interrupt / Replace handling ──────────────
        match &msg {
            ActorMessage::Interrupt => {
                ctx.cancel.store(true, Ordering::SeqCst);
                log_dept(&ctx, &role_name, "收到中断信号");
                continue;
            }
            ActorMessage::Replace { content } => {
                ctx.cancel.store(true, Ordering::SeqCst);
                pending_replace = Some(content.clone());
                log_dept(&ctx, &role_name, &format!("收到替换指令: {}", content));
                // 不 continue！直接 fall through 到执行逻辑，
                // 让 pending_replace 立即生效
            }
            ActorMessage::Task { .. } => {
                ctx.agent.reset_plan();
            }
        }

        // ── Reset cancel ──────────────────────────────
        ctx.cancel.store(false, Ordering::SeqCst);

        // If a Replace was queued while we were busy, use that
        // instead of the original Task content.
        let content = if let Some(replacement) = pending_replace.take() {
            log_console!("[actor] {}: using replacement instead of original task", role_name);
            replacement
        } else {
            msg.subject().to_string()
        };

        // ── Build context (only 内阁 gets talk history + skill prompts) ──
        let skill_prompts: Vec<String> = if ctx.role == crate::models::role::Role::Neige {
            ctx.current_skill.lock().ok()
                .and_then(|s| s.clone())
                .map(|name| {
                    let content = crate::agent::neige::NeigeAgent::load_skill(&name);
                    format!("[skill: {}]\n{}", name, content)
                })
                .into_iter()
                .collect()
        } else {
            vec![]
        };

        let mut context_msgs = Vec::new();
        if ctx.role == crate::models::role::Role::Neige {
            // Talk: full conversation with emperor
            if let Ok(talk) = ctx.talk_history.lock() {
                for line in talk.iter() {
                    context_msgs.push(crate::models::message::Message::assistant(line));
                }
            }
            // Append current emperor message to talk history
            if let Ok(mut talk) = ctx.talk_history.lock() {
                talk.push(format!("皇帝: {}", content));
            }
        }

        // ── Execute (with plan loop for 工部尚书) ────
        log_dept(&ctx, &role_name, "开始处理");

        let mut exec_iterations: u32 = 0;
        let mut last_plan_current: Option<usize> = None;
        let max_exec_iterations = ctx.runtime_config.actor.max_exec_iterations;
        'exec: loop {
            exec_iterations += 1;
            // Safety: break if stuck without plan progress.
            // If the plan's current batch has changed, the agent is making
            // legitimate progress → reset the counter.
            if let Ok(plan_json) = serde_json::from_str::<serde_json::Value>(&ctx.agent.plan_display()) {
                if let Some(cur) = plan_json["current"].as_u64() {
                    let cur_usize = cur as usize;
                    if last_plan_current.map_or(true, |prev| cur_usize != prev) {
                        // Batch changed — progress is being made, reset counter
                        exec_iterations = 1;
                        last_plan_current = Some(cur_usize);
                    }
                }
            }
            if exec_iterations > max_exec_iterations {
                log_console!("[actor] {}: plan loop exceeded {} iterations without batch progress, forcing exit",
                    role_name, max_exec_iterations);
                let _ = ctx.emperor_tx.send(ChatMessage::new("系统",
                    &format!("{} 计划循环超过次数限制（同一批次内 {} 轮未推进），请重新路由", role_name, max_exec_iterations)));
                break 'exec;
            }
            let current_skill = ctx.current_skill.lock().ok().and_then(|s| s.clone());
            let input = AgentInput {
                role: ctx.role,
                task_description: content.clone(),
                context_messages: context_msgs.clone(),
                project_dir: ctx.project_dir.clone(),
                working_dir: ctx.working_dir.clone(),
                skill_prompts: skill_prompts.clone(),
                current_skill,
                runtime_config: ctx.runtime_config.clone(),
            };

            let step_result = {
                let preview: String = content.chars().take(60).collect();
                log_console!("[actor] {} ← 开始执行: {}", role_name, preview);
                ctx.agent.execute(&input).await
            };
            match step_result {
                Ok(output) => {
                    let summary = output.content.chars().take(80).collect::<String>();
                    ctx.logger.log_agent(ctx.role, &summary).await;

                    if output.content.trim().is_empty() {
                        break 'exec;
                    }

                    let milestone = format!("{} | {}", role_name, summary);
                    let _ = ctx.milestone_tx.send(milestone);
                    if let Ok(mut shared) = ctx.shared_context.lock() {
                        shared.insert(ctx.role, output.content.clone());
                    }
                    if ctx.role == Role::Neige {
                        if let Ok(mut talk) = ctx.talk_history.lock() {
                            talk.push(format!("内阁: {}", output.content));
                        }
                        // Don't emit routing-only messages to chat — they go to the log panel
                        if output.route.is_none() && !output.content.starts_with("已路由") {
                            self::emit_to_emperor(&ctx.emperor_tx, ctx.role, &output.content);
                        }
                    } else {
                        log_dept(&ctx, &role_name, &format!("→ {}", output.content));
                    }

                    // Persist current skill for next turn (内阁 only)
                    if let Some(skill_name) = &output.skill {
                        if let Ok(mut s) = ctx.current_skill.lock() {
                            *s = Some(skill_name.clone());
                        }
                    }


                    // If summary mode, save output to summary_prompt in state.json
                    if output.skill.as_deref() == Some("summary") {
                        let state_path = ctx.working_dir.join(".shuji").join("state.json");
                        if let Ok(content) = tokio::fs::read_to_string(&state_path).await {
                            if let Ok(mut proj) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(obj) = proj.as_object_mut() {
                                    obj.insert("summary_prompt".into(), serde_json::Value::String(output.content.clone()));
                                    let content = serde_json::to_string_pretty(&proj).unwrap_or_default();
                                    let _ = tokio::fs::write(&state_path, content).await;
                                }
                            }
                        }
                    }

                    // If route_to present → forward and exit plan loop
                    if let Some(route) = output.route {
                        self::forward_route(&ctx, route).await;
                        break 'exec;
                    }
                    self::parse_legacy_route(&ctx, &output.content);

                    // Let agent decide: need another round?
                    match ctx.agent.after_execute(&output) {
                        crate::agent::r#trait::LoopDecision::Continue(ctx_msg) => {
                            // Emit plan progress to frontend
                            let done = ctx_msg.matches("[x]").count() + ctx_msg.matches("[X]").count();
                            let total = done + ctx_msg.matches("[ ]").count();
                            let plan_action = if total > 0 {
                                format!("执行计划：{}/{} 完成", done, total)
                            } else {
                                "执行计划已输出".to_string()
                            };
                            let _ = ctx.dept_log_tx.send(DeptLogEntry::new(&role_name, &plan_action));
                            let _ = ctx.dept_log_tx.send(DeptLogEntry::with_detail(&role_name, "计划", &ctx_msg));
                            let _ = ctx.milestone_tx.send(format!("{} | {}", role_name, plan_action));
                            // Emit structured plan progress for frontend card
                            let plan_json = ctx.agent.plan_display();
                            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&plan_json) {
                                let _ = ctx.plan_tx.send(value);
                            }
                            context_msgs.push(crate::models::message::Message::user(&ctx_msg));
                            continue 'exec;
                        }
                        crate::agent::r#trait::LoopDecision::Done => {
                            break 'exec;
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("执行错误: {}", e);
                    ctx.logger.log_agent(ctx.role, &err_msg).await;
                    log_dept(&ctx, &role_name, &format!("❌ {}", err_msg));
                    if ctx.role == Role::Neige {
                        let _ = ctx.emperor_tx.send(ChatMessage::new("系统", &err_msg));
                    }
                    break 'exec;
                }
            }
        }
    }

    log_console!("[actor] {}: stopped", role_name);
}

/// Forward a RouteTo instruction to the target actor.
async fn forward_route(ctx: &ActorContext, route: RouteTo) {
    let subject = route.subject.clone();
    let actor_msg = match route.msg_type {
        RouteMsgType::Task => ActorMessage::Task { content: route.subject },
        RouteMsgType::Replace => ActorMessage::Replace { content: route.subject },
        RouteMsgType::Interrupt => ActorMessage::Interrupt,
    };

    let target_name = route.target.name();
    log_dept(ctx, ctx.role.name(), &format!("→ {}", subject));

    // Log routing event to activity log
    ctx.logger.log_route(ctx.role.name(), &target_name, &subject).await;

    match ctx.peers.get(&route.target) {
        Some(tx) => {
            let _ = tx.send(actor_msg);
        }
        None => {
            let _ = ctx.emperor_tx.send(ChatMessage::new(
                "系统",
                &format!("找不到目标部门: {}", target_name),
            ));
        }
    }
}

/// Legacy: parse `<route to="..." subject="..." />` text tags.
/// Only activates when RouteTo is not present.
fn parse_legacy_route(ctx: &ActorContext, output: &str) {
    // Simple search for <route to="...">
    let marker = "<route to=\"";
    let mut start = 0;
    while let Some(pos) = output[start..].find(marker) {
        let abs_pos = start + pos;
        let after_to = abs_pos + marker.len();
        if let Some(end_quote) = output[after_to..].find('\"') {
            let target_name = &output[after_to..after_to + end_quote];
            // Find subject="..."
            let subj_marker = "subject=\"";
            if let Some(subj_start) = output[after_to + end_quote..].find(subj_marker) {
                let subj_abs = after_to + end_quote + subj_start + subj_marker.len();
                if let Some(subj_end) = output[subj_abs..].find('\"') {
                    let subject = &output[subj_abs..subj_abs + subj_end];
                    if let Some(target) = role_from_name(target_name) {
                        if let Some(tx) = ctx.peers.get(&target) {
                            log_console!("[actor] {} → {} (legacy route)", ctx.role.name(), target_name);
                            let _ = tx.send(ActorMessage::Task { content: subject.to_string() });
                        }
                    }
                    start = subj_abs + subj_end;
                    continue;
                }
            }
        }
        start = abs_pos + 1;
    }
}

/// Emit a chat message to the emperor's frontend.
/// Parses `<options>` tags from the content into structured ChatOption objects.
fn emit_to_emperor(tx: &mpsc::UnboundedSender<ChatMessage>, role: Role, content: &str) {
    let role_name = role.name();
    let trimmed = content.trim();
    if trimmed.is_empty() { return; }
    let (clean_content, options) = parse_options(trimmed);
    let mut msg = ChatMessage::new(role_name, &clean_content);
    msg.options = options;
    let _ = tx.send(msg);
}

/// Extract `<options>` from content, return (content_without_options, parsed_options).
fn parse_options(content: &str) -> (String, Vec<crate::models::chat::ChatOption>) {
    let mut options = Vec::new();
    // Find <options> ... </options> block
    if let Some(start) = content.find("<options>") {
        if let Some(end) = content.find("</options>") {
            let block = &content[start..end + "</options>".len()];
            // Extract each <option key="X" label="Y" description="Z" />
            let mut pos = 0;
            while let Some(opt_start) = block[pos..].find("<option ") {
                let abs_start = pos + opt_start;
                if let Some(opt_end) = block[abs_start..].find("/>") {
                    let tag = &block[abs_start..abs_start + opt_end + 2];
                    let key = extract_attr(tag, "key").unwrap_or_default();
                    let label = extract_attr(tag, "label").unwrap_or_default();
                    let desc = extract_attr(tag, "description").unwrap_or_default();
                    if !key.is_empty() {
                        options.push(crate::models::chat::ChatOption { key, label, description: desc });
                    }
                    pos = abs_start + opt_end + 2;
                } else { break; }
            }
            // Remove the options block from content
            let clean = format!("{}{}", content[..start].trim(), content[end + "</options>".len()..].trim());
            return (clean, options);
        }
    }
    (content.to_string(), options)
}

/// Extract an attribute value from an XML-like tag string.
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(val_start) = tag.find(&pattern) {
        let val_begin = val_start + pattern.len();
        if let Some(val_end) = tag[val_begin..].find('\"') {
            return Some(tag[val_begin..val_begin + val_end].to_string());
        }
    }
    None
}

/// Emit a department log entry to the frontend status panel.
fn log_dept(ctx: &ActorContext, dept: &str, action: &str) {
    let _ = ctx.dept_log_tx.send(DeptLogEntry::new(dept, action));
}
