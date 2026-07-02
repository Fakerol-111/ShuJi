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
        crate::tool::read_file_tool_def("Read file content"),
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

/// File write tools for code agents (Gongbushangshu/Xingbushangshu): create, apply_patch, delete, rename.
/// Intentionally excludes modify_file and append_file — use apply_patch for all edits.
pub fn file_write_tools_for_code() -> Vec<ToolDefinition> {
    vec![
        crate::tool::create_file_tool_def("Write new file"),
        crate::tool::edit_file_tool_def(),
        crate::tool::apply_patch_tool_def(),
        crate::tool::delete_file_tool_def(),
        crate::tool::rename_file_tool_def(),
    ]
}

/// Full file write tools (legacy, for non-code agents that still need modify/append).
pub fn file_write_tools() -> Vec<ToolDefinition> {
    vec![
        crate::tool::create_file_tool_def("Write new file"),
        crate::tool::edit_file_tool_def(),
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
    vec![crate::tool::execute_command_tool_def("Execute command")]
}

/// Run tests tool (Gongbushangshu-specific — wraps test commands to prevent typos).
pub fn run_tests_tool() -> Vec<ToolDefinition> {
    vec![crate::tool::run_tests_tool_def()]
}

/// Log summarization tool.
pub fn summarize_logs_tool() -> Vec<ToolDefinition> {
    vec![crate::tool::summarize_logs_tool_def()]
}

pub fn cancel_agent_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "cancel_agent".into(),
            description: "Interrupt a department's current operation. Interruptible: Zhongshuling, Menxiashizhong, Shangshuling, Libushangshu, Bingbushangshu, Gongbushangshu, Xingbushangshu, Liburshangshu. Cannot interrupt Neige.".into(),
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
pub fn submit_batch_plan_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "submit_plan".into(),
            description: "Split a task into multiple batches for execution. 1-2 goals per batch."
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "batches": {
                        "type": "array",
                        "description": "List of batches",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string", "description": "Short batch name"},
                                "goal": {"type": "string", "description": "Goal for this batch, one sentence"}
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
            description: "Mark the current batch task as complete, advancing to the next batch. When all batches are done, the engine handles it automatically.".into(),
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
            description: "Invoke the requirements expansion sub-agent. First create a task document with create_document(type=\"task\"), then pass the task_id. Returns the requirements document ID.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "Task document ID containing the emperor's original requirements (e.g. task_5)"
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
            description: "Invoke the codebase survey sub-agent. Scans the target repository structure and generates an analysis document. Just pass a task description. Returns the analysis document ID (anls_xxx).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_description": {
                        "type": "string",
                        "description": "Task description telling the sub-agent which areas to focus on during the survey"
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
            description: "Create a custom skill file in .shuji/skills/. Used to solidify recurring workflow patterns."
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Skill identifier (e.g. workflow_custom), without .md suffix"
                    },
                    "description": {
                        "type": "string",
                        "description": "One-line description (≤50 chars)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Full skill instructions in Markdown format"
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
            description: "Write one learning entry to the role soul file. One entry per call. Neige may queue global candidates.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "Content in format: [context] description. ≤500 chars."
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["experience", "lesson", "preference", "project_fact", "command", "review_rule"],
                        "description": "Learning category (English). Preferred over section."
                    },
                    "section": {
                        "type": "string",
                        "enum": ["经验", "教训", "偏好"],
                        "description": "Legacy Chinese section mapping (experience/lesson/preference)."
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["project", "global_candidate"],
                        "description": "project (default) or global_candidate (Neige only, requires evidence)."
                    },
                    "role": {
                        "type": "string",
                        "description": "Target role name. Default Neige. Other roles require evidence."
                    },
                    "evidence": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Evidence references (required for other roles or global_candidate)."
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional tags for indexing."
                    }
                },
                "required": ["content"]
            }),
        },
    }
}

/// Neige submits a pipeline-executable plan. After submission, Neige exits the dispatch loop and PipelineEngine takes over execution.
pub fn submit_pipeline_plan_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "submit_pipeline_plan".into(),
            description: "Submit a machine-executable dynamic task plan. After submission, the pipeline engine automatically executes steps sequentially; Neige no longer participates in each step's dispatch. Only use when the task needs fully automatic execution — if you should ask the emperor first, ask before submitting the plan.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_json": {
                        "type": "string",
                        "description": "Complete JSON plan. dispatch_to steps: {target, task} only — no doc IDs in plan. Engine captures each step's output document ID and passes it to downstream steps (via depends_on) as separate agent context."
                    }
                },
                "required": ["plan_json"]
            }),
        },
    }
}

/// Shangshuling assigns tasks to the six ministries. Blocks until the target department completes execution and returns.
/// Only dispatches one department per call; Shangshuling decides the next step based on the result.
pub fn assign_task_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "assign_task".into(),
            description: "Assign a task to a specified department and wait for the execution result.\
            Based on the result, decide the next step — if tests fail, send back to Gongbushangshu; if all pass, proceed to the next department.\
            Only dispatches one department per call; subsequent departments are decided based on the result."
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "enum": ["吏部", "兵部", "工部", "刑部", "礼部"],
                        "description": "Target department. Note: Neige/Zhongshuling/Menxiashizhong are dispatched directly by Neige, not by Shangshuling."
                    },
                    "task": {
                        "type": "string",
                        "description": "Task description telling the department what work needs to be done"
                    }
                },
                "required": ["to", "task"]
            }),
        },
    }
}

/// Neige modifies the currently executing pipeline plan (used when waking from an exception).
pub fn update_pipeline_plan_tool() -> ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "update_pipeline_plan".into(),
            description: "Modify the currently executing pipeline plan. Can insert/skip/replace steps or modify failure strategies. After modification, the pipeline engine automatically reloads and continues execution.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["insert_after", "skip", "replace"],
                        "description": "Action type: insert_after(insert new step after a step), skip(skip step), replace(replace entire plan)"
                    },
                    "step_id": {
                        "type": "string",
                        "description": "Target step ID"
                    },
                    "data": {
                        "type": "string",
                        "description": "Action data: insert_after/skip takes a JSON step object; replace takes a complete new plan_json"
                    }
                },
                "required": ["action", "data"]
            }),
        },
    }
}

// ── Audit tools (Liburshangshu/Shangshuling) ────────────────────────────────

pub fn audit_checklist_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".into(),
            function: crate::api::client::ToolFunction {
                name: "init_checklist".into(),
                description: "Initialize an audit checklist, generating standard check items by category. Categories: spec/test/general."
                    .into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "enum": ["spec", "test", "general"],
                            "description": "Checklist category"
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
                description: "Update audit checklist item status (pass/fail/na).".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "Checklist item ID, e.g. spec-001"},
                        "status": {"type": "string", "enum": ["pass", "fail", "na"], "description": "New status"},
                        "note": {"type": "string", "description": "Optional note"}
                    },
                    "required": ["id", "status"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: crate::api::client::ToolFunction {
                name: "add_violation".into(),
                description: "Record an audit violation.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "severity": {"type": "string", "enum": ["error", "warning", "info"], "description": "Severity level"},
                        "rule_id": {"type": "string", "description": "Rule ID, e.g. spec-001"},
                        "location": {"type": "string", "description": "Violation location, file path or line number"},
                        "description": {"type": "string", "description": "Violation description"}
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
                "Request Liburshangshu to re-audit a specified document. Automatically routes to the target department. Called by Shangshuling/Xingbushangshu after fixes are complete."
                    .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "subject": {"type": "string", "description": "Document ID to re-audit"},
                    "reason": {"type": "string", "description": "Reason for re-audit"},
                    "to": {
                        "type": "string",
                        "enum": ["礼部"],
                        "description": "Target department (default: Liburshangshu)"
                    }
                },
                "required": ["subject", "reason"]
            }),
        },
    }]
}
