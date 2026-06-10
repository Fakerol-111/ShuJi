use std::path::Path;

use crate::tool::ToolOutput;

// ── Audit tools (礼部 + 尚书令) ─────────────────────────────

pub async fn tool_init_checklist(args: &serde_json::Value, working_dir: &Path) -> String {
    let category = args["category"].as_str().unwrap_or("general");
    let msg = crate::audit::init_checklist(working_dir, category).await;
    serde_json::json!({"ok": true, "message": msg}).to_string()
}

pub async fn tool_update_checklist_item(args: &serde_json::Value, working_dir: &Path) -> String {
    let id = args["id"].as_str().unwrap_or("");
    let status = args["status"].as_str().unwrap_or("");
    let note = args["note"].as_str().unwrap_or("");
    if id.is_empty() || status.is_empty() {
        return serde_json::json!({"ok": false, "message": "id 和 status 不能为空"}).to_string();
    }
    match crate::audit::update_checklist_item(working_dir, id, status, note).await {
        Ok(msg) => serde_json::json!({"ok": true, "message": msg}).to_string(),
        Err(e) => serde_json::json!({"ok": false, "message": e}).to_string(),
    }
}

pub async fn tool_add_violation(args: &serde_json::Value, working_dir: &Path) -> String {
    let severity = args["severity"].as_str().unwrap_or("warning");
    let rule_id = args["rule_id"].as_str().unwrap_or("");
    let location = args["location"].as_str().unwrap_or("");
    let description = args["description"].as_str().unwrap_or("");
    if rule_id.is_empty() || description.is_empty() {
        return serde_json::json!({"ok": false, "message": "rule_id 和 description 不能为空"})
            .to_string();
    }
    crate::audit::add_violation(working_dir, severity, rule_id, location, description).await;
    serde_json::json!({"ok": true, "message": format!("违规记录已添加: {} — {}", rule_id, description)}).to_string()
}

pub async fn tool_request_reauth(args: &serde_json::Value, working_dir: &Path) -> String {
    let subject = args["subject"].as_str().unwrap_or("");
    let reason = args["reason"].as_str().unwrap_or("");
    if subject.is_empty() || reason.is_empty() {
        return serde_json::json!({"ok": false, "message": "subject 和 reason 不能为空"})
            .to_string();
    }
    let _ = crate::audit::request_reauth(working_dir, subject, reason).await;
    // Return route_to operation so the AgentController automatically routes to the target
    let msg = format!(
        "已提交复验请求，自动路由到 {} 进行重新审计。{}",
        "礼部", reason
    );
    ToolOutput::success("route_to", subject, &msg)
}
