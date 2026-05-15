use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::agent::util::{extract_skill, strip_skill_tag};
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
        let mut tools = crate::tool::registry::inspect_tools();
        tools.extend(crate::tool::registry::document_tools());
        tools
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
        ).with_role(self.role().name()).with_max_tokens(1024).with_debug_dir(input.working_dir.clone());

        let role_name = self.role().name().to_string();
        if let Some(ctx) = crate::api::session::PersistedContext::load_from(&working_dir, &role_name) {
            let mut msgs = ctx.to_messages();
            msgs.push(serde_json::json!({"role": "user", "content": input.task_description}));
            let snap = crate::api::session::SessionSnapshot::from_messages(msgs);
            session.restore(&snap);
        }

        let mut controller = crate::api::control::AgentController::new();
        let exec = |name: &str, args: &serde_json::Value| -> String {
            Self::execute_tool(name, args, &working_dir)
        };

        let (mut result, mut route);
        let mut current_skill = String::new();
        loop {
            (result, route) = controller.run(
                &mut session, &exec, &self.cancel, &tools, None,
            ).await?;

            if route.is_some() {
                break;
            }

            match extract_skill(&result) {
                Some(skill_name) if !Self::load_skill(&skill_name).is_empty() => {
                    if skill_name == current_skill {
                        log_console!("[中书令] skill {} already loaded, prompting continue", skill_name);
                        session.inject(&format!("[系统] 技能 {} 已在当前会话中。请直接继续执行该技能的指令，不要重复输出 <skill> 标签。", skill_name));
                        continue;
                    }
                    current_skill = skill_name.clone();
                    log_console!("[中书令] replace skill: {}", skill_name);
                    session.replace_skill(&skill_name, Self::load_skill(&skill_name));
                    session.inject(&format!("[系统] 模式已切换为 {}。请立即按照该模式的指令开始设计工作，不要再输出 <skill> 标签。", skill_name));
                    continue;
                }
                _ => break,
            }
        }

        let snap = session.snapshot();
        let ctx = crate::api::session::PersistedContext::from_messages(&snap.messages);
        ctx.save_to(&working_dir, self.role().name());

        let clean = strip_skill_tag(result);
        let mut output = AgentOutput::new(clean);
        output.route = route;
        if !current_skill.is_empty() {
            output.skill = Some(current_skill);
        }
        Ok(output)
    }
}
