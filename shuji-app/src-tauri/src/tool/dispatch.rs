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
            "{}...\n[截断：显示前 {} 字符，共 {} 字符。如需继续，请缩小范围后重试 (truncated: true)]",
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
        "read_file" => tool_read_file(working_dir, args).await,
        "create_file" => tool_create_file(working_dir, args).await,
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
        "set_document_status" => documents::tool_set_document_status(working_dir, args).await,
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
                &format!("find_document 已弃用。请改用 read_document(id=\"{}\")——一次调用即可查找+读取。", id))
        }
        "read_document" => documents::tool_read_document(working_dir, args).await,
        "search_text" => tool_search_text(working_dir, args).await,
        "run_tests" => tool_run_tests(working_dir, args).await,
        "run_lint" => crate::tool::lint_ops::tool_run_lint(working_dir, args).await,
        "setup_test_env" => crate::tool::test_env::tool_setup_test_env(working_dir, args).await,
        "execute_command" => tool_execute_command(working_dir, args, dept).await,
        "summarize_logs" => tool_summarize_logs(working_dir, args).await,
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
        "route" => {
            ToolOutput::success_raw("route", "请调用 route_to 工具，不要输出文本 route 标签。")
        }
        "init_checklist" => tool_init_checklist(args, working_dir).await,
        "update_checklist_item" => tool_update_checklist_item(args, working_dir).await,
        "add_violation" => tool_add_violation(args, working_dir).await,
        "request_reauth" => tool_request_reauth(args, working_dir).await,
        "request_decision" => tool_request_decision(args).await,
        _ => ToolOutput::error("unknown_tool", name, "unknown_tool", "未知工具"),
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
        }
        _ => {}
    }
    truncate_tool_result_by_name(name, &raw_result)
}

/// Validate and execute route_to — returns a ToolOutput with `operation: "route_to"`
/// so the AgentController can detect it in the result and break the tool loop.
fn handle_route_to(args: &serde_json::Value, dept: &str) -> String {
    let to_name = args["to"].as_str().unwrap_or("");
    if to_name.is_empty() {
        return ToolOutput::error("route_to", "", "missing_target", "缺少目标部门（to 参数）");
    }
    let subject = args["subject"].as_str().unwrap_or("");
    if subject.is_empty() {
        return ToolOutput::error(
            "route_to",
            "",
            "missing_subject",
            "缺少文档 ID（subject 参数）",
        );
    }
    let _type = args["type"].as_str().unwrap_or("task");
    if !matches!(_type, "task" | "replace" | "interrupt") {
        return ToolOutput::error(
            "route_to",
            "",
            "invalid_type",
            &format!("无效的路由类型: {}，必须是 task/replace/interrupt", _type),
        );
    }
    let _ = dept;
    ToolOutput::success(
        "route_to",
        "",
        &format!("路由到 {}（{}）：{}", to_name, _type, subject),
    )
}

// ── summarize_logs ───────────────────────────────────────────

/// Read `.shuji/logs/activity.log`, parse JSON lines, return as formatted text.
pub async fn tool_summarize_logs(working_dir: &Path, args: &serde_json::Value) -> String {
    let log_path = working_dir.join(".shuji").join("logs").join("activity.log");
    if !log_path.exists() {
        return ToolOutput::success_raw("summarize_logs", "暂无日志记录");
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
        return ToolOutput::success_raw("summarize_logs", "暂无日志记录");
    }

    let mut result = Vec::new();
    result.push(format!(
        "共 {} 条日志记录（文件共 {} 行，自第 {} 行开始）：",
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
            description: "读取 activity.log 日志，可按行号增量读取".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "since": {
                        "type": "integer",
                        "description": "起始行号（从 0 开始），不传则从开头读"
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
            description: "当需要皇帝决策时调用。传入选项列表供皇帝选择。必须在选项前附上上下文说明为什么需要决策。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "options": {
                        "type": "array",
                        "description": "供皇帝选择的选项列表（至少 1 项）",
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
        _ => return ToolOutput::error("request_decision", "", "empty_options", "options 不能为空"),
    };
    let mut msg = "【等待皇帝决策】请选择一项：\n".to_string();
    for (i, opt) in options.iter().enumerate() {
        let text = opt.as_str().unwrap_or("(无效选项)");
        msg.push_str(&format!("{}. {}\n", i + 1, text));
    }
    ToolOutput::success_raw("request_decision", &msg.trim())
}
