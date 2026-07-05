use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::actor::FastMessage;
use crate::models::role::Role;
use crate::tool::ToolContext;

/// Dispatch handler for 内阁's special tools. Returns `Some(result)` if
/// the tool name matches, `None` to fall through to the normal tool dispatch.
pub async fn tool_handle_neige_special(
    name: &str,
    args: &serde_json::Value,
    ctx: &ToolContext,
) -> Option<String> {
    match name {
        "cancel_agent" => Some(tool_cancel_agent(args, ctx).await),
        "update_soul" => Some(tool_update_soul(args, ctx).await),
        "expand_requirements" => Some(tool_expand_requirements(args, ctx).await),
        "survey_codebase" => Some(tool_survey_codebase(args, ctx).await),
        "create_skill" => Some(tool_create_skill(args, ctx).await),
        "submit_pipeline_plan" => Some(tool_submit_pipeline_plan(args, ctx).await),
        _ => None,
    }
}

async fn tool_submit_pipeline_plan(args: &serde_json::Value, _ctx: &ToolContext) -> String {
    let plan_json = args["plan_json"].as_str().unwrap_or("");
    if plan_json.is_empty() {
        return r#"{"ok": false, "message": "plan_json cannot be empty"}"#.to_string();
    }

    // Validate against JSON Schema + Rust-level checks
    match crate::pipeline::schema::validate_plan_json(plan_json) {
        Ok(plan) => {
            log_console!(
                "[tool] submit_pipeline_plan accepted — plan '{}' ({} steps)",
                plan.plan_id,
                plan.steps.len()
            );
            serde_json::json!({"ok": true, "message": "Pipeline plan submitted, engine takes over execution", "plan_json": plan_json}).to_string()
        }
        Err(e) => {
            log_console!("[tool] submit_pipeline_plan REJECTED: {}", e);
            serde_json::json!({
                "ok": false,
                "message": format!("Pipeline plan validation failed: {}", e.message),
                "field_path": e.field_path,
                "error_type": "schema_validation"
            })
            .to_string()
        }
    }
}

async fn tool_cancel_agent(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let target = args["to"].as_str().unwrap_or("");
    if let Some(role) = Role::from_name(target) {
        // Set cancel flag (无锁直接查找)
        if let Some(ref map) = ctx.cancel_map {
            if let Some(flag) = map.get(&role) {
                flag.store(true, Ordering::SeqCst);
            }
        }
        // Send fast interrupt to immediately stop tool execution
        if let Some(ref fast_txs) = ctx.fast_txs {
            if let Some(tx) = fast_txs.get(&role) {
                let _ = tx.try_send(FastMessage::Interrupt);
            }
        }
        log_console!("[tool] cancel_agent → {} interrupted", target);
        crate::audit::append(
            &ctx.working_dir,
            "cancel_agent",
            "Neige",
            target,
            "cancel_agent",
        )
        .await;
        return serde_json::json!({"ok": true, "message": format!("Interrupted {}'s current operation", target)})
            .to_string();
    }
    serde_json::json!({"ok": false, "message": format!("Cannot interrupt: {}", target)}).to_string()
}

async fn tool_update_soul(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let content = args["content"].as_str().unwrap_or("");
    if content.is_empty() {
        return r#"{"ok": false, "message": "content cannot be empty"}"#.to_string();
    }

    let target_role = match crate::learning::normalize_role_name(args["role"].as_str()) {
        Ok(r) => r,
        Err(e) => {
            return serde_json::json!({"ok": false, "message": e}).to_string();
        }
    };

    let evidence: Vec<String> = args["evidence"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if target_role != "Neige" && evidence.is_empty() {
        return r#"{"ok": false, "message": "Writing for another role requires evidence"}"#
            .to_string();
    }

    let tags: Vec<String> = args["tags"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let kind = crate::learning::LearningKind::from_section_or_kind(
        args["section"].as_str(),
        args["kind"].as_str(),
    )
    .unwrap_or(crate::learning::LearningKind::Experience);

    let scope = match args["scope"].as_str().unwrap_or("project") {
        "global_candidate" => crate::learning::LearningScope::GlobalCandidate,
        "global" => crate::learning::LearningScope::Global,
        _ => crate::learning::LearningScope::Project,
    };

    match crate::learning::SoulStore::append_entry(
        &ctx.working_dir,
        &target_role,
        kind,
        scope,
        content,
        evidence,
        tags,
    )
    .await
    {
        Ok(msg) => {
            log_console!(
                "[tool] update_soul → {} (role={}, kind={:?})",
                content,
                target_role,
                kind
            );

            if scope == crate::learning::LearningScope::Project {
                let soul_path =
                    crate::learning::SoulStore::project_soul_path(&ctx.working_dir, &target_role);
                if let Ok(metadata) = tokio::fs::metadata(&soul_path).await {
                    if metadata.len() > crate::learning::MAX_SOUL_FILE_BYTES as u64 {
                        if let (Some(client), Some(model)) =
                            (ctx.client.as_ref(), ctx.model.as_ref())
                        {
                            match crate::learning::SoulStore::compact_project_soul_with_llm(
                                &ctx.working_dir,
                                &target_role,
                                client,
                                model,
                            )
                            .await
                            {
                                Ok(compact_msg) => {
                                    return serde_json::json!({"ok": true, "message": format!("{}. {}", msg, compact_msg)})
                                        .to_string();
                                }
                                Err(e) => {
                                    log_console!("[tool] soul compaction failed: {}", e);
                                }
                            }
                        }
                    }
                }
            }

            serde_json::json!({"ok": true, "message": msg}).to_string()
        }
        Err(e) => serde_json::json!({"ok": false, "message": e}).to_string(),
    }
}

/// Extract 内阁's cancel flag from the ToolContext's cancel_map.
fn neige_cancel_flag(ctx: &ToolContext) -> Option<Arc<AtomicBool>> {
    ctx.cancel_map
        .as_ref()
        .and_then(|m| m.get(&Role::Neige).cloned())
}

async fn tool_expand_requirements(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let task_id = args["task_id"].as_str().unwrap_or("");
    if task_id.is_empty() {
        return r#"{"ok": false, "message": "task_id cannot be empty"}"#.to_string();
    }
    let client = match ctx.client.as_ref() {
        Some(c) => c,
        None => {
            return serde_json::json!({"ok": false, "message": "API client unavailable"})
                .to_string()
        }
    };
    let model = match ctx.model.as_ref() {
        Some(m) => m,
        None => {
            return serde_json::json!({"ok": false, "message": "Model unavailable"}).to_string()
        }
    };
    // Use Neige's cancel flag so sub-agent stops when "stop all agents" fires
    let cancel = neige_cancel_flag(ctx).unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    match crate::agent::expand_requirements::run(task_id, &ctx.working_dir, client, model, &cancel)
        .await
    {
        Ok(doc_id) => {
            log_console!("[tool] expand_requirements → {}", doc_id);
            serde_json::json!({"ok": true, "document_id": doc_id}).to_string()
        }
        Err(e) => {
            log_console!("[tool] expand_requirements failed: {}", e);
            serde_json::json!({"ok": false, "message": e}).to_string()
        }
    }
}

async fn tool_survey_codebase(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let task_description = args["task_description"].as_str().unwrap_or("");
    if task_description.is_empty() {
        return r#"{"ok": false, "message": "task_description cannot be empty"}"#.to_string();
    }
    let client = match ctx.client.as_ref() {
        Some(c) => c,
        None => {
            return serde_json::json!({"ok": false, "message": "API client unavailable"})
                .to_string()
        }
    };
    let model = match ctx.model.as_ref() {
        Some(m) => m,
        None => {
            return serde_json::json!({"ok": false, "message": "Model unavailable"}).to_string()
        }
    };
    let cancel = neige_cancel_flag(ctx).unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    match crate::agent::survey_codebase::run(
        task_description,
        &ctx.working_dir,
        client,
        model,
        &cancel,
    )
    .await
    {
        Ok(doc_id) => {
            log_console!("[tool] survey_codebase → {}", doc_id);
            serde_json::json!({"ok": true, "document_id": doc_id}).to_string()
        }
        Err(e) => {
            log_console!("[tool] survey_codebase failed: {}", e);
            serde_json::json!({"ok": false, "message": e}).to_string()
        }
    }
}

async fn tool_create_skill(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let skill_name = args["name"].as_str().unwrap_or("");
    let description = args["description"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if skill_name.is_empty() || content.is_empty() {
        return r#"{"ok": false, "message": "name and content cannot be empty"}"#.to_string();
    }
    if skill_name
        .chars()
        .any(|c| !c.is_alphanumeric() && c != '_' && c != '-')
    {
        return r#"{"ok": false, "message": "name can only contain letters, numbers, underscores, and hyphens"}"#
            .to_string();
    }
    let skills_dir = ctx.working_dir.join(".shuji").join("skills");
    let _ = tokio::fs::create_dir_all(&skills_dir).await;
    let skill_path = skills_dir.join(format!("{}.md", skill_name));
    let file_content = format!("# {}\n\n{}\n\n---\n\n{}", skill_name, description, content);
    match tokio::fs::write(&skill_path, &file_content).await {
        Ok(_) => {
            log_console!("[tool] create_skill → {} ({})", skill_name, description);
            serde_json::json!({
                "ok": true,
                "message": format!("Skill {} created", skill_name),
                "skill_name": skill_name
            })
            .to_string()
        }
        Err(e) => {
            serde_json::json!({"ok": false, "message": format!("Write failed: {}", e)}).to_string()
        }
    }
}
