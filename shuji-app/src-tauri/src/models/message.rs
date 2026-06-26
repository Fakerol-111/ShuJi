//! 通用消息模型。
//!
//! `Message` 与 `ChatMessage` 的区别：
//! - `Message` — 轻量版，仅用于 **LLM API 层面的消息序列化**
//!   （role + content + timestamp，无 id/options）
//! - `ChatMessage` — 完整版，用于**前端展示和 JSONL 持久化**
//!   （id + role + content + options + timestamp）
//!
//! 简单说：Message 是 LLM 对话的"原料"，ChatMessage 是给用户看的"成品"。

use serde::{Deserialize, Serialize};

/// LLM API 层面的消息结构，适配 OpenAI/Anthropic 的 `role + content` 格式。
///
/// 用于：
/// - context_messages 数组中的历史消息条目
/// - PersistedContext 的序列化/反序列化
/// - LLM API 请求体的 messages 字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息角色：`"user"` | `"assistant"` | `"system"` | `"tool"`
    pub role: String,

    /// 消息正文
    pub content: String,

    /// ISO 8601 创建时间戳（RFC 3339）
    pub timestamp: String,
}

impl Message {
    /// 创建一条 user 角色消息（用户输入或业务指令）。
    pub fn user(content: &str) -> Self {
        Self {
            role: "user".into(),
            content: content.to_string(),
            timestamp: chrono::Local::now().to_rfc3339(),
        }
    }

    /// 创建一条 assistant 角色消息（LLM 输出）。
    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".into(),
            content: content.to_string(),
            timestamp: chrono::Local::now().to_rfc3339(),
        }
    }

    /// 创建一条 system 角色消息（上下文注入，非用户任务）。
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".into(),
            content: content.to_string(),
            timestamp: chrono::Local::now().to_rfc3339(),
        }
    }
}
