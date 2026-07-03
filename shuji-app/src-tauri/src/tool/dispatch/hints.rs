//! Error hint augmentation — appends actionable hints to tool error results.
//! Extracted from dispatch.rs.

/// Classify a tool error and append an actionable correction hint for the LLM.
pub fn augment_error_with_hint(name: &str, raw_result: &str, _dept: &str) -> String {
    // Only augment error results
    if !crate::tool::output::ToolOutput::is_error(raw_result) {
        return raw_result.to_string();
    }

    let error_code = crate::tool::output::ToolOutput::error_code(raw_result);
    let message = crate::tool::output::ToolOutput::extract_message(raw_result).unwrap_or_default();
    let msg_lower = message.to_lowercase();

    let hint = match (name, error_code.as_deref()) {
        // -- File operations --
        ("read_file", _)
            if msg_lower.contains("not found") || msg_lower.contains("no such file") =>
        {
            "HINT: 文件不存在。请先使用 list_dir 确认文件路径，或检查路径拼写。如果文件可能在其他位置，使用 search_text 搜索。"
        }
        ("create_file", _) if msg_lower.contains("already exists") => {
            "HINT: 文件已存在。请使用 edit_file（单处修改）或 apply_patch（多处修改）来更新现有文件，不要重复创建。"
        }
        ("edit_file", _) | ("apply_patch", _)
            if msg_lower.contains("search") || msg_lower.contains("not found") =>
        {
            "HINT: SEARCH 块匹配失败。请先 read_file 获取文件当前内容，确保 SEARCH 块与文件中完全一致的代码段匹配（包括缩进和空格）。"
        }
        ("edit_file", _) | ("apply_patch", _)
            if msg_lower.contains("no such") || msg_lower.contains("not found") =>
        {
            "HINT: 要修改的文件不存在。请先使用 read_file 确认文件路径和当前内容，再进行修改。"
        }

        // -- Document operations --
        ("create_document", Some("forbidden_type")) => {
            "HINT: 该文档类型不属于本部门职责。内阁出流程请用 submit_pipeline_plan，不要 create_document(type=\"plan\")；plan/dsgn 由中书令创建，revw 由门下侍中创建。"
        }
        ("create_document", _)
            if msg_lower.contains("type")
                && (msg_lower.contains("invalid") || msg_lower.contains("illegal")) =>
        {
            "HINT: 文档类型不合法。请使用以下之一：dsgn（设计文档）、plan（计划）、pdsg（阶段设计）、revw（审核报告）、anls（分析文档）、rprt（工作报告）。"
        }
        ("read_document", Some("not_found")) | ("read_document", Some("empty_id")) => {
            "HINT: 文档 ID 不存在或格式错误。先用 list_dir 浏览 .shuji/designs，从文件名得到 ID（如 dsgn_3.md → id=\"dsgn_3\"，不要带 .md）。不要用 context/pipeline 下的 JSON 文件名。"
        }
        ("read_document", _)
            if msg_lower.contains("not found") || msg_lower.contains("does not exist") =>
        {
            "HINT: 文档 ID 不存在。先用 list_dir 浏览 .shuji/designs，使用返回行中的 id=\"...\" 参数调用 read_document。"
        }
        ("append_document", Some("doc_not_approved")) => {
            "HINT: 该文档引用的审批文档尚未通过审批。这是系统门禁异常——如果目标是 revw 文档，请勿创建新的 revw 重试。如果是非 revw 文档引用了未审批的 revw，请等待审批完成后再操作。"
        }
        ("append_document", _) if msg_lower.contains("not found") => {
            "HINT: 要追加的文档 ID 不存在。请先使用 create_document 创建文档，然后再追加内容。"
        }

        // -- Route operations --
        ("route_to", Some("unknown_target")) => {
            "HINT: 目标部门名称无法识别。请使用中文全称：内阁、中书令、门下侍中、尚书令、吏部、兵部、工部、刑部、礼部。"
        }
        ("route_to", Some("doc_not_approved")) => {
            "HINT: 路由被拒绝，因为目标文档涉及的内容尚未通过审批。请先完成审批流程。"
        }

        (_, Some("ROLE_GATE")) | (_, Some("CONTRACT_TOOL")) => {
            "HINT: 该工具不在本部门职责范围内。请改用文档工具产出交付物，或通过尚书令/内阁调度下游部门。"
        }

        // -- Command operations --
        ("execute_command", _)
            if msg_lower.contains("not found") || msg_lower.contains("no such") =>
        {
            "HINT: 命令不存在或找不到可执行文件。请检查命令名称和路径是否正确。"
        }

        // -- Unknown tool --
        ("unknown_tool", _) => {
            "HINT: 调用了不存在的工具。请检查工具名称拼写。可用工具列表在 system prompt 中有定义。"
        }

        // Default: generic hint based on tool category
        _ => {
            if matches!(name, "read_file" | "read_document" | "list_dir" | "search_text") {
                "HINT: 请确认参数正确，或检查目标文件/文档是否存在。"
            } else if matches!(name, "create_file" | "create_document") {
                "HINT: 请检查参数是否完整（路径、内容等），或确认目标路径不重复。"
            } else if matches!(name, "edit_file" | "apply_patch" | "modify_file") {
                "HINT: 请先 read_file 获取文件最新内容，确保修改基准确确。"
            } else {
                "HINT: 请检查工具参数后重试。如果持续失败，考虑换一种实现方式。"
            }
        }
    };

    // Insert the hint into the JSON result: add a "hint" field inside the existing object
    if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(raw_result) {
        if let Some(obj) = v.as_object_mut() {
            obj.insert(
                "hint".to_string(),
                serde_json::Value::String(hint.to_string()),
            );
            return serde_json::to_string(obj).unwrap_or_else(|_| raw_result.to_string());
        }
    }
    raw_result.to_string()
}
