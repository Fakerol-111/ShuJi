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
    super::truncate::truncate_tool_result_by_name(name, content)
}

/// Central tool dispatch: all agents call this instead of writing their own match block.
pub async fn execute_named_tool(
    name: &str,
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    if let Err(reason) = crate::config::esaa_contract::check_dispatch_tool_gate(dept, name) {
        return crate::tool::output::ToolOutput::error(name, "", "ROLE_GATE", &reason);
    }
    tool_log::log_tool_call(dept, name, args, working_dir).await;
    let raw_result = match name {
        "read_file" => {
            if args
                .get("path")
                .and_then(|v| v.as_str())
                .is_none_or(|p| p.is_empty())
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
                .is_none_or(|p| p.is_empty())
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
            Some("approved") => documents::tool_set_document_status(working_dir, args).await,
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
            // Gate: check refs before appending to a document.
            // Uses append-level check which allows in_review revw docs to be written.
            let id = args["id"].as_str().unwrap_or("");
            if !id.is_empty() {
                if let Err(msg) =
                    documents::check_doc_refs_approved_for_append(working_dir, id).await
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
        "check_compile" => crate::tool::command_ops::tool_check_compile(working_dir, args).await,
        "run_lint" => crate::tool::lint_ops::tool_run_lint(working_dir, args).await,
        "setup_test_env" => crate::tool::test_env::tool_setup_test_env(working_dir, args).await,
        "execute_command" => tool_execute_command(working_dir, args, dept).await,
        "summarize_logs" => tool_summarize_logs(working_dir, args).await,
        // Legacy route_to — NOT the primary orchestration path.
        //
        // Main flow: 内阁 submit_pipeline_plan → PipelineEngine schedules departments.
        // route_to remains for:
        //   - Pipeline plan steps with action "route_to" (engine-internal dispatch)
        //   - request_reauth (audit_tools) — the only active agent-level producer
        //   - Actor spawn output parsing when neige emits route_to in tool results
        // Do not extend route_to as a new orchestration mechanism.
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
    // Record heartbeat regardless of result
    crate::runtime_notify::record_tool_call(name);

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
                    crate::runtime_notify::record_write(&full);
                }
            }
        }
        "create_document" | "modify_document" | "append_document" | "set_document_status" => {
            // Invalidate .shuji/ dir since document listings and reads may change
            let shuji_dir = working_dir.join(".shuji");
            cache_invalidate(working_dir, &shuji_dir);
            if raw_result.contains("\"ok\":true") {
                let doc_id = super::cache::doc_id_from_write_result(name, args, &raw_result);
                if let Some(id) = &doc_id {
                    crate::audit::sync_ref_index(working_dir, id).await;
                }
                crate::runtime_notify::record_write(&shuji_dir);
            }
        }
        _ => {}
    }
    truncate_tool_result_by_name(name, &augment_error_with_hint(name, &raw_result, dept))
}

/// Classify a tool error and append an actionable correction hint for the LLM.
/// The hint is appended to the JSON result so the LLM knows what to do differently.
fn augment_error_with_hint(name: &str, raw_result: &str, dept: &str) -> String {
    super::hints::augment_error_with_hint(name, raw_result, dept)
}

fn handle_route_to(args: &serde_json::Value, dept: &str) -> String {
    super::legacy_route::handle_route_to(args, dept)
}

// ── summarize_logs ───────────────────────────────────────────
pub async fn tool_summarize_logs(working_dir: &Path, args: &serde_json::Value) -> String {
    super::tool_defs::tool_summarize_logs(working_dir, args).await
}

pub fn summarize_logs_tool_def() -> crate::api::client::ToolDefinition {
    super::tool_defs::summarize_logs_tool_def()
}

// ── request_decision ─────────────────────────────────────────
pub fn request_decision_tool_def() -> crate::api::client::ToolDefinition {
    super::tool_defs::request_decision_tool_def()
}

pub async fn tool_request_decision(args: &serde_json::Value) -> String {
    super::tool_defs::tool_request_decision(args).await
}
