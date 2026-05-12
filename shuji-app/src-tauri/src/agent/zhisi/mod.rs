use std::sync::Arc;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::api::client::AnthropicClient;
use crate::models::message::Message;
use crate::models::role::Role;

pub struct ZhisiAgent {
    client: AnthropicClient,
    model: String,
}

impl ZhisiAgent {
    pub fn new(client: AnthropicClient, model: &str) -> Self {
        Self { client, model: model.to_string() }
    }
}

#[async_trait::async_trait]
impl Agent for ZhisiAgent {
    fn role(&self) -> Role { Role::Zhisi }

    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput> {
        let system_prompt = include_str!("prompt.md");
        let tools: Vec<crate::api::client::ToolDefinition> = Vec::new();

        let mut msgs = input.context_messages.clone();
        msgs.push(Message::user(&input.task_description));

        let client = Arc::new(self.client.clone());
        let mut session = crate::api::session::Session::new(
            system_prompt, &msgs, &self.model, &tools, &client,
            &[],
        ).with_role(self.role().name());
        let mut controller = crate::api::control::AgentController::new();
        let exec = |_name: &str, _args: &serde_json::Value| -> String {
            "制司没有可执行的工具".to_string()
        };
        let (result, route) = controller.run(&mut session, &exec, &std::sync::atomic::AtomicBool::new(false), &tools).await?;
        let mut output = AgentOutput::new(result);
        output.route = route;
        Ok(output)
    }


}
