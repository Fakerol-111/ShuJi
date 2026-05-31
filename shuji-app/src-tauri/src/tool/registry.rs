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
    vec![crate::tool::execute_command_tool_def("执行命令")]
}

/// Log summarization tool.
pub fn summarize_logs_tool() -> Vec<ToolDefinition> {
    vec![crate::tool::summarize_logs_tool_def()]
}

/// Agent-interaction tools (route_to for all, cancel_agent for 内阁).
pub fn route_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "route_to".into(),
            description:
                "向其他部门发送任务。type: task=新任务, replace=中断并替换, interrupt=仅中断。"
                    .into(),
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
                        "description": "文档ID（如 task_5）"
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
            description: "中断指定部门当前操作。可中断: 中书令、门下侍中、尚书令、吏部、兵部、工部、刑部、礼部。不可中断内阁和制司。".into(),
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
            description: "将任务拆分为多个批次执行。每批1-2个目标。".into(),
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
            description: "标记当前批次任务完成，推进到下一批。所有批次完成后会提示写报告并路由。"
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    }
}

pub fn expand_requirements_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "expand_requirements".into(),
            description: "唤起需求展开 sub-agent。需先 create_document(type=\"task\") 创建任务文档，再传入 task_id。返回需求文档ID。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "包含皇帝需求原文的 task 文档 ID（如 task_5）"
                    }
                },
                "required": ["task_id"]
            }),
        },
    }
}

pub fn create_skill_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_skill".into(),
            description: "创建自定义技能文件到 .shuji/skills/。用于固化重复出现的工作流模式。"
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "技能标识符（如 workflow_custom），不含 .md"
                    },
                    "description": {
                        "type": "string",
                        "description": "一句话描述（≤50字符）"
                    },
                    "content": {
                        "type": "string",
                        "description": "Markdown 格式的完整技能指令"
                    }
                },
                "required": ["name", "description", "content"]
            }),
        },
    }
}

pub fn update_soul_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "update_soul".into(),
            description: "向 soul 文件写入一条经验/教训/偏好。每次一条。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "内容，格式: [场景] 描述。≤300字符。"
                    },
                    "section": {
                        "type": "string",
                        "enum": ["经验", "教训", "偏好"],
                        "description": "写入章节: 经验/教训/偏好。不指定则追加到末尾。"
                    }
                },
                "required": ["content"]
            }),
        },
    }
}
