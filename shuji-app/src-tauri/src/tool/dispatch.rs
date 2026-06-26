use std::path::Path;

use crate::tool::audit_tools::{
    tool_add_violation, tool_init_checklist, tool_request_reauth, tool_update_checklist_item,
};
use crate::tool::cache::cache_invalidate;
use crate::tool::command_ops::{tool_execute_command, tool_run_tests};
use crate::tool::documents;
use crate::tool::file_ops::{
    tool_append_file, tool_apply_patch, tool_create_file, tool_delete_file, tool_edit_file,
    tool_list_dir, tool_list_dir_tree, tool_modify_file, tool_read_file, tool_rename_file,
    tool_search_text,
};
use crate::tool::path::resolve_scoped_path;
use crate::tool::tool_log;
use crate::tool::ToolOutput;

/// Truncate verbose tool results to avoid blowing up context.
/// Uses per-tool limits. Truncated results include a hint for continuation.
pub fn truncate_tool_result_by_name(name: &str, content: &str) -> String {
    let limit = match name {
        "read_file" | "read_document" => 8000,
        "search_text" => 8000,
        "list_dir" => 8000,
        "execute_command" => 4000,
        "run_tests" => 4000,
        "run_lint" => 4000,
        "summarize_logs" => 4000,
        _ => 16000,
    };
    if content.len() > limit {
        let head: String = content.chars().take(limit).collect();
        format!(
            "{}...\n[Truncated: showing first {} of {} chars. To continue, narrow your search and retry (truncated: true)]",
            head,
            limit,
            content.len()
        )
    } else {
        content.to_string()
    }
}

/// Central tool dispatch: all agents call this instead of writing their own match block.
pub async fn execute_named_tool(
    name: &str,
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    tool_log::log_tool_call(dept, name, args, working_dir).await;
    let raw_result = match name {
        "read_file" => {
            if args
                .get("path")
                .and_then(|v| v.as_str())
                .map_or(true, |p| p.is_empty())
            {
                crate::tool::output::ToolOutput::error(
                    "read_file",
                    "",
                    "empty_path",
                    "请指定要读取的文件路径（path 参数不能为空）。",
                )
            } else {
                tool_read_file(working_dir, args).await
            }
        }
        "create_file" => {
            if args
                .get("path")
                .and_then(|v| v.as_str())
                .map_or(true, |p| p.is_empty())
            {
                crate::tool::output::ToolOutput::error(
                    "create_file",
                    "",
                    "empty_path",
                    "请指定要创建的文件路径（path 参数不能为空）。",
                )
            } else {
                tool_create_file(working_dir, args).await
            }
        }
        "list_dir" => tool_list_dir(working_dir, args).await,
        "list_dir_tree" => tool_list_dir_tree(working_dir, args).await,
        "append_file" => tool_append_file(working_dir, args).await,
        "delete_file" => tool_delete_file(working_dir, args).await,
        "rename_file" => tool_rename_file(working_dir, args).await,
        "modify_file" => tool_modify_file(working_dir, args).await,
        "apply_patch" => tool_apply_patch(working_dir, args).await,
        "edit_file" => tool_edit_file(working_dir, args).await,
        "create_document" => documents::tool_create_document(working_dir, args, dept).await,
        "modify_document" => documents::tool_modify_document(working_dir, args, dept).await,
        "set_document_status" => match args.get("status").and_then(|v| v.as_str()) {
            Some(s) if s == "approved" => {
                documents::tool_set_document_status(working_dir, args).await
            }
            Some(other) => crate::tool::output::ToolOutput::error(
                "set_document_status",
                other,
                "invalid_status",
                &format!("状态值不合法：必须是 approved。当前值: {other}"),
            ),
            None => crate::tool::output::ToolOutput::error(
                "set_document_status",
                "",
                "missing_status",
                "请指定状态值（status 参数）。合法值: approved",
            ),
        },
        "append_document" => {
            // Gate: check refs before appending to a document
            let id = args["id"].as_str().unwrap_or("");
            if !id.is_empty() {
                if let Err(msg) =
                    documents::check_doc_refs_approved_for_route(working_dir, id).await
                {
                    ToolOutput::error("append_document", id, "doc_not_approved", &msg)
                } else {
                    documents::tool_append_document(working_dir, args, dept).await
                }
            } else {
                documents::tool_append_document(working_dir, args, dept).await
            }
        }
        "find_document" => {
            // P0-2: find_document is deprecated — redirect to read_document
            let id = args["id"].as_str().unwrap_or("");
            ToolOutput::success_raw("find_document",
                &format!("find_document is deprecated. Use read_document(id=\"{}\") instead — it finds + reads in one call.", id))
        }
        "read_document" => documents::tool_read_document(working_dir, args).await,
        "search_text" => tool_search_text(working_dir, args).await,
        "run_tests" => tool_run_tests(working_dir, args).await,
        "run_lint" => crate::tool::lint_ops::tool_run_lint(working_dir, args).await,
        "setup_test_env" => crate::tool::test_env::tool_setup_test_env(working_dir, args).await,
        "execute_command" => tool_execute_command(working_dir, args, dept).await,
        "summarize_logs" => tool_summarize_logs(working_dir, args).await,
        // Legacy route_to — NOT the primary orchestration path.
        //
        // Main flow: 内阁 submit_pipeline_plan → PipelineEngine schedules departments.
        // route_to remains for:
        //   - Pipeline plan steps with action "route_to" (engine-internal dispatch)
        //   - 尚书令 and execution departments forwarding work inside a running task
        //   - Actor spawn output parsing when an agent still emits route_to in tool results
        // Do not extend route_to as a new cabinet-level orchestration mechanism.
        "route_to" => {
            // Gate: check refs before routing to execution departments
            let exec_depts = ["尚书令", "吏部", "兵部", "工部", "刑部", "礼部"];
            let to_name = args["to"].as_str().unwrap_or("");
            let subject = args["subject"].as_str().unwrap_or("");
            if exec_depts.contains(&to_name) && !subject.is_empty() {
                if let Err(msg) =
                    documents::check_doc_refs_approved_for_route(working_dir, subject).await
                {
                    ToolOutput::error("route_to", subject, "doc_not_approved", &msg)
                } else {
                    handle_route_to(args, dept)
                }
            } else {
                handle_route_to(args, dept)
            }
        }
        "route" => ToolOutput::success_raw(
            "route",
            "Please use the route_to tool instead of outputting a raw route tag.",
        ),
        "init_checklist" => tool_init_checklist(args, working_dir).await,
        "update_checklist_item" => tool_update_checklist_item(args, working_dir).await,
        "add_violation" => tool_add_violation(args, working_dir).await,
        "request_reauth" => tool_request_reauth(args, working_dir).await,
        "request_decision" => tool_request_decision(args).await,
        _ => ToolOutput::error("unknown_tool", name, "unknown_tool", "Unknown tool"),
    };
    // P2-2: Invalidate read cache after write operations
    match name {
        "create_file" | "modify_file" | "append_file" | "delete_file" | "rename_file"
        | "apply_patch" | "edit_file" => {
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                if let Ok(full) = resolve_scoped_path(working_dir, path).await {
                    cache_invalidate(working_dir, &full);
                    // Also invalidate parent directory (list_dir results change)
                    if let Some(parent) = full.parent() {
                        cache_invalidate(working_dir, parent);
                    }
                }
            }
        }
        "create_document" | "modify_document" | "append_document" | "set_document_status" => {
            // Invalidate .shuji/ dir since document listings and reads may change
            let shuji_dir = working_dir.join(".shuji");
            cache_invalidate(working_dir, &shuji_dir);
            if raw_result.contains("\"ok\":true") {
                if let Some(doc_id) = doc_id_from_write_result(name, args, &raw_result) {
                    crate::audit::sync_ref_index(working_dir, &doc_id).await;
                }
            }
        }
        _ => {}
    }
    truncate_tool_result_by_name(name, &augment_error_with_hint(name, &raw_result, dept))
}

/// Classify a tool error and append an actionable correction hint for the LLM.
/// The hint is appended to the JSON result so the LLM knows what to do differently.
fn augment_error_with_hint(name: &str, raw_result: &str, _dept: &str) -> String {
    // Only augment error results
    if !crate::tool::output::ToolOutput::is_error(raw_result) {
        return raw_result.to_string();
    }

    let error_code = crate::tool::output::ToolOutput::error_code(raw_result);
    let message = crate::tool::output::ToolOutput::extract_message(raw_result).unwrap_or_default();
    let msg_lower = message.to_lowercase();

    let hint = match (name, error_code.as_deref()) {
        // -- File operations --
        ("read_file", _) if msg_lower.contains("not found") || msg_lower.contains("no such file") => {
            "HINT: 文件不存在。请先使用 list_dir 确认文件路径，或检查路径拼写。如果文件可能在其他位置，使用 search_text 搜索。"
        }
        ("create_file", _) if msg_lower.contains("already exists") => {
            "HINT: 文件已存在。请使用 edit_file（单处修改）或 apply_patch（多处修改）来更新现有文件，不要重复创建。"
        }
        ("edit_file", _) | ("apply_patch", _) if msg_lower.contains("search") || msg_lower.contains("not found") => {
            "HINT: SEARCH 块匹配失败。请先 read_file 获取文件当前内容，确保 SEARCH 块与文件中完全一致的代码段匹配（包括缩进和空格）。"
        }
        ("edit_file", _) | ("apply_patch", _) if msg_lower.contains("no such") || msg_lower.contains("not found") => {
            "HINT: 要修改的文件不存在。请先使用 read_file 确认文件路径和当前内容，再进行修改。"
        }

        // -- Document operations --
        ("create_document", Some("forbidden_type")) => {
            "HINT: 该文档类型不属于本部门职责。内阁出流程请用 submit_pipeline_plan，不要 create_document(type=\"plan\")；plan/dsgn 由中书令创建，revw 由门下侍中创建。"
        }
        ("create_document", _) if msg_lower.contains("type") && (msg_lower.contains("invalid") || msg_lower.contains("illegal")) => {
            "HINT: 文档类型不合法。请使用以下之一：dsgn（设计文档）、plan（计划）、pdsg（阶段设计）、revw（审核报告）、anls（分析文档）、rprt（工作报告）。"
        }
        ("read_document", Some("not_found")) | ("read_document", Some("empty_id")) => {
            "HINT: 文档 ID 不存在或格式错误。先用 list_dir 浏览 .shuji/designs，从文件名得到 ID（如 dsgn_3.md → id=\"dsgn_3\"，不要带 .md）。不要用 context/pipeline 下的 JSON 文件名。"
        }
        ("read_document", _) if msg_lower.contains("not found") || msg_lower.contains("does not exist") => {
            "HINT: 文档 ID 不存在。先用 list_dir 浏览 .shuji/designs，使用返回行中的 id=\"...\" 参数调用 read_document。"
        }
        ("append_document", Some("doc_not_approved")) => {
            "HINT: 该文档引用的内容尚未通过审批。请先完成审批流程（使用 set_document_status 工具），然后再追加内容。"
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

        // -- Command operations --
        ("execute_command", _) if msg_lower.contains("not found") || msg_lower.contains("no such") => {
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

/// Resolve doc id from tool args or success JSON (`path` field for create_document).
fn doc_id_from_write_result(
    name: &str,
    args: &serde_json::Value,
    raw_result: &str,
) -> Option<String> {
    if name != "create_document" {
        if let Some(id) = args.get("id").and_then(|v| v.as_str()) {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw_result) {
        if v.get("ok").and_then(|o| o.as_bool()) == Some(true) {
            if let Some(path) = v.get("path").and_then(|p| p.as_str()) {
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }
    }
    None
}

/// Validate and execute route_to — returns a ToolOutput with `operation: "route_to"`
/// so the AgentController can detect it in the result and break the tool loop.
fn handle_route_to(args: &serde_json::Value, dept: &str) -> String {
    let to_name = args["to"].as_str().unwrap_or("");
    if to_name.is_empty() {
        return ToolOutput::error(
            "route_to",
            "",
            "missing_target",
            "Missing target department (to parameter)",
        );
    }
    let subject = args["subject"].as_str().unwrap_or("");
    if subject.is_empty() {
        return ToolOutput::error(
            "route_to",
            "",
            "missing_subject",
            "Missing document ID (subject parameter)",
        );
    }
    let _type = args["type"].as_str().unwrap_or("task");
    if !matches!(_type, "task" | "replace" | "interrupt") {
        return ToolOutput::error(
            "route_to",
            "",
            "invalid_type",
            &format!(
                "Invalid route type: {}, must be task/replace/interrupt",
                _type
            ),
        );
    }
    // P1-3: 路由预验证 - 检查目标部门是否合法
    if crate::models::role::Role::from_name(to_name).is_none() {
        return ToolOutput::error(
            "route_to",
            to_name,
            "unknown_target",
            &format!("无法识别目标部门 '{}' 可用部门: 内阁、中书令、门下侍中、尚书令、吏部、兵部、工部、刑部、礼部。请使用中文全称或英文名称(cabinet/architect/reviewer/personnel/war/works/justice/rites)。",
                to_name
            ),
        );
    }
    let _ = dept;
    ToolOutput::success(
        "route_to",
        "",
        &format!("Route to {} ({}): {}", to_name, _type, subject),
    )
}

// ── summarize_logs ───────────────────────────────────────────

/// Read `.shuji/logs/activity.log`, parse JSON lines, return as formatted text.
pub async fn tool_summarize_logs(working_dir: &Path, args: &serde_json::Value) -> String {
    let log_path = working_dir.join(".shuji").join("logs").join("activity.log");
    if !log_path.exists() {
        return ToolOutput::success_raw("summarize_logs", "No log entries yet");
    }

    let content = match tokio::fs::read_to_string(&log_path).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("summarize_logs", "", "read_error", &e.to_string()),
    };

    let since = args["since"].as_u64().unwrap_or(0) as usize;
    let mut entries: Vec<serde_json::Value> = Vec::new();
    let total_lines;

    {
        let lines: Vec<&str> = content.lines().collect();
        total_lines = lines.len();
        for line in lines.iter().skip(since) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                entries.push(val);
            }
        }
    }

    if entries.is_empty() {
        return ToolOutput::success_raw("summarize_logs", "No log entries yet");
    }

    let mut result = Vec::new();
    result.push(format!(
        "{} log entries (file has {} lines, starting from line {}):",
        entries.len(),
        total_lines,
        since
    ));
    result.push(String::new());

    for entry in &entries {
        let ts = entry["ts"].as_str().unwrap_or("?");
        let author = entry["author"].as_str().unwrap_or("?");
        let summary = entry["summary"].as_str().unwrap_or("");

        let short_ts = if ts.len() > 19 { &ts[..19] } else { ts };
        result.push(format!("[{}] {}: {}", short_ts, author, summary));
    }

    ToolOutput::success_raw("summarize_logs", &result.join("\n"))
}

/// Tool definition for summarize_logs.
pub fn summarize_logs_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "summarize_logs".into(),
            description: "Read activity.log; can incrementally read by line number".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "since": {
                        "type": "integer",
                        "description": "Starting line number (0-based), omit to read from the beginning"
                    }
                }
            }),
        },
    }
}

// ── request_decision ─────────────────────────────────────────

/// Tool for 内阁 to request emperor decision with structured options.
pub fn request_decision_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "request_decision".into(),
            description: "Call when the emperor's decision is needed. Pass a list of options for the emperor to choose from. Must include context before the options explaining why a decision is needed.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "options": {
                        "type": "array",
                        "description": "List of options for the emperor to choose from (at least 1)",
                        "items": {"type": "string"},
                        "minItems": 1
                    }
                },
                "required": ["options"]
            }),
        },
    }
}

pub async fn tool_request_decision(args: &serde_json::Value) -> String {
    let options = match args["options"].as_array() {
        Some(arr) if !arr.is_empty() => arr,
        _ => {
            return ToolOutput::error(
                "request_decision",
                "",
                "empty_options",
                "options cannot be empty",
            )
        }
    };
    let mut msg = "[Waiting for emperor's decision] Please choose one:\n".to_string();
    for (i, opt) in options.iter().enumerate() {
        let text = opt.as_str().unwrap_or("(invalid option)");
        msg.push_str(&format!("{}. {}\n", i + 1, text));
    }
    ToolOutput::success_raw("request_decision", &msg.trim())
}
