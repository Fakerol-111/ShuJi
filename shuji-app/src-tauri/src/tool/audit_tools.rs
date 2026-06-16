use std::path::Path;

use crate::tool::ToolOutput;

// ── Audit tools (Liburshangshu + Shangshuling) ─────────────────────────────

pub async fn tool_init_checklist(args: &serde_json::Value, working_dir: &Path) -> String {
    let category = args["category"].as_str().unwrap_or("general");

    // If checklist is empty and category is "general", try precepts-based init
    let existing = crate::audit::load_checklist(working_dir).await;
    if existing.items.is_empty() && category == "general" {
        let rules = crate::precepts::load_project_rules(working_dir);
        if !rules.is_empty() {
            let items = crate::precepts::rules_to_checklist_items(&rules);
            let checklist = crate::audit::Checklist { items };
            crate::audit::save_checklist(working_dir, &checklist).await;
            return format!("Loaded {} check items from precepts", checklist.items.len());
        }
    }

    let msg = crate::audit::init_checklist(working_dir, category).await;
    serde_json::json!({"ok": true, "message": msg}).to_string()
}

pub async fn tool_update_checklist_item(args: &serde_json::Value, working_dir: &Path) -> String {
    let id = args["id"].as_str().unwrap_or("");
    let status = args["status"].as_str().unwrap_or("");
    let note = args["note"].as_str().unwrap_or("");
    if id.is_empty() || status.is_empty() {
        return serde_json::json!({"ok": false, "message": "id and status cannot be empty"})
            .to_string();
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
        return serde_json::json!({"ok": false, "message": "rule_id and description cannot be empty"})
            .to_string();
    }
    crate::audit::add_violation(working_dir, severity, rule_id, location, description).await;
    serde_json::json!({"ok": true, "message": format!("Violation recorded: {} — {}", rule_id, description)}).to_string()
}

pub async fn tool_request_reauth(args: &serde_json::Value, working_dir: &Path) -> String {
    let subject = args["subject"].as_str().unwrap_or("");
    let reason = args["reason"].as_str().unwrap_or("");
    if subject.is_empty() || reason.is_empty() {
        return serde_json::json!({"ok": false, "message": "subject and reason cannot be empty"})
            .to_string();
    }
    let _ = crate::audit::request_reauth(working_dir, subject, reason).await;
    // Return route_to operation so the AgentController automatically routes to the target
    let msg = format!(
        "Re-audit request submitted, auto-routing to {} for re-inspection. {}",
        "Liburshangshu", reason
    );
    ToolOutput::success("route_to", subject, &msg)
}
