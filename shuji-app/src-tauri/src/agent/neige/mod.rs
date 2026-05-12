use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::path::Path;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
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
            crate::tool::documents::update_document_tool_def(),
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

/// Extract the first `<skill>xxx</skill>` tag from text.
fn extract_skill(text: &str) -> Option<String> {
    let start = text.find("<skill>")?;
    let after = &text[start + 7..];
    let end = after.find("</skill>")?;
    if end > 50 {
        return None;
    }
    Some(after[..end].to_string())
}

/// Remove `<skill>xxx</skill>` tag from text.
fn strip_skill_tag(mut text: String) -> String {
    if let Some(start) = text.find("<skill>") {
        if let Some(end) = text[start..].find("</skill>") {
            text.replace_range(start..start + end + 8, "");
        }
    }
    text.trim().to_string()
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
        ).with_role(self.role().name());
        let mut controller = crate::api::control::AgentController::new();
        let exec = |name: &str, args: &serde_json::Value| -> String {
            Self::execute_tool(name, args, &working_dir)
        };

        // Run until no more skill switches:
        // 1st pass: base prompt → LLM picks a skill → inject skill → loop
        // 2nd pass: skill loaded → LLM acts → may switch to another skill → loop
        let (mut result, mut route);
        let mut current_skill = String::new();
        let mut skill_guard_retries: u32 = 0;
        loop {
            (result, route) = controller.run(
                &mut session, &exec, &self.cancel, &tools,
            ).await?;

            match extract_skill(&result) {
                Some(skill_name) if !Self::load_skill(&skill_name).is_empty() => {
                    if skill_name == current_skill {
                        // Already running this skill — ignore redundant tag and proceed
                        break;
                    }
                    current_skill = skill_name.clone();
                    skill_guard_retries = 0;
                    log_console!("[内阁] inject skill: {}", skill_name);
                    session.inject_skill(&skill_name, Self::load_skill(&skill_name));
                    // If entering summary mode, inject project state + previous summary
                    if skill_name == "summary" {
                        Self::inject_project_state(&mut session, &working_dir);
                    }
                    // Discard this round's output (only the skill tag) and re-run
                    continue;
                }
                _ if current_skill.is_empty() => {
                    skill_guard_retries += 1;
                    if skill_guard_retries >= 2 {
                        log_console!("[内阁] skill guard failed twice; returning text fallback");
                        break;
                    }
                    log_console!("[内阁] skill guard: no skill selected before action; forcing retry");
                    session.inject("[系统约束] 当前尚未选择工作模式。下一条回复必须且只能是一个 `<skill>...</skill>` 标签，用于选择 clarify / workflow_demo / workflow_simple / workflow_standard / workflow_complex / discuss / summary 之一。禁止调用任何工具，禁止解释，禁止写文件，禁止路由。");
                    continue;
                }
                _ => break,
            }
        }

        let clean = strip_skill_tag(result);
        let mut output = AgentOutput::new(clean);
        output.route = route;
        if !current_skill.is_empty() {
            output.skill = Some(current_skill);
        }
        Ok(output)
    }
}
