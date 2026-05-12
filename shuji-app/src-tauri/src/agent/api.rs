use crate::agent::r#trait::{Agent, AgentInput, AgentOutput, AgentDecision};
use crate::api::client::AnthropicClient;
use crate::models::document::{Document, DocumentType};
use crate::models::message::Message;
use crate::models::role::Role;

pub struct ApiAgent {
    role: Role,
    client: AnthropicClient,
}

impl ApiAgent {
    pub fn new(role: Role, api_key: String) -> Self {
        Self {
            role,
            client: AnthropicClient::new(api_key),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.client = self.client.with_model(model);
        self
    }
}

#[async_trait::async_trait]
impl Agent for ApiAgent {
    fn role(&self) -> Role {
        self.role
    }

    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput> {
        let system_prompt = format!(
            r#"你是{}，枢机系统中的{}。

## 你的职责
{}

## 工作守则
1. 只做你职责范围内的事
2. 输出的方案要有实际内容，不要敷衍
3. 如果发现方案有问题，明确指出
4. 产出文档时，用明确的标题分隔

## 当前任务
{}"#,
            self.role.name(),
            self.role.name(),
            self.role.system_prompt(),
            input.task_description,
        );

        let mut messages = input.context_messages.clone();
        messages.push(Message::user(
            &format!("请执行任务：{}", input.task_description)
        ));

        let response = self.client.send_message(&system_prompt, &messages).await?;

        // If this is 门下省, check if the response contains a rejection
        if self.role == Role::MenxiaShizhong {
            if response.contains("[驳回]") || response.to_lowercase().contains("不通过") || response.to_lowercase().contains("驳回") {
                return Ok(AgentOutput::new(response));
            }
        }

        // If this is 兵部 or an execution role, check for issues
        if self.role == Role::BingbuShangshu || self.role == Role::XingbuShangshu {
            if response.contains("[阻塞]") || response.contains("严重问题") || response.contains("无法通过") {
                return Ok(AgentOutput::new(response));
            }
        }

        let doc_type = match self.role {
            Role::MenxiaShizhong => DocumentType::Review,
            Role::Neige => DocumentType::Memorial,
            Role::Shangshuling => DocumentType::Dispatch,
            Role::LiBuShangshu => DocumentType::TaskBreakdown,
            _ => DocumentType::Log,
        };

        let title = format!("{}-{}", self.role.name(), doc_type.as_str());
        let doc = Document {
            title,
            content: response.clone(),
            doc_type,
            path: None,
        };

        Ok(AgentOutput::new(response).with_document(doc))
    }

    fn parse_decision(&self, output: &AgentOutput) -> AgentDecision {
        let content = &output.content;
        if self.role == Role::MenxiaShizhong {
            if content.contains("[驳回]") || content.contains("不通过") || content.contains("驳回") {
                return AgentDecision::Rejected {
                    reason: content.clone(),
                    count: 1,
                };
            }
        }
        if self.role == Role::BingbuShangshu || self.role == Role::XingbuShangshu {
            if content.contains("[阻塞]") || content.contains("严重问题") || content.contains("无法通过") {
                let is_blocking = content.contains("[阻塞]") || content.contains("严重问题");
                return AgentDecision::ExecutionIssue {
                    is_blocking,
                    reason: content.clone(),
                };
            }
        }
        AgentDecision::None
    }
}
