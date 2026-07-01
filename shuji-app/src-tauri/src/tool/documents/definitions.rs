//! Tool definition schemas for document CRUD operations.

pub(crate) fn create_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_document".into(),
            description: "Create a new document. The system auto-assigns an ID and generates a YAML header. Returns the document ID.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["dsgn", "plan", "pdsg", "ddtl", "task", "ctrt", "rprt", "revw", "anls", "reqs"],
                        "description": "dsgn/plan/pdsg/ddtl/revw/task/ctrt/rprt/anls/reqs"
                    },
                    "refs": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "Referenced document IDs (integers, without type prefix). Pass [] for no references"
                    }
                },
                "required": ["type", "refs"]
            }),
        },
    }
}

pub(crate) fn create_task_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_document".into(),
            description: "Create a task document (type fixed to 'task'). 内阁出流程请用 submit_pipeline_plan；本工具仅用于创建 task 文档作为 expand_requirements 的前置。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "enum": ["task"], "description": "固定为 task" },
                    "refs": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "Referenced document IDs (integers, no prefix). Pass [] for none"
                    }
                },
                "required": ["type", "refs"]
            }),
        },
    }
}

pub(crate) fn modify_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "modify_document".into(),
            description: "Replace text in a document body (find+replace). ≤3000 chars.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID, e.g. dsgn_003"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Text to replace (≤3000 chars)",
                        "maxLength": 3000
                    },
                    "new_text": {
                        "type": "string",
                        "description": "Replacement text (≤3000 chars)",
                        "maxLength": 3000
                    }
                },
                "required": ["id", "old_text", "new_text"]
            }),
        },
    }
}

pub(crate) fn append_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "append_document".into(),
            description: "Append content to an existing document's body. content ≤6000 chars. For multi-part content, call multiple times. Do NOT use the contents array — array JSON is prone to truncation with long content.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID, e.g. dsgn_003"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to append (≤6000 chars). For multi-part content, call multiple times — do NOT use the contents array.",
                        "maxLength": 6000
                    },
                    "contents": {
                        "type": "array",
                        "description": "[Not recommended] Array JSON is prone to truncation with long content. Use single content parameter with multiple calls instead.",
                        "items": {
                            "type": "string",
                            "maxLength": 6000
                        },
                        "maxItems": 5
                    }
                },
                "anyOf": [
                    {"required": ["id", "content"]},
                    {"required": ["id", "contents"]}
                ]
            }),
        },
    }
}

pub(crate) fn read_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "read_document".into(),
            description: "Preferred document reading method. Reads by document ID, returns YAML metadata + body. Default truncation at 4000 chars (pass max_chars=0 to disable). Optional ## section extraction. Replaces the two-step find_document -> read_file approach.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID without .md suffix, e.g. dsgn_3, plan_1, revw_2. Use list_dir on .shuji/designs to discover IDs from filenames."
                    },
                    "section": {
                        "type": "string",
                        "description": "Optional: extract a specific ## section by title (e.g. 'Signature', 'Data Operations'). Omit to return full body."
                    },
                    "max_chars": {
                        "type": "integer",
                        "description": "Optional: max characters to return; truncates beyond this (prevents oversized documents from blowing up context)"
                    }
                },
                "required": ["id"]
            }),
        },
    }
}

#[allow(dead_code)]
pub(crate) fn find_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "find_document".into(),
            description: "Deprecated: Do not use unless read_document fails. read_document combines find + read + section extraction in one call.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Document ID, e.g. rprt_32, dsgn_003, task_5"
                    }
                },
                "required": ["id"]
            }),
        },
    }
}
