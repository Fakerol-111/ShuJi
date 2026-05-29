use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::path::Path;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::agent::util::{extract_skill, strip_skill_tag};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;

pub struct NeigeAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
    cancel_map: Option<Arc<Mutex<HashMap<Role, Arc<AtomicBool>>>>>,
}

impl NeigeAgent {
    pub fn new(
        client: AnthropicClient,
        model: &str,
        cancel: Arc<AtomicBool>,
        cancel_map: Option<Arc<Mutex<HashMap<Role, Arc<AtomicBool>>>>>,
    ) -> Self {
        Self { client, model: model.to_string(), cancel, cancel_map }
    }

    fn tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::inspect_tools();
        tools.extend(crate::tool::registry::document_tools());
        tools.extend(crate::tool::registry::summarize_logs_tool());
        tools.push(crate::tool::registry::cancel_agent_tool());
        tools.push(crate::tool::registry::update_soul_tool());
        tools.push(crate::tool::registry::create_skill_tool());
        tools.push(crate::tool::registry::expand_requirements_tool());
        tools
    }

    async fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &Path) -> String {
        crate::tool::execute_named_tool(name, working_dir, args, "neige").await
    }

    /// Read .shuji/state.json and inject project state + previous summary
    /// into the session as context for the summary skill.
    async fn inject_project_state(session: &mut crate::api::session::Session, working_dir: &Path) {
        let state_path = working_dir.join(".shuji").join("state.json");
        let content = match tokio::fs::read_to_string(&state_path).await {
            Ok(c) => c,
            Err(_) => return,
        };
        let project: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return,
        };

        let goal = project["goal"].as_str().unwrap_or("");
        let status = project["summary"].as_str().unwrap_or("");
        let task = project["task"].as_str().unwrap_or("");
        let prev_summary = project["summary_prompt"].as_str().unwrap_or("");

        let mut parts = vec![
            "[Project State]".to_string(),
            format!("Goal: {}", goal),
            format!("Status: {}", status),
        ];
        if !task.is_empty() {
            parts.push("Milestones:".to_string());
            for line in task.lines() {
                parts.push(format!("  {}", line));
            }
        }
        if !prev_summary.is_empty() {
            parts.push(String::new());
            parts.push("[Previous Summary]".to_string());
            parts.push(prev_summary.to_string());
        }

        session.inject(&parts.join("\n"));
    }

    /// Load soul from `.shuji/soul/neige.md`. If the file doesn't exist,
    /// bootstrap it from the compile-time default. This allows the soul to
    /// evolve at runtime (via `update_soul` tool or manual editing).
    async fn load_soul(working_dir: &Path) -> String {
        let soul_dir = working_dir.join(".shuji").join("soul");
        let soul_path = soul_dir.join("neige.md");
        if let Ok(content) = tokio::fs::read_to_string(&soul_path).await {
            if !content.trim().is_empty() {
                return content;
            }
        }
        // Bootstrap from compile-time default
        let default = include_str!("soul.md");
        let _ = tokio::fs::create_dir_all(&soul_dir).await;
        let _ = tokio::fs::write(&soul_path, default).await;
        default.to_string()
    }

    /// Load skill content. Checks `.shuji/skills/{name}.md` first (runtime-
    /// created skills), then falls back to compile-time embedded skills.
    /// Returns empty string if the skill is not found in either location.
    pub async fn load_skill(name: &str, working_dir: &Path) -> String {
        // 1. Check runtime skills on disk
        let disk_path = working_dir.join(".shuji").join("skills").join(format!("{}.md", name));
        if let Ok(content) = tokio::fs::read_to_string(&disk_path).await {
            if !content.trim().is_empty() {
                log_console!("[内阁] load skill from disk: {}", name);
                return content;
            }
        }
        // 2. Fall back to compiled-in skills
        let content: &str = match name {
            "discuss" => include_str!("skills/discuss.md"),
            "clarify" => include_str!("skills/clarify.md"),
            "workflow_demo" => include_str!("skills/workflow_demo.md"),
            "workflow_simple" => include_str!("skills/workflow_simple.md"),
            "workflow_standard" => include_str!("skills/workflow_standard.md"),
            "workflow_complex" => include_str!("skills/workflow_complex.md"),
            "workflow_optimize" => include_str!("skills/workflow_optimize.md"),
            "workflow_bugfix" => include_str!("skills/workflow_bugfix.md"),
            "workflow_refactor" => include_str!("skills/workflow_refactor.md"),
            "workflow_audit" => include_str!("skills/workflow_audit.md"),
            "summary" => include_str!("skills/summary.md"),
            "reflect" => include_str!("skills/reflect.md"),
            _ => "",
        };
        content.to_string()
    }

    /// Save raw session messages for pause/resume.
    /// These bypass PersistedContext compression so the full session
    /// context (including <options> decision points) is preserved.
    async fn save_paused_session(messages: &[serde_json::Value], working_dir: &std::path::Path) {
        let path = working_dir.join(".shuji").join("paused_session.json");
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Ok(json) = serde_json::to_string(messages) {
            let _ = tokio::fs::write(&path, &json).await;
            log_console!("[内阁] paused session saved ({} messages)", messages.len());
        }
    }

    /// Load and delete the paused session file.
    async fn load_paused_session(working_dir: &std::path::Path) -> Option<Vec<serde_json::Value>> {
        let path = working_dir.join(".shuji").join("paused_session.json");
        let content = tokio::fs::read_to_string(&path).await.ok()?;
        let messages: Vec<serde_json::Value> = serde_json::from_str(&content).ok()?;
        let _ = tokio::fs::remove_file(&path).await;
        log_console!("[内阁] paused session loaded ({} messages)", messages.len());
        Some(messages)
    }

    /// Clean up paused session file (used on Interrupt/Replace).
    pub async fn clear_paused_session(working_dir: &std::path::Path) {
        let path = working_dir.join(".shuji").join("paused_session.json");
        if path.exists() {
            let _ = tokio::fs::remove_file(&path).await;
            log_console!("[内阁] paused session cleared");
        }
    }
}

#[async_trait::async_trait]
impl Agent for NeigeAgent {
    fn role(&self) -> Role { Role::Neige }

    fn set_interrupt_flag(&mut self, flag: Arc<AtomicBool>) {
        self.cancel = flag;
    }

    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput> {
        let system_prompt = include_str!("prompt.md");
        let tools = Self::tools();
        let working_dir = input.working_dir.clone();

        let mut msgs = input.context_messages.clone();
        msgs.push(Message::user(&input.task_description));

        let client = Arc::new(self.client.clone());
        let model = self.model.clone();
        let mut session = crate::api::session::Session::new(
            system_prompt, &msgs, &self.model, &tools, &client,
            &input.skill_prompts, &input.runtime_config,
        ).with_role(self.role().name())
         .with_soul(self.role().name(), &Self::load_soul(&input.working_dir).await)
         .with_debug_dir(input.working_dir.clone());

        // ── Resume from paused session (内阁 waiting for emperor) ──
        let resumed = if input.resume_paused {
            match Self::load_paused_session(&working_dir).await {
                Some(messages) => {
                    session.restore(&crate::api::session::SessionSnapshot::from_messages(messages));
                    session.inject(&format!("[皇帝回复] {}", input.task_description));
                    true
                }
                None => {
                    log_console!("[内阁] resume_paused=true but no paused session found, falling back to normal flow");
                    false
                }
            }
        } else {
            false
        };

        // ── Normal restore from PersistedContext (skipped when resumed) ──
        if !resumed {
            if let Some(mut ctx) = crate::api::session::PersistedContext::load_from(&working_dir, "neige").await {
                log_console!("[内阁] loading context: base={} chars, skills={}, summary={} chars, recent={} msgs",
                    ctx.base_prompt.len(), ctx.skill_prompts.len(), ctx.history_messages.len(), ctx.context_messages.len());

            // Compact iteratively: context first, then history. Persist after each step.
            loop {
                let mut changed = false;

                if let Some(result) = crate::api::compact::maybe_compact(
                    &self.client, &self.model, &ctx.history_messages, &ctx.context_messages, &input.runtime_config,
                ).await {
                    ctx.history_messages = result.new_history;
                    ctx.context_messages = result.kept_context;
                    ctx.save_to(&working_dir, "neige").await;
                    changed = true;
                }

                if let Some(merged) = crate::api::compact::maybe_compact_history(
                    &self.client, &self.model, &ctx.history_messages, &input.runtime_config,
                ).await {
                    ctx.history_messages = merged;
                    ctx.save_to(&working_dir, "neige").await;
                    changed = true;
                }

                if !changed { break; }
            }

            let mut msgs = ctx.to_messages();
            msgs.push(serde_json::json!({"role": "user", "content": format!("皇帝新指令：{}", input.task_description)}));
            let snap = crate::api::session::SessionSnapshot::from_messages(msgs);
            session.restore(&snap);
        }
        } // end if !resumed

        let mut controller = crate::api::control::AgentController::new();

        // ── Periodic checkpoint ──
        let ckpt_wd = working_dir.clone();
        let ckpt_role = self.role().name().to_string();
        let ckpt_desc = input.task_description.clone();
        controller.set_checkpoint_handler(Box::new(move |snap| {
            let wd = ckpt_wd.clone();
            let role = ckpt_role.clone();
            let desc = ckpt_desc.clone();
            Box::pin(async move {
                crate::storage::checkpoint::save(&wd, &role, &desc, &snap).await;
            })
        }));

        let cancel_map = self.cancel_map.clone();
        let config = input.runtime_config.clone();
        let wd = working_dir.clone();
        let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
            let name = name.to_owned();
            let args = args.clone();
            let ctx = crate::tool::ToolContext {
                working_dir: wd.clone(),
                cancel_map: cancel_map.clone(),
                client: Some(client.clone()),
                model: Some(model.clone()),
            };
            Box::pin(async move {
                if let Some(result) = crate::tool::tool_handle_neige_special(&name, &args, &ctx).await {
                    result
                } else {
                    Self::execute_tool(&name, &args, &ctx.working_dir).await
                }
            })
        };

        let mut result;
        let mut route: Option<crate::api::control::RouteTo>;
        let mut current_skill = input.current_skill.clone().unwrap_or_default();
        loop {
            // Suspension point D: check cancel between controller.run() rounds
            if self.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                log_console!("[内阁] interrupted in outer skill loop");
                result = String::new();
                route = None;
                break;
            }

            let run_result = controller.run(
                &mut session, &exec, &self.cancel, &tools, None, &config,
            ).await?;
            (result, route) = match run_result {
                crate::api::control::RunResult::Done(text) => (text, None),
                crate::api::control::RunResult::Routed { text, route: r } => (text, Some(r)),
                crate::api::control::RunResult::Stopped(text) => (text, None),
            };

            if route.is_some() {
                break;
            }

            match extract_skill(&result) {
                Some(skill_name) if !Self::load_skill(&skill_name, &working_dir).await.is_empty() => {
                    if skill_name == current_skill {
                        log_console!("[内阁] skill {} already loaded, prompting continue", skill_name);
                        session.inject(&format!("[系统] 技能 {} 已在当前会话中。请直接继续执行该技能的指令，不要重复输出 <skill> 标签。", skill_name));
                        continue;
                    }
                    current_skill = skill_name.clone();
                    log_console!("[内阁] inject skill: {}", skill_name);
                    session.inject_skill(&skill_name, &Self::load_skill(&skill_name, &working_dir).await);
                    session.inject(&format!("[系统] 技能 {} 已加载。请立即按照该技能的指令行动，不要再输出 <skill> 标签。", skill_name));
                    if skill_name == "summary" {
                        Self::inject_project_state(&mut session, &working_dir).await;
                    }
                    continue;
                }
                _ => break,
            }
        }

        // Re-read participation level each turn so /level commands take effect immediately
        let level_prompt = match std::env::var("PARTICIPATION_LEVEL")
            .unwrap_or_else(|_| "1".to_string())
            .as_str()
        {
            "3" => include_str!("levels/level3.md"),
            "2" => include_str!("levels/level2.md"),
            _ => include_str!("levels/level1.md"),
        };
        session.inject(level_prompt);

        // ── Pause detection: save raw session when waiting for emperor ──
        let has_options = result.contains("<options>");

        if has_options && route.is_none() {
            // Save raw session (bypasses PersistedContext compression)
            let snap = session.snapshot();
            Self::save_paused_session(&snap.messages, &working_dir).await;
            log_console!("[内阁] <options> detected — session paused, awaiting emperor decision");
        } else {
            // Normal: save to PersistedContext
            let snap = session.snapshot();
	            let ctx = crate::api::session::PersistedContext::from_messages(&snap.messages);
	            ctx.save_to(&working_dir, "neige").await;
	        }

	        let clean = strip_skill_tag(result);
	        let mut output = AgentOutput::new(clean);
	        output.paused = has_options && route.is_none();
	        output.route = route;
	        if !current_skill.is_empty() {
	            output.skill = Some(current_skill);
	        }
	        Ok(output)
	    }
	}
