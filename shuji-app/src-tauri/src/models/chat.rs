use serde::{Deserialize, Serialize};

use super::document::Document;
use crate::orchestrator::engine::ProjectSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub options: Vec<ChatOption>,
    pub documents: Vec<Document>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOption {
    pub key: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub messages: Vec<ChatMessage>,
    pub snapshot: ProjectSnapshot,
}

impl ChatMessage {
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            role: role.to_string(),
            content: content.to_string(),
            options: vec![],
            documents: vec![],
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn with_options(mut self, options: Vec<ChatOption>) -> Self {
        self.options = options;
        self
    }

    pub fn with_document(mut self, doc: Document) -> Self {
        self.documents.push(doc);
        self
    }
}

/// Standard options for emperor approval
pub fn approval_options() -> Vec<ChatOption> {
    vec![
        ChatOption { key: "A".into(), label: "同意".into(), description: "批准执行".into() },
        ChatOption { key: "B".into(), label: "同意，补充".into(), description: "批准方向，补充需求".into() },
        ChatOption { key: "C".into(), label: "不行".into(), description: "方案不可行，重新设计".into() },
    ]
}

/// Options for execution issues
pub fn issue_options() -> Vec<ChatOption> {
    vec![
        ChatOption { key: "A".into(), label: "退回修改".into(), description: "退回设计阶段修改".into() },
        ChatOption { key: "C".into(), label: "终止".into(), description: "终止项目".into() },
    ]
}
