use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Stable unique identifier (UUID v4), generated at creation.
    /// Frontend uses this for React key and dedup instead of composite keys.
    pub id: String,
    pub role: String,
    pub content: String,
    pub options: Vec<ChatOption>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOption {
    pub key: String,
    pub label: String,
    pub description: String,
}

impl ChatMessage {
    pub fn new(role: &str, content: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: role.to_string(),
            content: content.to_string(),
            options: vec![],
            timestamp: chrono::Local::now().to_rfc3339(),
        }
    }
}
