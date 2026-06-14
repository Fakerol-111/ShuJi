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
        return r#"{"ok": false, "message": "plan_json 不能为空"}"#.to_string();
    }

    // Validate against JSON Schema + Rust-level checks
    match crate::pipeline::schema::validate_plan_json(plan_json) {
        Ok(plan) => {
            log_console!(
                "[tool] submit_pipeline_plan accepted — plan '{}' ({} steps)",
                plan.plan_id,
                plan.steps.len()
            );
            serde_json::json!({"ok": true, "message": "管道计划已提交，引擎接管执行", "plan_json": plan_json}).to_string()
        }
        Err(e) => {
            log_console!("[tool] submit_pipeline_plan REJECTED: {}", e);
            serde_json::json!({
                "ok": false,
                "message": format!("管道计划校验失败: {}", e.message),
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
        // Set cancel flag
        if let Some(ref map) = ctx.cancel_map {
            if let Ok(guard) = map.lock() {
                if let Some(flag) = guard.get(&role) {
                    flag.store(true, Ordering::SeqCst);
                }
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
            "内阁",
            target,
            "cancel_agent 操作",
        )
        .await;
        return serde_json::json!({"ok": true, "message": format!("已中断 {} 的当前操作", target)})
            .to_string();
    }
    serde_json::json!({"ok": false, "message": format!("无法中断: {}", target)}).to_string()
}

async fn tool_update_soul(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let content = args["content"].as_str().unwrap_or("");
    if content.is_empty() {
        return r#"{"ok": false, "message": "content 不能为空"}"#.to_string();
    }
    if content.len() > 500 {
        return r#"{"ok": false, "message": "内容过长（最多500字符）"}"#.to_string();
    }
    let section = args["section"].as_str();
    let soul_dir = ctx.working_dir.join(".shuji").join("soul");
    let soul_path = soul_dir.join("neige.md");
    let _ = tokio::fs::create_dir_all(&soul_dir).await;

    let entry = format!("- {}\n", content);
    let result = if let Some(sec) = section {
        match tokio::fs::read_to_string(&soul_path).await {
            Ok(existing) => {
                let heading = format!("## {}", sec);
                if let Some(pos) = existing.find(&heading) {
                    let after_heading = &existing[pos + heading.len()..];
                    let next_heading = after_heading.find("\n## ");
                    let insert_pos =
                        pos + heading.len() + next_heading.unwrap_or(after_heading.len());
                    let mut new_content = existing[..insert_pos].to_string();
                    if !new_content.ends_with('\n') {
                        new_content.push('\n');
                    }
                    if !new_content.ends_with("\n\n") {
                        new_content.push('\n');
                    }
                    new_content.push_str(&entry);
                    new_content.push_str(&existing[insert_pos..]);
                    match tokio::fs::write(&soul_path, &new_content).await {
                        Ok(_) => Ok(format!("已记录到「{}」章节", sec)),
                        Err(e) => Err(e),
                    }
                } else {
                    use tokio::io::AsyncWriteExt;
                    match tokio::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .write(true)
                        .open(&soul_path)
                        .await
                    {
                        Ok(mut f) => {
                            let line = format!("\n## {}\n\n{}", sec, entry);
                            f.write_all(line.as_bytes()).await.ok();
                            Ok("已创建章节并记录".to_string())
                        }
                        Err(e) => Err(e),
                    }
                }
            }
            Err(_) => {
                let default = include_str!("../agent/neige/soul.md");
                let with_entry = format!("{}\n{}", default, entry);
                match tokio::fs::write(&soul_path, &with_entry).await {
                    Ok(_) => Ok("已记录".to_string()),
                    Err(e) => Err(e),
                }
            }
        }
    } else {
        use tokio::io::AsyncWriteExt;
        match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(&soul_path)
            .await
        {
            Ok(mut f) => {
                f.write_all(entry.as_bytes()).await.ok();
                Ok("已记录".to_string())
            }
            Err(e) => Err(e),
        }
    };

    match result {
        Ok(msg) => {
            log_console!(
                "[tool] update_soul → {} (section={})",
                content,
                section.unwrap_or("末尾")
            );

            // Check soul file size and auto-compact if > 8KB
            if let Ok(metadata) = tokio::fs::metadata(&soul_path).await {
                if metadata.len() > 8 * 1024 {
                    log_console!("[tool] soul 超过 8KB（{}），触发自动压缩", metadata.len());
                    match compact_soul_file(ctx).await {
                        Ok(compact_msg) => {
                            let full_msg = format!("{}. {}", msg, compact_msg);
                            return serde_json::json!({"ok": true, "message": full_msg})
                                .to_string();
                        }
                        Err(e) => {
                            log_console!("[tool] soul 压缩失败: {}", e);
                        }
                    }
                }
            }

            serde_json::json!({"ok": true, "message": msg}).to_string()
        }
        Err(e) => {
            serde_json::json!({"ok": false, "message": format!("写入失败: {}", e)}).to_string()
        }
    }
}

/// Compact soul file using LLM when it exceeds 8KB.
/// Summarizes into a concise version with core 10 items max.
async fn compact_soul_file(ctx: &ToolContext) -> Result<String, String> {
    let soul_path = ctx.working_dir.join(".shuji").join("soul").join("neige.md");
    let content = tokio::fs::read_to_string(&soul_path)
        .await
        .map_err(|e| format!("读取 soul 失败: {}", e))?;

    let client = ctx
        .client
        .clone()
        .ok_or("LLM 客户端未配置，无法压缩 soul")?;
    let model = ctx.model.clone().ok_or("LLM 模型未配置")?;

    let prompt = format!(
        r#"你是一个 soul 压缩工具。soul 是内阁首辅的经验/教训/偏好记录。

当前 soul {} 字节，已超过 8KB 上限。请提炼为核心版本，保留最有价值的条目。

要求：
- 保留 ## 经验 / ## 教训 / ## 偏好 三个章节
- 每章节不超过 5 条
- 保持原有格式（每条用 `- ` 开头）
- 去除重复或相似条目
- 总字符数不超过 4000

原始 soul：
{}"#,
        content.len(),
        content
    );

    let msg = crate::models::message::Message::user(&prompt);
    let compacted = client
        .send_message(
            "请压缩 soul 内容，输出精简版 Markdown（包含 ## 经验 / ## 教训 / ## 偏好）",
            &[msg],
            &model,
        )
        .await
        .map_err(|e| format!("LLM 压缩请求失败: {}", e))?
        .trim()
        .to_string();

    if compacted.is_empty() || compacted.len() >= content.len() {
        return Err("压缩结果无效或未减小".to_string());
    }

    tokio::fs::write(&soul_path, &compacted)
        .await
        .map_err(|e| format!("写入压缩后 soul 失败: {}", e))?;

    log_console!(
        "[tool] soul 压缩完成: {} → {} 字节",
        content.len(),
        compacted.len()
    );

    Ok(format!(
        "soul 已自动压缩（{} → {} 字节）",
        content.len(),
        compacted.len()
    ))
}

/// Extract 内阁's cancel flag from the ToolContext's cancel_map.
fn neige_cancel_flag(ctx: &ToolContext) -> Option<Arc<AtomicBool>> {
    ctx.cancel_map.as_ref().and_then(|m| {
        m.lock()
            .ok()
            .and_then(|guard| guard.get(&Role::Neige).cloned())
    })
}

async fn tool_expand_requirements(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let task_id = args["task_id"].as_str().unwrap_or("");
    if task_id.is_empty() {
        return r#"{"ok": false, "message": "task_id 不能为空"}"#.to_string();
    }
    let client = match ctx.client.as_ref() {
        Some(c) => c,
        None => return serde_json::json!({"ok": false, "message": "API客户端不可用"}).to_string(),
    };
    let model = match ctx.model.as_ref() {
        Some(m) => m,
        None => return serde_json::json!({"ok": false, "message": "模型不可用"}).to_string(),
    };
    // Use 内阁's cancel flag so sub-agent stops when "叫停诸司" fires
    let cancel = neige_cancel_flag(ctx).unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    match crate::agent::expand_requirements::run(task_id, &ctx.working_dir, client, model, &cancel)
        .await
    {
        Ok(doc_id) => {
            log_console!("[tool] expand_requirements → {}", doc_id);
            serde_json::json!({"ok": true, "document_id": doc_id}).to_string()
        }
        Err(e) => {
            log_console!("[tool] expand_requirements 失败: {}", e);
            serde_json::json!({"ok": false, "message": e}).to_string()
        }
    }
}

async fn tool_survey_codebase(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let task_description = args["task_description"].as_str().unwrap_or("");
    if task_description.is_empty() {
        return r#"{"ok": false, "message": "task_description 不能为空"}"#.to_string();
    }
    let client = match ctx.client.as_ref() {
        Some(c) => c,
        None => return serde_json::json!({"ok": false, "message": "API客户端不可用"}).to_string(),
    };
    let model = match ctx.model.as_ref() {
        Some(m) => m,
        None => return serde_json::json!({"ok": false, "message": "模型不可用"}).to_string(),
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
            log_console!("[tool] survey_codebase 失败: {}", e);
            serde_json::json!({"ok": false, "message": e}).to_string()
        }
    }
}

async fn tool_create_skill(args: &serde_json::Value, ctx: &ToolContext) -> String {
    let skill_name = args["name"].as_str().unwrap_or("");
    let description = args["description"].as_str().unwrap_or("");
    let content = args["content"].as_str().unwrap_or("");
    if skill_name.is_empty() || content.is_empty() {
        return r#"{"ok": false, "message": "name 和 content 不能为空"}"#.to_string();
    }
    if skill_name
        .chars()
        .any(|c| !c.is_alphanumeric() && c != '_' && c != '-')
    {
        return r#"{"ok": false, "message": "name 只能包含英文字母、数字、下划线和连字符"}"#
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
                "message": format!("技能 {} 已创建", skill_name),
                "skill_name": skill_name
            })
            .to_string()
        }
        Err(e) => {
            serde_json::json!({"ok": false, "message": format!("写入失败: {}", e)}).to_string()
        }
    }
}
