use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;

pub struct BingbuShangshuAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl BingbuShangshuAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self { client, model: model.to_string(), cancel }
    }

        fn tools() -> Vec<ToolDefinition> {
            let mut tools = crate::tool::registry::inspect_tools();
            tools.extend(crate::tool::registry::document_tools());
            tools
        }

        fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &Path) -> String {
            crate::tool::execute_named_tool(name, working_dir, args, "bingbushangshu")
        }
}

#[async_trait::async_trait]
impl Agent for BingbuShangshuAgent {
    fn role(&self) -> Role { Role::BingbuShangshu }

    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput> {
        let system_prompt = include_str!("prompt.md");
        let tools = Self::tools();
        let working_dir = input.working_dir.clone();

        let mut msgs = input.context_messages.clone();
        msgs.push(Message::user(&input.task_description));

        let client = Arc::new(self.client.clone());
        let mut session = crate::api::session::Session::new(
            system_prompt, &msgs, &self.model, &tools, &client,
            &[], &input.runtime_config,
        ).with_role(self.role().name()).with_debug_dir(input.working_dir.clone());

        let role_name = self.role().name().to_string();
        if let Some(ctx) = crate::api::session::PersistedContext::load_from(&working_dir, &role_name) {
            let mut msgs = ctx.to_messages();
            msgs.push(serde_json::json!({"role": "user", "content": input.task_description}));
            let snap = crate::api::session::SessionSnapshot::from_messages(msgs);
            session.restore(&snap);
        }

        let mut controller = crate::api::control::AgentController::new();
        let config = input.runtime_config.clone();
        let wd_clone = working_dir.clone();
        let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
            let name = name.to_owned();
            let args = args.clone();
            let wd = wd_clone.clone();
            Box::pin(async move { Self::execute_tool(&name, &args, &wd) })
        };
        let (result, route) = controller.run(&mut session, &exec, &self.cancel, &tools, None, &config).await?;

        let snap = session.snapshot();
        let ctx = crate::api::session::PersistedContext::from_messages(&snap.messages);
        ctx.save_to(&working_dir, &role_name);

        let mut output = AgentOutput::new(result);
        output.route = route;
        Ok(output)
    }


}
