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
        tools.push(crate::tool::registry::expand_requirements_tool());
        tools
    }

    fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &Path) -> String {
        crate::tool::execute_named_tool(name, working_dir, args, "neige")
    }

    /// Read .shuji/state.json and inject project state + previous summary
    /// into the session as context for the summary skill.
    fn inject_project_state(session: &mut crate::api::session::Session, working_dir: &Path) {
        let state_path = working_dir.join(".shuji").join("state.json");
        let content = match std::fs::read_to_string(&state_path) {
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
    fn load_soul(working_dir: &Path) -> String {
        let soul_dir = working_dir.join(".shuji").join("soul");
        let soul_path = soul_dir.join("neige.md");
        if let Ok(content) = std::fs::read_to_string(&soul_path) {
            if !content.trim().is_empty() {
                return content;
            }
        }
        // Bootstrap from compile-time default
        let default = include_str!("soul.md");
        let _ = std::fs::create_dir_all(&soul_dir);
        let _ = std::fs::write(&soul_path, default);
        default.to_string()
    }

    pub fn load_skill(name: &str) -> &'static str {
        match name {
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
            _ => "",
        }
    }
}

#[async_trait::async_trait]
impl Agent for NeigeAgent {
    fn role(&self) -> Role { Role::Neige }

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
         .with_soul(self.role().name(), &Self::load_soul(&input.working_dir))
         .with_debug_dir(input.working_dir.clone());

        // Restore saved context from previous invocation, compacting if needed
        if let Some(mut ctx) = crate::api::session::PersistedContext::load_from(&working_dir, "neige") {
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
                    ctx.save_to(&working_dir, "neige");
                    changed = true;
                }

                if let Some(merged) = crate::api::compact::maybe_compact_history(
                    &self.client, &self.model, &ctx.history_messages, &input.runtime_config,
                ).await {
                    ctx.history_messages = merged;
                    ctx.save_to(&working_dir, "neige");
                    changed = true;
                }

                if !changed { break; }
            }

            let mut msgs = ctx.to_messages();
            msgs.push(serde_json::json!({"role": "user", "content": format!("皇帝新指令：{}", input.task_description)}));
            let snap = crate::api::session::SessionSnapshot::from_messages(msgs);
            session.restore(&snap);
        }

        let mut controller = crate::api::control::AgentController::new();
        let cancel_map = self.cancel_map.clone();
        let config = input.runtime_config.clone();
        let exec = |name: &str, args: &serde_json::Value| -> String {
            if name == "cancel_agent" {
                if let Some(ref map) = cancel_map {
                    let target = args["to"].as_str().unwrap_or("");
                    if let Some(role) = Role::from_name(target) {
                        if let Ok(guard) = map.lock() {
                            if let Some(flag) = guard.get(&role) {
                                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                                log_console!("[内阁] cancel_agent → {} interrupted", target);
                                return serde_json::json!({"ok": true, "message": format!("已中断 {} 的当前操作", target)}).to_string();
                            }
                        }
                    }
                    return serde_json::json!({"ok": false, "message": format!("无法中断: {}", target)}).to_string();
                }
                return serde_json::json!({"ok": false, "message": "cancel_map 不可用"}).to_string();
            }
            if name == "update_soul" {
                let content = args["content"].as_str().unwrap_or("");
                if content.is_empty() {
                    return r#"{"ok": false, "message": "content 不能为空"}"#.to_string();
                }
                if content.len() > 300 {
                    return r#"{"ok": false, "message": "内容过长（最多300字符）"}"#.to_string();
                }
                let soul_dir = working_dir.join(".shuji").join("soul");
                let soul_path = soul_dir.join("neige.md");
                let _ = std::fs::create_dir_all(&soul_dir);
                let entry = format!("\n- {}", content);
                match std::fs::OpenOptions::new().create(true).append(true).write(true).open(&soul_path) {
                    Ok(mut f) => {
                        use std::io::Write;
                        let _ = writeln!(f, "{}", entry);
                        log_console!("[内阁] update_soul → {}", content);
                        return r#"{"ok": true, "message": "已记录"}"#.to_string();
                    }
                    Err(e) => {
                        return serde_json::json!({"ok": false, "message": format!("写入失败: {}", e)}).to_string();
                    }
                }
            }
            if name == "expand_requirements" {
                let task_id = args["task_id"].as_str().unwrap_or("");
                if task_id.is_empty() {
                    return r#"{"ok": false, "message": "task_id 不能为空"}"#.to_string();
                }
                let c = client.clone();
                let m = model.clone();
                let wd = working_dir.clone();
                let tid = task_id.to_string();
                
                match tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(
                        crate::agent::expand_requirements::run(&tid, &wd, &c, &m)
                    )
                }) {
                    Ok(doc_id) => {
                        log_console!("[内阁] expand_requirements → {}", doc_id);
                        return serde_json::json!({"ok": true, "document_id": doc_id}).to_string();
                    }
                    Err(e) => {
                        log_console!("[内阁] expand_requirements 失败: {}", e);
                        return serde_json::json!({"ok": false, "message": e}).to_string();
                    }
                }
            }
            Self::execute_tool(name, args, &working_dir)
        };

        let (mut result, mut route);
        let mut current_skill = input.current_skill.clone().unwrap_or_default();
        loop {
            (result, route) = controller.run(
                &mut session, &exec, &self.cancel, &tools, None, &config,
            ).await?;

            if route.is_some() {
                break;
            }

            match extract_skill(&result) {
                Some(skill_name) if !Self::load_skill(&skill_name).is_empty() => {
                    if skill_name == current_skill {
                        log_console!("[内阁] skill {} already loaded, prompting continue", skill_name);
                        session.inject(&format!("[系统] 技能 {} 已在当前会话中。请直接继续执行该技能的指令，不要重复输出 <skill> 标签。", skill_name));
                        continue;
                    }
                    current_skill = skill_name.clone();
                    log_console!("[内阁] inject skill: {}", skill_name);
                    session.inject_skill(&skill_name, Self::load_skill(&skill_name));
                    session.inject(&format!("[系统] 技能 {} 已加载。请立即按照该技能的指令行动，不要再输出 <skill> 标签。", skill_name));
                    if skill_name == "summary" {
                        Self::inject_project_state(&mut session, &working_dir);
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

        // Persist context for next invocation (compaction already done at load time)
        let snap = session.snapshot();
        let ctx = crate::api::session::PersistedContext::from_messages(&snap.messages);
        ctx.save_to(&working_dir, "neige");

        let clean = strip_skill_tag(result);
        let mut output = AgentOutput::new(clean);
        output.route = route;
        if !current_skill.is_empty() {
            output.skill = Some(current_skill);
        }
        Ok(output)
    }
}
