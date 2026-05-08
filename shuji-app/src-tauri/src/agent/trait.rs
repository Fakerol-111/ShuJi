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
}

#[derive(Debug, Clone)]
pub struct AgentOutput {
    pub content: String,
    pub documents: Vec<Document>,
}

pub enum AgentDecision {
    /// No decision needed, continue workflow
    None,
    /// Emperor needs to approve/reject
    NeedsApproval {
        document: Document,
    },
    /// Design was rejected (returned for redesign)
    Rejected {
        reason: String,
        count: u32,
    },
    /// Execution encountered a problem
    ExecutionIssue {
        is_blocking: bool,
        reason: String,
    },
}

impl AgentOutput {
    pub fn new(content: String) -> Self {
        Self {
            content,
            documents: Vec::new(),
        }
    }

    pub fn with_document(mut self, doc: Document) -> Self {
        self.documents.push(doc);
        self
    }
}

#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    fn role(&self) -> Role;
    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput>;
    fn parse_decision(&self, output: &AgentOutput) -> AgentDecision;
}
