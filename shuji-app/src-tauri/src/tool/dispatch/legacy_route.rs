//! Legacy route_to compatibility handler.
//! Extracted from dispatch.rs. Only reached via `request_reauth` (audit_tools).

use crate::tool::ToolOutput;

/// Validate and execute route_to — returns a ToolOutput with `operation: "route_to"`.
///
/// This function is reached today only via `request_reauth` (audit_tools)
/// which fakes `operation: "route_to"` to trigger the AgentController's
/// routing detection path. Agent-level `route_to` tool calls have been removed
/// (M2 milestone) but the dispatch arm is retained for request_reauth compat.
pub fn handle_route_to(args: &serde_json::Value, dept: &str) -> String {
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
            &format!(
                "无法识别目标部门 '{}' 可用部门: 内阁、中书令、门下侍中、尚书令、吏部、兵部、工部、刑部、礼部。请使用中文全称或英文名称(cabinet/architect/reviewer/personnel/war/works/justice/rites)。",
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
