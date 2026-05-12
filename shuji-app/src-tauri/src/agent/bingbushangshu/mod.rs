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
            vec![
                crate::tool::read_file_tool_def("读取详细设计文档、接口契约"),
                crate::tool::write_file_tool_def("写入测试文件或接口契约文件"),
                crate::tool::append_file_tool_def(),
                crate::tool::delete_file_tool_def(),
                crate::tool::rename_file_tool_def(),
                crate::tool::edit_file_tool_def(),
                crate::tool::execute_command_tool_def("在项目根目录运行命令。用于创建虚拟环境和安装依赖"),
                crate::tool::list_dir_tool_def(),
            ]
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
