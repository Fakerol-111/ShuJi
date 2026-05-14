use std::sync::Arc;
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
}

impl NeigeAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self { client, model: model.to_string(), cancel }
    }

    fn tools() -> Vec<ToolDefinition> {
        vec![
            crate::tool::read_file_tool_def("读取项目目录下的设计文档、日志、状态文件等"),
            crate::tool::list_dir_tool_def(),
            crate::tool::documents::create_document_tool_def(),
            crate::tool::documents::modify_document_tool_def(),
            crate::tool::documents::append_document_tool_def(),
            crate::tool::documents::find_document_tool_def(),
            crate::tool::summarize_logs_tool_def(),
        ]
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

    pub fn load_skill(name: &str) -> &'static str {
        match name {
            "discuss" => include_str!("skills/discuss.md"),
            "clarify" => include_str!("skills/clarify.md"),
            "workflow_demo" => include_str!("skills/workflow_demo.md"),
            "workflow_simple" => include_str!("skills/workflow_simple.md"),
            "workflow_standard" => include_str!("skills/workflow_standard.md"),
            "workflow_complex" => include_str!("skills/workflow_complex.md"),
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
        let mut session = crate::api::session::Session::new(
            system_prompt, &msgs, &self.model, &tools, &client,
            &input.skill_prompts,
        ).with_role(self.role().name()).with_debug_dir(input.working_dir.clone());

        // Restore saved context from previous invocation, compacting if needed
        if let Some(mut ctx) = crate::api::session::PersistedContext::load_from(&working_dir, "neige") {
            log_console!("[内阁] loading context: base={} chars, skills={}, summary={} chars, recent={} msgs",
                ctx.base_prompt.len(), ctx.skill_prompts.len(), ctx.history_messages.len(), ctx.context_messages.len());

            // Compact iteratively: context first, then history. Persist after each step.
            loop {
                let mut changed = false;

                if let Some(result) = crate::api::compact::maybe_compact(
                    &self.client, &self.model, &ctx.history_messages, &ctx.context_messages,
                ).await {
                    ctx.history_messages = result.new_history;
                    ctx.context_messages = result.kept_context;
                    ctx.save_to(&working_dir, "neige");
                    changed = true;
                }

                if let Some(merged) = crate::api::compact::maybe_compact_history(
                    &self.client, &self.model, &ctx.history_messages,
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
        let exec = |name: &str, args: &serde_json::Value| -> String {
            Self::execute_tool(name, args, &working_dir)
        };

        let (mut result, mut route);
        let mut current_skill = input.current_skill.clone().unwrap_or_default();
        loop {
            (result, route) = controller.run(
                &mut session, &exec, &self.cancel, &tools,
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
