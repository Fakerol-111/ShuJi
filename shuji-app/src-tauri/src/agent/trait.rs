#![allow(dead_code)]
use crate::models::document::Document;
use crate::models::role::Role;
use crate::models::message::Message;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AgentInput {
    pub role: Role,
    pub task_description: String,
    pub context_messages: Vec<Message>,
    pub project_dir: PathBuf,
    pub working_dir: PathBuf,
    /// Active skill system messages (内阁 only), injected between base_prompt
    /// and history. Stored as a vec to allow multiple active skills and future
    /// context compression.
    pub skill_prompts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AgentOutput {
    pub content: String,
    pub documents: Vec<Document>,
    pub route: Option<crate::api::control::RouteTo>,
    /// Current skill name, used for cross-turn persistence (内阁 only).
    pub skill: Option<String>,
}

impl AgentOutput {
    pub fn new(content: String) -> Self {
        Self {
            content,
            documents: Vec::new(),
            route: None,
            skill: None,
        }
    }

    pub fn with_document(mut self, doc: Document) -> Self {
        self.documents.push(doc);
        self
    }

    pub fn with_route(mut self, route: crate::api::control::RouteTo) -> Self {
        self.route = Some(route);
        self
    }
}

/// Returned by `after_execute`: whether the actor should continue the loop.
pub enum LoopDecision {
    /// Stop the loop, break out
    Done,
    /// Continue with a context message for the next round
    Continue(String),
}

#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    fn role(&self) -> Role;
    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput>;

    /// Called after each execute() round. Default: stop the loop.
    /// Agents like 工部尚书 can override to continue with plan tracking.
    fn after_execute(&self, _output: &AgentOutput) -> LoopDecision {
        LoopDecision::Done
    }
}
