//! Central tool registry. Every agent composes its tool list from the
//! group functions below instead of assembling Vec<ToolDefinition> manually.
//!
//! Tool implementations stay in `mod.rs` (file ops, commands) and
//! `documents.rs` (document CRUD).  This file only exports groups.

use crate::api::client::ToolDefinition;

// ── Tool groups ───────────────────────────────────────────────────

/// Read-only inspection tools: read files, list directories, find documents.
pub fn inspect_tools() -> Vec<ToolDefinition> {
    vec![
        crate::tool::read_file_tool_def("读取文件内容"),
        crate::tool::list_dir_tool_def(),
        crate::tool::documents::find_document_tool_def(),
    ]
}

/// File write tools: create, modify, append, delete, rename.
pub fn file_write_tools() -> Vec<ToolDefinition> {
    vec![
        crate::tool::create_file_tool_def("写入新文件"),
        crate::tool::modify_file_tool_def(),
        crate::tool::append_file_tool_def(),
        crate::tool::delete_file_tool_def(),
        crate::tool::rename_file_tool_def(),
    ]
}

/// Document tools: create, modify, append (no find — that's in inspect).
pub fn document_tools() -> Vec<ToolDefinition> {
    vec![
        crate::tool::documents::create_document_tool_def(),
        crate::tool::documents::modify_document_tool_def(),
        crate::tool::documents::append_document_tool_def(),
    ]
}

/// Command execution tool.
pub fn execute_command_tool() -> Vec<ToolDefinition> {
    vec![
        crate::tool::execute_command_tool_def("执行命令"),
    ]
}

/// Log summarization tool.
pub fn summarize_logs_tool() -> Vec<ToolDefinition> {
    vec![
        crate::tool::summarize_logs_tool_def(),
    ]
}

/// Agent-interaction tools (route_to for all, cancel_agent for 内阁).
pub fn route_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "route_to".into(),
            description: "向其他部门发送任务或消息。ONLY for cross-department communication — NEVER use this to switch skills or modes. To switch skills, output a <skill>name</skill> tag in your text response instead. 消息类型：task（新任务）、replace（中断当前任务并替换）、interrupt（仅中断）。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "enum": ["中书令", "门下侍中", "内阁", "尚书令", "吏部", "工部", "兵部", "刑部", "礼部", "制司"]
                    },
                    "type": {
                        "type": "string",
                        "enum": ["task", "replace", "interrupt"]
                    },
                    "subject": {
                        "type": "string",
                        "description": "文档ID（如 task_5, dsgn_003），接收部门会读取该文档理解任务。必须用文档ID，不能写自然语言描述。先 create_document 拿到ID再路由。"
                    }
                },
                "required": ["to", "type", "subject"]
            }),
        },
    }
}

pub fn cancel_agent_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "cancel_agent".into(),
            description: "中断指定部门的当前操作，使其恢复到操作前状态。可中断中书令、门下侍中及执行链部门（尚书令、吏部、兵部、工部、刑部、礼部）。不能中断内阁和制司。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "enum": ["中书令", "门下侍中", "尚书令", "吏部", "工部", "兵部", "刑部", "礼部"]
                    }
                },
                "required": ["to"]
            }),
        },
    }
}

/// Submit a plan with batches for the plan loop (工部).
pub fn submit_plan_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "submit_plan".into(),
            description: "将大任务拆分为多个批次。仅在任务繁重（超过3个文件的实现）时使用。每批1-2个目标，系统每次只注入当前批的上下文。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "batches": {
                        "type": "array",
                        "description": "批次列表",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string", "description": "批次简短名称"},
                                "goal": {"type": "string", "description": "本批要完成的目标，一句话"}
                            },
                            "required": ["name", "goal"]
                        }
                    }
                },
                "required": ["batches"]
            }),
        },
    }
}

/// Mark the current batch as complete (工部).
pub fn complete_task_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "complete_task".into(),
            description: "标记当前批次任务完成，推进到下一批。所有批次完成后会提示写报告并路由。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    }
}
