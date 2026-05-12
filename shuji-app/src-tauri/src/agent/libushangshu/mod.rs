use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;

pub struct LibuShangshuAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl LibuShangshuAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self { client, model: model.to_string(), cancel }
    }

        fn tools() -> Vec<ToolDefinition> {
            vec![
                crate::tool::read_file_tool_def("读取阶段设计、接口契约"),
                crate::tool::write_file_tool_def("写入详细设计文档到 .shuji/designs/detail/ 目录"),
                crate::tool::append_file_tool_def(),
                crate::tool::delete_file_tool_def(),
                crate::tool::rename_file_tool_def(),
                crate::tool::list_dir_tool_def(),
            ]
        }

        fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &Path) -> String {
            crate::tool::execute_named_tool(name, working_dir, args, "libushangshu")
        }
}

#[async_trait::async_trait]
impl Agent for LibuShangshuAgent {
    fn role(&self) -> Role { Role::LiBuShangshu }

    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput> {
        let system_prompt = include_str!("prompt.md");
        let tools = Self::tools();
        let working_dir = input.working_dir.clone();

        let mut msgs = input.context_messages.clone();
        msgs.push(Message::user(&input.task_description));

        let client = Arc::new(self.client.clone());
        let mut session = crate::api::session::Session::new(
            system_prompt, &msgs, &self.model, &tools, &client,
            &[],
        ).with_role(self.role().name());
        let mut controller = crate::api::control::AgentController::new();
        let exec = |name: &str, args: &serde_json::Value| -> String {
            Self::execute_tool(name, args, &working_dir)
        };
        let (result, route) = controller.run(&mut session, &exec, &self.cancel, &tools).await?;
        let mut output = AgentOutput::new(result);
        output.route = route;
        Ok(output)
    }


}
