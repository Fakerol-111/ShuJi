use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput, LoopDecision};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;

pub struct GongbuShangshuAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl GongbuShangshuAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self { client, model: model.to_string(), cancel }
    }

        fn tools() -> Vec<ToolDefinition> {
            vec![
                crate::tool::read_file_tool_def("读取接口契约、详细设计、审查报告等"),
                crate::tool::create_file_tool_def("写入代码文件到项目目录"),
                crate::tool::append_file_tool_def(),
                crate::tool::delete_file_tool_def(),
                crate::tool::rename_file_tool_def(),
                crate::tool::modify_file_tool_def(),
                crate::tool::list_dir_tool_def(),
                crate::tool::documents::create_document_tool_def(),
                crate::tool::documents::modify_document_tool_def(),
                crate::tool::documents::append_document_tool_def(),
                crate::tool::documents::find_document_tool_def(),
            ]
        }

        fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &Path) -> String {
            crate::tool::execute_named_tool(name, working_dir, args, "gongbushangshu")
        }
}

#[async_trait::async_trait]
impl Agent for GongbuShangshuAgent {
    fn role(&self) -> Role { Role::GongbuShangshu }

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
        ).with_role(self.role().name()).with_max_tokens(2048).with_debug_dir(input.working_dir.clone());

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
        let (result, route) = controller.run(&mut session, &exec, &self.cancel, &tools).await?;

        let snap = session.snapshot();
        let ctx = crate::api::session::PersistedContext::from_messages(&snap.messages);
        ctx.save_to(&working_dir, &role_name);

        let mut output = AgentOutput::new(result);
        output.route = route;
        Ok(output)
    }

    fn after_execute(&self, output: &AgentOutput) -> crate::agent::r#trait::LoopDecision {
        let mut items: Vec<String> = Vec::new();
        let mut all_done = true;
        for line in output.content.lines() {
            let t = line.trim();
            if t.starts_with("- [") || t.starts_with("* [") {
                if !(t.contains("[x]") || t.contains("[X]")) { all_done = false; }
                items.push(t.to_string());
            }
        }
        if items.is_empty() { return LoopDecision::Done; }
        if all_done { return LoopDecision::Done; }
        let plan_text = format!(
            "## 当前任务计划\n{}\n\n每次只完成一个任务项，完成后重新输出完整计划并标记 `[x]`。全部完成后调用 route_to。",
            items.join("\n")
        );
        LoopDecision::Continue(plan_text)
    }
}
