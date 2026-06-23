//! 聊天消息数据模型。
//!
//! 定义了前后端之间传递的聊天消息结构，包括消息体本身和可点击选项。
//! 这是整个项目中最基础的数据结构之一——所有 actor 的输出最终都序列化为
//! `ChatMessage` 通过 `emperor_tx` 通道发送到前端。

use serde::{Deserialize, Serialize};

/// 单条聊天消息，从后端 actor 输出序列化后发送到前端。
///
/// 每个字段同时实现 Serialize（后端 → 前端 Tauri 事件）和 Deserialize
/// （前端接收 + JSONL 持久化读取）。
///
/// `id` 字段使用 UUID v4（新建时生成），前端据此做 React key 和去重，
/// 而不是依赖 `(role, timestamp)` 组合键，避免同一毫秒内多条消息的冲突。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// 稳定唯一标识 (UUID v4)，创建时自动生成。
    /// 替代 `(role, timestamp)` 组合作为 React `key` 和去重依据。
    pub id: String,

    /// 发送者角色名，如 "内阁"、"System"、"中书令"。
    /// 对应 `Role::name()` 或特殊值 "System"（系统通知）/ "Emperor"（用户消息）。
    pub role: String,

    /// 消息正文（markdown 格式）。
    /// 内阁输出的 `<options>` 标签会被前端解析为可点击按钮，而非原样展示。
    pub content: String,

    /// 可选的操作按钮列表，由内阁的 `<options>` 标签解析生成。
    /// 例如审批流中的"批准/驳回"选项会作为 `ChatOption` 出现。
    /// 普通消息（无 `<options>`）此字段为空数组。
    pub options: Vec<ChatOption>,

    /// ISO 8601 时间戳（RFC 3339 格式），创建时自动生成。
    /// 前端用于按时间排序和显示消息时间。
    pub timestamp: String,
}

/// 前端可点击的操作选项，源自内阁输出的 `<options>` 标签。
///
/// 典型场景：内阁需要皇帝做决策时，通过 `<options>` 标签生成按钮，
/// 用户点击后前端将对应 `key` 发回后端作为下一步输入。
///
/// ```xml
/// <options>
/// <option key="approve" label="批准" description="批准当前设计方案"/>
/// <option key="reject" label="驳回" description="要求中书令重新设计"/>
/// </options>
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatOption {
    /// 选项键值，用户点击后作为消息发送到后端（如 "approve", "reject"）。
    pub key: String,

    /// 按钮显示文本，简明扼要（如 "批准", "驳回"）。
    pub label: String,

    /// 辅助说明文本，展示在按钮下方或 tooltip 中。
    pub description: String,
}

impl ChatMessage {
    /// 创建一条新消息，自动生成 UUID id 和当前时间戳。
    ///
    /// `options` 字段初始化为空数组——大多数消息不需要操作选项。
    /// 如果后端需要附带选项，可在创建后手动设置 `options`。
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
