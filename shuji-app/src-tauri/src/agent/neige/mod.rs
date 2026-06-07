use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};


use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::agent::util::{extract_skill, strip_skill_tag};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;
use crate::workflow::{GateEngine, WorkflowConfig, WorkflowResolver, WorkflowState};

pub mod routing;

pub struct NeigeAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
    cancel_map: Option<crate::CancelMap>,
    fast_txs: Option<crate::FastTxMap>,
}

impl NeigeAgent {
    pub fn new(
        client: AnthropicClient,
        model: &str,
        cancel: Arc<AtomicBool>,
        cancel_map: Option<crate::CancelMap>,
        fast_txs: Option<crate::FastTxMap>,
    ) -> Self {
        Self {
            client,
            model: model.to_string(),
            cancel,
            cancel_map,
            fast_txs,
        }
    }

    fn tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::minimal_inspect_tools();
        // Documents: create + append only (no modify_document, no set_document_status)
        tools.push(crate::tool::documents::create_document_tool_def());
        tools.push(crate::tool::documents::append_document_tool_def());
        // Special tools
        tools.extend(crate::tool::registry::summarize_logs_tool());
        tools.push(crate::tool::registry::cancel_agent_tool());
        tools.push(crate::tool::registry::update_soul_tool());
        tools.push(crate::tool::registry::create_skill_tool());
        tools.push(crate::tool::registry::expand_requirements_tool());
        tools.push(crate::tool::registry::survey_codebase_tool());
        tools.push(crate::tool::registry::route_tool());
        tools
    }

    /// Read-only tool set for discuss mode — no document mutation, no routing.
    fn discuss_tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::minimal_inspect_tools();
        tools.extend(crate::tool::registry::summarize_logs_tool());
        tools
    }

    async fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &Path) -> String {
        crate::tool::execute_named_tool(name, working_dir, args, "neige").await
    }

    /// Read .shuji/state.json and inject project state + previous summary
    /// into the session as context for the summary skill.
    async fn inject_project_state(session: &mut crate::api::session::Session, working_dir: &Path) {
        let state_path = working_dir.join(".shuji").join("state.json");
        let content = match tokio::fs::read_to_string(&state_path).await {
            Ok(c) => c,
            Err(_) => return,
        };
        let project: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return,
        };

        let goal = project["goal"].as_str().unwrap_or("");
        let status = project["summary"].as_str().unwrap_or("");
        let task = project["task"].as_str().unwrap_or("");
        let prev_summary = project["summary_prompt"].as_str().unwrap_or("");

        let mut parts = vec![
            "[Project State]".to_string(),
            format!("Goal: {}", goal),
            format!("Status: {}", status),
        ];
        if !task.is_empty() {
            parts.push("Milestones:".to_string());
            for line in task.lines() {
                parts.push(format!("  {}", line));
            }
        }
        if !prev_summary.is_empty() {
            parts.push(String::new());
            parts.push("[Previous Summary]".to_string());
            parts.push(prev_summary.to_string());
        }

        session.inject(&parts.join("\n"));
    }

    /// Load soul from `.shuji/soul/neige.md`. If the file doesn't exist,
    /// bootstrap it from the compile-time default. This allows the soul to
    /// evolve at runtime (via `update_soul` tool or manual editing).
    /// Enforces ≤4000 chars to keep prompt injection size bounded.
    async fn load_soul(working_dir: &Path) -> String {
        let soul_dir = working_dir.join(".shuji").join("soul");
        let soul_path = soul_dir.join("neige.md");
        if let Ok(content) = tokio::fs::read_to_string(&soul_path).await {
            if !content.trim().is_empty() {
                if content.len() > 4000 {
                    log_console!(
                        "[soul] soul 长度 {} 超过 4000 上限，截断至 4000",
                        content.len()
                    );
                    let truncated: String = content.chars().take(4000).collect();
                    return truncated;
                }
                return content;
            }
        }
        // Bootstrap from compile-time default
        let default = include_str!("soul.md");
        let _ = tokio::fs::create_dir_all(&soul_dir).await;
        let _ = tokio::fs::write(&soul_path, default).await;
        default.to_string()
    }

    /// Load skill content. Checks `.shuji/skills/{name}.md` first (runtime-
    /// created skills), then falls back to compile-time embedded skills.
    /// Returns empty string if the skill is not found in either location.
    pub async fn load_skill(name: &str, working_dir: &Path) -> String {
        // 1. Check runtime skills on disk
        let disk_path = working_dir
            .join(".shuji")
            .join("skills")
            .join(format!("{}.md", name));
        if let Ok(content) = tokio::fs::read_to_string(&disk_path).await {
            if !content.trim().is_empty() {
                log_console!("[内阁] load skill from disk: {}", name);
                return content;
            }
        }
        // 2. Fall back to compiled-in skills
        let content: &str = match name {
            "discuss" => include_str!("skills/discuss.md"),
            "clarify" => include_str!("skills/clarify.md"),
            "workflow_demo" => include_str!("skills/workflow_demo.md"),
            "workflow_simple" => include_str!("skills/workflow_simple.md"),
            "workflow_standard" => include_str!("skills/workflow_standard.md"),
            "workflow_complex" => include_str!("skills/workflow_complex.md"),
            "workflow_optimize" => include_str!("skills/workflow_optimize.md"),
            "workflow_bugfix" => include_str!("skills/workflow_bugfix.md"),
            "workflow_refactor" => include_str!("skills/workflow_refactor.md"),
            "workflow_audit" => include_str!("skills/workflow_audit.md"),
            "summary" => include_str!("skills/summary.md"),
            "reflect" => include_str!("skills/reflect.md"),
            _ => "",
        };
        content.to_string()
    }

    /// Save raw session messages for pause/resume.
    /// These bypass PersistedContext compression so the full session
    /// context (including <options> decision points) is preserved.
    async fn save_paused_session(messages: &[serde_json::Value], working_dir: &std::path::Path) {
        let path = working_dir.join(".shuji").join("paused_session.json");
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Ok(json) = serde_json::to_string(messages) {
            let _ = tokio::fs::write(&path, &json).await;
            log_console!("[内阁] paused session saved ({} messages)", messages.len());
        }
    }

    /// Load and delete the paused session file.
    async fn load_paused_session(working_dir: &std::path::Path) -> Option<Vec<serde_json::Value>> {
        let path = working_dir.join(".shuji").join("paused_session.json");
        let content = tokio::fs::read_to_string(&path).await.ok()?;
        let messages: Vec<serde_json::Value> = serde_json::from_str(&content).ok()?;
        let _ = tokio::fs::remove_file(&path).await;
        log_console!("[内阁] paused session loaded ({} messages)", messages.len());
        Some(messages)
    }

    /// Read workflow preset from `.shuji/workflow_preset.json` and inject
    /// behavioral rules into the session.  Guides skill selection and
    /// workflow routing without hard-coding LLM behavior.
    async fn inject_workflow_preset(
        session: &mut crate::api::session::Session,
        working_dir: &Path,
    ) {
        let path = working_dir.join(".shuji").join("workflow_preset.json");
        let preset = match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str::<serde_json::Value>(&content)
                .ok()
                .and_then(|v| v["preset"].as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "standard".to_string()),
            Err(_) => "standard".to_string(),
        };

        let instruction = match preset.as_str() {
            "full" => {
                "\
[Workflow Preset: Full — 完整治理]
- 所有流程必经：需求展开 → 设计 → 门下审查 → 皇帝批复 → 尚书令执行 → 礼部规范检查
- 必须调用 expand_requirements（除非是极小 bugfix）
- skill 选择：workflow_standard / workflow_complex（根据复杂度）
- 门下侍中审查不可跳过"
            }

            "fast" => {
                "\
[Workflow Preset: Fast — 极速模式]
- 使用最轻量的流程：workflow_demo 或 workflow_simple
- 禁止使用 workflow_standard 和 workflow_complex
- expand_requirements: 已禁用，不得调用
- 跳过门下侍中审查和礼部规范检查
- 直接 route_to 尚书令执行"
            }
            "audit" => {
                "\
[Workflow Preset: Audit — 审计模式]
- 强制包含门下侍中审查和礼部规范检查
- 推荐 skill: workflow_audit
- 所有产出必须经门下侍中审查 + 礼部规范检查后才能交付"
            }

            _ => {
                "\
[Workflow Preset: Standard — 标准模式]
- 流程：需求展开 → 设计（中书令）→ 皇帝批复 → 执行（尚书令）
- 跳过门下侍中审查环节
- expand_requirements: 仅对 workflow_standard/complex 调用
- skill 选择：根据复杂度使用 workflow_simple / workflow_standard / workflow_complex"
            }
        };

        log_console!("[内阁] workflow preset: {}", preset);
        session.inject(instruction);
    }

    /// Clean up paused session file (used on Interrupt/Replace).
    pub async fn clear_paused_session(working_dir: &std::path::Path) {
        let path = working_dir.join(".shuji").join("paused_session.json");
        if path.exists() {
            let _ = tokio::fs::remove_file(&path).await;
            log_console!("[内阁] paused session cleared");
        }
    }
}

#[async_trait::async_trait]
impl Agent for NeigeAgent {
    fn role(&self) -> Role {
        Role::Neige
    }

    fn set_interrupt_flag(&mut self, flag: Arc<AtomicBool>) {
        self.cancel = flag;
    }

    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput> {
        let system_prompt = include_str!("prompt.md");
        let tools = if input.discuss_mode {
            Self::discuss_tools()
        } else {
            Self::tools()
        };
        let working_dir = input.working_dir.clone();

        let mut msgs = input.context_messages.clone();
        msgs.push(Message::user(&input.task_description));

        let client = Arc::new(self.client.clone());
        let model = self.model.clone();
        let mut session = crate::api::session::Session::new(
            system_prompt,
            &msgs,
            &self.model,
            &tools,
            &client,
            &input.runtime_config,
        )
        .with_role(self.role().name())
        .with_soul(
            self.role().name(),
            &Self::load_soul(&input.working_dir).await,
        )
        .with_debug_dir(input.working_dir.clone());

        // Inject workflow preset (discuss mode only — normal mode uses combined
        // resolver block below; resume mode restores session with preset already embedded)
        if input.discuss_mode {
            Self::inject_workflow_preset(&mut session, &working_dir).await;
        }

        // ── Discuss mode: force-inject discuss skill ──
        if input.discuss_mode {
            let discuss_skill = Self::load_skill("discuss", &working_dir).await;
            if !discuss_skill.is_empty() {
                session.inject_skill("discuss", &discuss_skill);
            }
        }

        // ── Resume from paused session (内阁 waiting for emperor) ──
        let resumed = if input.resume_paused {
            match Self::load_paused_session(&working_dir).await {
                Some(messages) => {
                    session.restore(&crate::api::session::SessionSnapshot::from_messages(
                        messages,
                    ));
                    session.inject(&format!("[皇帝回复] {}", input.task_description));
                    true
                }
                None => {
                    log_console!("[内阁] resume_paused=true but no paused session found, falling back to normal flow");
                    false
                }
            }
        } else {
            false
        };

        let thresholds = input.runtime_config.resolve_compact_thresholds(
            self.role().name(),
            input.context_window_config.get(self.role().name()),
        );

        // ── Normal restore from PersistedContext (skipped when resumed) ──
        let role_name = self.role().name().to_string();
        if !resumed {
            if let Some(mut ctx) =
                crate::api::session::PersistedContext::load_from(&working_dir, &role_name).await
            {
                log_console!(
                    "[内阁] loading context: base={} chars, recent={} msgs, skills={}",
                    ctx.base_prompt.len(),
                    ctx.context_messages.len(),
                    crate::api::session::count_skill_messages(&ctx.context_messages)
                );

                crate::api::compact::compact_and_save(
                    &self.client,
                    &self.model,
                    &mut ctx,
                    &thresholds,
                    true,
                    &working_dir,
                    &role_name,
                )
                .await;

                let mut msgs = ctx.to_messages();
                msgs.push(serde_json::json!({"role": "user", "content": format!("皇帝新指令：{}", input.task_description)}));
                let snap = crate::api::session::SessionSnapshot::from_messages(msgs);
                session.restore(&snap);
            }
        } // end if !resumed

        // ── Workflow Profile: resolve intent + inject skill/hints ──
        // Always resolves, even without workflow_config.json (defaults to auto+standard).
        let mut workflow_config = WorkflowConfig::load_from(&working_dir).await;
        let resolve_result =
            WorkflowResolver::resolve(&workflow_config, &working_dir, &input.task_description)
                .await;

        // ── Consume intent_override (one-shot) after resolution ──
        if workflow_config.intent_override.is_some() {
            workflow_config.intent_override = None;
            workflow_config.save_to(&working_dir).await.ok();
        }

        // ── Initialize workflow state (read by 尚书令 for chain injection) ──
        if !input.discuss_mode && !resumed {
            let wf_state = WorkflowState::new(
                &resolve_result.profile.profile_id,
                resolve_result.profile.governance.as_str(),
                &resolve_result.profile.execution_chain_id,
            );
            wf_state.save_to(&working_dir).await;
        }

        // ── Clarify re-evaluation: on resume, hint LLM to reconsider workflow ──
        if resumed && !input.discuss_mode {
            session.inject("[系统] 皇帝已回复澄清问题。请根据新信息重新评估工作流选择——如果任务类型与最初判断不同，用 <skill> 标签切换到合适的工作流。");
        }

        // ── Workflow preset + resolver hint (combined for cache efficiency) ──
        // DeepSeek prefix cache: contiguous system messages = longer stable prefix.
        // Always inject workflow preset first, then resolver hint/skill right after.
        if !input.discuss_mode && !resumed {
            Self::inject_workflow_preset(&mut session, &working_dir).await;

            let resolve_note = if resolve_result.locked {
                let skill_content =
                    Self::load_skill(&resolve_result.profile.cabinet_skill, &working_dir).await;
                if !skill_content.is_empty() {
                    log_console!(
                        "[workflow] locked profile {} — injecting skill {}",
                        resolve_result.profile.profile_id,
                        resolve_result.profile.cabinet_skill
                    );
                    session.inject_skill(&resolve_result.profile.cabinet_skill, &skill_content);
                }
                String::new()
            } else {
                resolve_result
                    .hint
                    .clone()
                    .unwrap_or_default()
            };
            if !resolve_note.is_empty() {
                session.inject(&resolve_note);
            }
        }

        let active_profile = Arc::new(resolve_result.profile);

        let mut controller = crate::api::control::AgentController::new();

        // ── Mid-run compaction ──
        {
            let client = self.client.clone();
            let model = self.model.clone();
            let wd = working_dir.clone();
            let role = role_name.clone();
            let cfg = input.runtime_config.clone();
            controller.set_compact_handler(
                Box::new(move |messages: Vec<serde_json::Value>| {
                    let client = client.clone();
                    let model = model.clone();
                    let wd = wd.clone();
                    let role = role.clone();
                    let cfg = cfg.clone();
                    Box::pin(async move {
                        // Re-read context config for live threshold updates
                        let ctx_roles = tokio::fs::read_to_string(wd.join("context_config.json"))
                            .await
                            .ok()
                            .and_then(|c| {
                                serde_json::from_str::<
                                        crate::commands::settings::ContextWindowConfig,
                                    >(&c)
                                    .ok()
                            })
                            .map(|c| c.roles)
                            .unwrap_or_default();
                        let thresholds =
                            cfg.resolve_compact_thresholds(&role, ctx_roles.get(&role));

                        let mut ctx =
                            crate::api::session::PersistedContext::from_messages(&messages);
                        crate::api::compact::compact_and_save(
                            &client,
                            &model,
                            &mut ctx,
                            &thresholds,
                            true,
                            &wd,
                            &role,
                        )
                        .await;
                    })
                }),
                40,
            );
        }

        // ── Periodic checkpoint ──
        let ckpt_wd = working_dir.clone();
        let ckpt_role = self.role().name().to_string();
        let ckpt_desc = input.task_description.clone();
        controller.set_checkpoint_handler(Box::new(move |snap| {
            let wd = ckpt_wd.clone();
            let role = ckpt_role.clone();
            let desc = ckpt_desc.clone();
            Box::pin(async move {
                crate::storage::checkpoint::save(&wd, &role, &desc, &snap).await;
            })
        }));

        let cancel_map = self.cancel_map.clone();
        let fast_txs: Option<crate::FastTxMap> =
            self.fast_txs.clone();
        let config = input.runtime_config.clone();
        let wd = working_dir.clone();

        // Shared skill state for the outer skill detection loop (not used for gates).
        let skill_state: Arc<Mutex<String>> =
            Arc::new(Mutex::new(input.current_skill.clone().unwrap_or_default()));

        let active_profile_for_exec = active_profile.clone();

        let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
            let name = name.to_owned();
            let args = args.clone();
            let ctx = crate::tool::ToolContext {
                working_dir: wd.clone(),
                cancel_map: cancel_map.clone(),
                client: Some(client.clone()),
                model: Some(model.clone()),
                fast_txs: fast_txs.clone(),
            };
            let profile = active_profile_for_exec.clone();
            Box::pin(async move {
                // ── GateEngine: check tool/route restrictions ──
                if let Err(violation) = GateEngine::check_tool(&profile, &name, &args) {
                    return serde_json::to_string(&violation.to_error_json())
                        .unwrap_or_else(|_| String::new());
                }
                if let Some(result) =
                    crate::tool::tool_handle_neige_special(&name, &args, &ctx).await
                {
                    result
                } else {
                    Self::execute_tool(&name, &args, &ctx.working_dir).await
                }
            })
        };

        let mut result;
        let mut route: Option<crate::api::control::RouteTo>;
        let mut current_skill = input.current_skill.clone().unwrap_or_default();
        *skill_state.lock().unwrap() = current_skill.clone();
        let mut must_approve_retries = 0u32;
        loop {
            // Suspension point D: check cancel between controller.run() rounds
            if self.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                log_console!("[内阁] interrupted in outer skill loop");
                result = String::new();
                route = None;
                break;
            }

            let run_result = controller
                .run(
                    &mut session,
                    &exec,
                    &self.cancel,
                    &tools,
                    None,
                    &config,
                    Some(&*input.fast_cancel),
                )
                .await?;
            (result, route) = match run_result {
                crate::api::control::RunResult::Done(text) => (text, None),
                crate::api::control::RunResult::Routed { text, route: r } => (text, Some(r)),
                crate::api::control::RunResult::Stopped(text) => (text, None),
            };

            if route.is_some() {
                break;
            }

            // ── Hard enforcement: must-approve doc pending but no <options> ──
            if !result.contains("<options>") {
                let pending_id =
                    crate::tool::documents::get_first_pending_approval(&working_dir).await;
                if let Some(ref id) = pending_id {
                    must_approve_retries += 1;
                    if must_approve_retries >= 3 {
                        log_console!("[内阁] must-approve doc {} 重试{must_approve_retries}次仍无<options>，自动批复", id);
                        let _ =
                            crate::tool::documents::remove_pending_approval(&working_dir, id).await;
                        let msg = format!(
                            "[系统] 文档 {} 已自动御批（内阁{}次重试仍未输出选项）。继续执行。",
                            id, must_approve_retries
                        );
                        session.inject(&msg);
                        continue;
                    }
                    log_console!("[内阁] must-approve doc {} pending (retry {must_approve_retries}/3), no <options> — re-prompting", id);
                    let msg = format!(
                        "[系统] 文档 {} 已创建但未经皇帝御批。请立即在回复中包含 <options> 标签，提供选项供皇帝决策。注意：route_to 和 <options> 不能在同一回合使用。请先输出 <options>，等待皇帝批复后再路由。",
                        id
                    );
                    session.inject(&msg);
                    continue;
                }
            }

            match extract_skill(&result) {
                Some(skill_name)
                    if !Self::load_skill(&skill_name, &working_dir).await.is_empty() =>
                {
                    if skill_name == current_skill {
                        log_console!(
                            "[内阁] skill {} already loaded, prompting continue",
                            skill_name
                        );
                        session.inject(&format!("[系统] 技能 {} 已在当前会话中。请直接继续执行该技能的指令，不要重复输出 <skill> 标签。", skill_name));
                        continue;
                    }
                    current_skill = skill_name.clone();
                    *skill_state.lock().unwrap() = current_skill.clone();
                    log_console!("[内阁] replace skill: {}", skill_name);
                    session.inject_skill(
                        &skill_name,
                        &Self::load_skill(&skill_name, &working_dir).await,
                    );
                    session.inject(&format!("[系统] 技能 {} 已加载。请立即按照该技能的指令行动，不要再输出 <skill> 标签。", skill_name));
                    if skill_name == "summary" {
                        Self::inject_project_state(&mut session, &working_dir).await;
                    }
                    continue;
                }
                _ => break,
            }
        }

        // Re-read participation level each turn so /level commands take effect immediately
        let level_prompt = match std::env::var("PARTICIPATION_LEVEL")
            .unwrap_or_else(|_| "1".to_string())
            .as_str()
        {
            "3" => include_str!("levels/level3.md"),
            "2" => include_str!("levels/level2.md"),
            _ => include_str!("levels/level1.md"),
        };
        session.inject(level_prompt);

        // ── Pause detection: save raw session when waiting for emperor ──
        let has_options = result.contains("<options>");

        if has_options && route.is_none() {
            // Save raw session (bypasses PersistedContext compression)
            let snap = session.snapshot();
            Self::save_paused_session(&snap.messages, &working_dir).await;
            log_console!("[内阁] <options> detected — session paused, awaiting emperor decision");
        } else {
            // Normal: save to PersistedContext
            let snap = session.snapshot();
            let ctx = crate::api::session::PersistedContext::from_messages(&snap.messages);
            ctx.save_to(&working_dir, &role_name).await;
        }

        let clean = strip_skill_tag(result);
        let mut output = AgentOutput::new(clean);
        output.paused = has_options && route.is_none();
        output.route = route;
        if !current_skill.is_empty() {
            output.skill = Some(current_skill);
        }
        Ok(output)
    }
}
