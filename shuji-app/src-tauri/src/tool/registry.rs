//! Central tool registry. Every agent composes its tool list from the
//! group functions below instead of assembling Vec<ToolDefinition> manually.
//!
//! Tool implementations stay in `mod.rs` (file ops, commands) and
//! `documents.rs` (document CRUD).  This file only exports groups.

use crate::api::client::ToolDefinition;

// ── Tool groups ───────────────────────────────────────────────────

// ── Role-based inspection groups ─────────────────────────────

/// Document-oriented inspection tools: for agents that work with .shuji documents.
pub fn doc_inspect_tools() -> Vec<ToolDefinition> {
    vec![
        crate::tool::documents::read_document_tool_def(),
        crate::tool::list_dir_tool_def(),
        crate::tool::search_text_tool_def(),
    ]
}

/// Code-oriented inspection tools: for agents that read/modify source files.
pub fn code_inspect_tools() -> Vec<ToolDefinition> {
    vec![
        crate::tool::read_file_tool_def("读取文件内容"),
        crate::tool::list_dir_tree_tool_def(),
        crate::tool::search_text_tool_def(),
    ]
}

/// Minimal inspection: for agents that only need read_document + list_dir.
pub fn minimal_inspect_tools() -> Vec<ToolDefinition> {
    vec![
        crate::tool::documents::read_document_tool_def(),
        crate::tool::list_dir_tool_def(),
    ]
}

/// Legacy alias — kept for backward compat, prefer role-specific groups.
pub fn inspect_tools() -> Vec<ToolDefinition> {
    doc_inspect_tools()
}

// ── Write tool groups ────────────────────────────────────────

/// File write tools for code agents (工部/刑部): create, apply_patch, delete, rename.
/// Intentionally excludes modify_file and append_file — use apply_patch for all edits.
pub fn file_write_tools_for_code() -> Vec<ToolDefinition> {
    vec![
        crate::tool::create_file_tool_def("写入新文件"),
        crate::tool::apply_patch_tool_def(),
        crate::tool::delete_file_tool_def(),
        crate::tool::rename_file_tool_def(),
    ]
}

/// Full file write tools (legacy, for non-code agents that still need modify/append).
pub fn file_write_tools() -> Vec<ToolDefinition> {
    vec![
        crate::tool::create_file_tool_def("写入新文件"),
        crate::tool::apply_patch_tool_def(),
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
        crate::tool::documents::set_document_status_tool_def(),
    ]
}

/// Command execution tool.
pub fn execute_command_tool() -> Vec<ToolDefinition> {
    vec![crate::tool::execute_command_tool_def("执行命令")]
}

/// Run tests tool (工部专用 — 封装测试命令防拼错).
pub fn run_tests_tool() -> Vec<ToolDefinition> {
    vec![crate::tool::run_tests_tool_def()]
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
                        "enum": ["中书令", "门下侍中", "内阁", "尚书令", "吏部", "工部", "兵部", "刑部", "礼部"]
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
            description: "中断指定部门当前操作。可中断: 中书令、门下侍中、尚书令、吏部、兵部、工部、刑部、礼部。不可中断内阁。".into(),
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

pub fn survey_codebase_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "survey_codebase".into(),
            description: "唤起代码库勘察 sub-agent。扫描目标仓库结构生成分析文档。只需传入任务描述。返回分析文档ID（anls_xxx）。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_description": {
                        "type": "string",
                        "description": "任务描述，告知 sub-agent 勘察的重点方向和关注点"
                    }
                },
                "required": ["task_description"]
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
                        "description": "内容，格式: [场景] 描述。≤500字符。"
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

// ── Audit tools (礼部/尚书令) ────────────────────────────────

pub fn audit_checklist_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".into(),
            function: crate::api::client::ToolFunction {
                name: "init_checklist".into(),
                description: "初始化审计检查清单，按类别生成标准检查项。类别: spec/test/general。"
                    .into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "enum": ["spec", "test", "general"],
                            "description": "检查类别"
                        }
                    },
                    "required": ["category"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: crate::api::client::ToolFunction {
                name: "update_checklist_item".into(),
                description: "更新审计检查项状态（pass/fail/na）。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "检查项 ID，如 spec-001"},
                        "status": {"type": "string", "enum": ["pass", "fail", "na"], "description": "新状态"},
                        "note": {"type": "string", "description": "备注说明（可选）"}
                    },
                    "required": ["id", "status"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: crate::api::client::ToolFunction {
                name: "add_violation".into(),
                description: "记录一条审计违规。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "severity": {"type": "string", "enum": ["error", "warning", "info"], "description": "严重程度"},
                        "rule_id": {"type": "string", "description": "规则 ID，如 spec-001"},
                        "location": {"type": "string", "description": "违规位置，文件路径或行号"},
                        "description": {"type": "string", "description": "违规描述"}
                    },
                    "required": ["rule_id", "description"]
                }),
            },
        },
    ]
}

pub fn reauth_tool() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "request_reauth".into(),
            description:
                "请求礼部对指定文档重新审计。会自动路由到目标部门。修复完成后由尚书令/刑部调用。"
                    .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "subject": {"type": "string", "description": "需要复验的文档 ID"},
                    "reason": {"type": "string", "description": "复验原因"},
                    "to": {
                        "type": "string",
                        "enum": ["礼部"],
                        "description": "目标部门（默认礼部）"
                    }
                },
                "required": ["subject", "reason"]
            }),
        },
    }]
}
