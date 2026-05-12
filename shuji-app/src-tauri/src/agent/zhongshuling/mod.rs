use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;

pub struct ZhongshulingAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl ZhongshulingAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self { client, model: model.to_string(), cancel }
    }

    fn tools() -> Vec<ToolDefinition> {
        vec![
            crate::tool::read_file_tool_def("读取设计文件或祖训"),
            crate::tool::write_file_tool_def("新建或覆盖写入设计文件"),
            crate::tool::append_file_tool_def(),
            crate::tool::delete_file_tool_def(),
            crate::tool::rename_file_tool_def(),
            crate::tool::list_dir_tool_def(),
            crate::tool::documents::create_document_tool_def(),
            crate::tool::documents::update_document_tool_def(),
        ]
    }

    fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &Path) -> String {
        crate::tool::execute_named_tool(name, working_dir, args, "zhongshuling")
    }

    pub fn load_skill(name: &str) -> &'static str {
        match name {
            "overall_design" => include_str!("skills/overall_design.md"),
            "phase_plan" => include_str!("skills/phase_plan.md"),
            "phase_design" => include_str!("skills/phase_design.md"),
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
impl Agent for ZhongshulingAgent {
    fn role(&self) -> Role { Role::Zhongshuling }

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
                        break;
                    }
                    current_skill = skill_name.clone();
                    skill_guard_retries = 0;
                    log_console!("[中书令] replace skill: {}", skill_name);
                    session.replace_skill(&skill_name, Self::load_skill(&skill_name));
                    continue;
                }
                _ if current_skill.is_empty() => {
                    skill_guard_retries += 1;
                    if skill_guard_retries >= 2 {
                        log_console!("[中书令] skill guard failed twice; returning text fallback");
                        break;
                    }
                    log_console!("[中书令] skill guard: no design skill selected before action; forcing retry");
                    session.inject("[系统约束] 当前尚未选择设计模式。下一条回复必须且只能是一个 `<skill>...</skill>` 标签，用于选择 overall_design / phase_plan / phase_design 之一。禁止调用任何工具，禁止解释，禁止写文件，禁止路由。");
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
