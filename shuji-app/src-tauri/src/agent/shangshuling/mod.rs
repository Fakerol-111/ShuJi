use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;
use crate::tool::{resolve_scoped_path, ToolOutput};

pub struct ShangshulingAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl ShangshulingAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self { client, model: model.to_string(), cancel }
    }

    fn tools() -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                tool_type: "function".into(),
                function: crate::api::client::ToolFunction {
                    name: "read_file".into(),
                    description: "读取设计方案、任务文件、审查报告等".into(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "文件路径，相对于项目根目录"
                            }
                        },
                        "required": ["path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".into(),
                function: crate::api::client::ToolFunction {
                    name: "list_dir".into(),
                    description: "列出目录下的文件（支持递归：用 **/* 后缀）".into(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "目录路径，相对于项目根目录"
                            }
                        },
                        "required": ["path"]
                    }),
                },
            },
            crate::tool::delete_file_tool_def(),
            ToolDefinition {
                tool_type: "function".into(),
                function: crate::api::client::ToolFunction {
                    name: "execute_command".into(),
                    description: "在项目根目录执行命令。用于死代码检测工具如：vulture .、cargo check".into(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "要执行的命令"
                            }
                        },
                        "required": ["command"]
                    }),
                },
            },
        ]
    }

    fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &std::path::Path) -> String {
        match name {
            "read_file" => {
                let path = args["path"].as_str().unwrap_or("");
                let full = match resolve_scoped_path(working_dir, path) {
                    Ok(p) => p,
                    Err(e) => return ToolOutput::error("read_file", path, "path_error", &e),
                };
                match std::fs::read_to_string(&full) {
                    Ok(c) => ToolOutput::read_file("read_file", path, &c),
                    Err(e) => ToolOutput::error("read_file", path, "read_error", &e.to_string()),
                }
            }
            "list_dir" => {
                let path = args["path"].as_str().unwrap_or("");
                let full = match resolve_scoped_path(working_dir, path) {
                    Ok(p) => p,
                    Err(e) => return ToolOutput::error("list_dir", path, "path_error", &e),
                };
                match std::fs::read_dir(&full) {
                    Ok(entries) => {
                        let items: Vec<String> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let tag = e.file_type()
                                    .map(|t| if t.is_dir() { "[DIR]" } else { "[FILE]" })
                                    .unwrap_or("[?]");
                                format!("{} {}", tag, e.file_name().to_string_lossy())
                            })
                            .collect();
                        let message = if items.is_empty() { "(空目录)".to_string() } else { items.join("\n") };
                        ToolOutput::success_raw("list_dir", &message)
                    }
                    Err(e) => ToolOutput::error("list_dir", path, "list_error", &e.to_string()),
                }
            }
            "delete_file" => crate::tool::tool_delete_file(working_dir, args),
            "execute_command" => crate::tool::tool_execute_command(working_dir, args, "shangshuling"),
            "route" => ToolOutput::success_raw("route",
                "请调用 route_to 工具，不要输出文本 route 标签。"),
            _ => ToolOutput::error("unknown_tool", name, "unknown_tool", "未知工具"),
        }
    }
}

#[async_trait::async_trait]
impl Agent for ShangshulingAgent {
    fn role(&self) -> Role { Role::Shangshuling }


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
