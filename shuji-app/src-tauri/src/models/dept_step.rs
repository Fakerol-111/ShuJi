//! 部门步骤事件模型。
//!
//! 定义了 agent 执行过程中实时推送的步骤级事件。前端通过 `dept-step` Tauri 事件订阅，
//! 在 DeptInspector 面板中逐个渲染——用户可以看到每个部门"正在想什么、正在调什么工具"。
//!
//! 事件流示例：
//! ```
//! Iteration(3) → Thinking("分析需求...") → ToolCall("read_file") → ToolResult(ok)
//! → Thinking("根据文件内容...") → ToolCall("create") → ToolResult(ok) → Text("设计完成")
//! ```

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// 单条步骤事件，agent 执行循环中实时发出。
///
/// 每条事件携带 `dept`（来源部门）和 `ts`（ISO 8601 时间戳），
/// 前端 DeptInspector 根据 `kind` 字段用不同的视觉样式渲染。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptStepEntry {
    /// 来源部门名，如 "内阁"、"工部尚书"
    pub dept: String,
    /// ISO 8601 时间戳（RFC 3339），前端用于排序
    pub ts: String,
    /// 事件类型及其携带的数据
    pub kind: DeptStepKind,
}

impl DeptStepEntry {
    /// 创建一条步骤事件，自动填充当前时间戳。
    pub fn new(dept: &str, kind: DeptStepKind) -> Self {
        Self {
            dept: dept.to_string(),
            ts: chrono::Local::now().to_rfc3339(),
            kind,
        }
    }
}

/// 步骤事件的五种类型，对应 agent 执行循环中的不同阶段。
///
/// 序列化为 JSON 时使用 `"type"` 字段作为标签（serde tag），
/// 前端据此做 switch-case 渲染。
///
/// 生命周期：一轮 agent 迭代 = Thinking → ToolCall → ToolResult → (循环) → Text(输出)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DeptStepKind {
    /// 新一轮 tool-call 迭代开始，`n` 为当前迭代次数。
    /// 前端渲染为分隔线或序号标记。
    #[serde(rename = "iteration")]
    Iteration { n: u32 },

    /// LLM 的思考过程（推理模式开启时可见），展示在可折叠区域中。
    /// 注意：这只是 thinking block 内容的实时推送，最终输出走 Text。
    #[serde(rename = "thinking")]
    Thinking { content: String },

    /// LLM 发起了一次工具调用请求。
    /// - `tool`: 工具函数名（如 "read_file", "create"）
    /// - `args`: 工具参数（JSON 格式，前端可展开查看）
    #[serde(rename = "tool_call")]
    ToolCall {
        tool: String,
        args: serde_json::Value,
    },

    /// 工具执行完毕返回结果。
    /// - `tool`: 工具函数名
    /// - `ok`: 执行是否成功
    /// - `summary`: 结果的摘要信息（截断后的，非完整内容）
    #[serde(rename = "tool_result")]
    ToolResult {
        tool: String,
        ok: bool,
        summary: String,
    },

    /// agent 输出的最终文本（非工具调用）。
    /// 这通常是给用户的回复或给下游部门的工作指令。
    #[serde(rename = "text")]
    Text { content: String },
}

/// 部门步骤事件的发送端类型别名。
///
/// 使用无界通道（unbounded channel）——步骤事件允许瞬时尖峰，
/// 不需要背压保护，且 unbounded 避免了 async send 的复杂度。
/// 生产环境中如果内存增长，可改为 bounded + try_send 模式。
pub type DeptStepSender = mpsc::UnboundedSender<DeptStepEntry>;
