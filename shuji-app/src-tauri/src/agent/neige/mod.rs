use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub(crate) mod decision;
pub(crate) mod pipeline_plan;
pub(crate) mod skill;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::agent::util::strip_skill_tag;
use crate::api::client::{LlmClient, ToolDefinition};
use crate::models::role::Role;

pub struct NeigeAgent {
    client: LlmClient,
    model: String,
    cancel: Arc<AtomicBool>,
    cancel_map: Option<crate::CancelMap>,
    fast_txs: Option<crate::FastTxMap>,
}

impl NeigeAgent {
    pub fn new(
        client: LlmClient,
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
        // read_file for reading source code & .shuji/ files (read_document only finds docs by ID)
        tools.push(crate::tool::read_file_tool_def(
            "read source files and .shuji/ regular files",
        ));
        // Documents: create + append only (no modify_document, no set_document_status)
        tools.push(crate::tool::documents::create_task_document_tool_def());
        tools.push(crate::tool::documents::append_document_tool_def());
        // Special tools
        tools.extend(crate::tool::registry::summarize_logs_tool());
        tools.push(crate::tool::registry::cancel_agent_tool());
        tools.push(crate::tool::registry::update_soul_tool());
        tools.push(crate::tool::registry::create_skill_tool());
        tools.push(crate::tool::registry::expand_requirements_tool());
        tools.push(crate::tool::registry::survey_codebase_tool());
        tools.push(crate::tool::registry::submit_pipeline_plan_tool());
        tools.push(crate::tool::request_decision_tool_def());
        // route_tool 已移除 —— 内阁提交 Plan 给 PipelineEngine，不再手动 route_to
        tools
    }

    /// Read-only tool set for discuss mode — no document mutation, no routing.
    fn discuss_tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::minimal_inspect_tools();
        tools.extend(crate::tool::registry::summarize_logs_tool());
        tools
    }

    /// Read .shuji/state.json and inject project state + previous summary
    /// into the session as context for the summary skill.
    #[allow(dead_code)]
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

    /// Load soul via the shared learning store (project + optional global).
    async fn load_soul(working_dir: &Path) -> String {
        crate::agent::runner::load_role_soul(working_dir, "Neige").await
    }

    /// Load skill content from disk (delegates to skill module).
    pub async fn load_skill(name: &str, working_dir: &Path) -> String {
        skill::load_skill(name, working_dir).await
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
[Workflow Preset: Full — Full Governance]
- All processes must go through: requirements expansion → design → review by 门下侍中 → emperor approval → 尚书令 execution → 礼部 standards check
- Must call expand_requirements (unless it's a very small bugfix)
- skill selection: workflow_standard / workflow_complex (based on complexity)
- 门下侍中 review cannot be skipped"
            }

            "fast" => {
                "\
[Workflow Preset: Fast — Speed Mode]
- Use the lightest workflow: workflow_demo or workflow_simple
- workflow_standard and workflow_complex are forbidden
- expand_requirements: disabled, must not call
- Skip 门下侍中 review and 礼部 standards check
- submit_pipeline_plan with minimal dispatch_to steps via 尚书令 for execution"
            }
            "audit" => {
                "\
[Workflow Preset: Audit — Audit Mode]
- Mandatory: 门下侍中 review + 礼部 standards check
- Recommended skill: workflow_audit
- All output must pass 门下侍中 review + 礼部 standards check before delivery"
            }

            _ => {
                "\
[Workflow Preset: Standard — Standard Mode]
- Process: requirements expansion → design (中书令) → emperor approval → execution (尚书令)
- Skip 门下侍中 review step
- expand_requirements: only call for workflow_standard/complex
- skill selection: workflow_simple / workflow_standard / workflow_complex based on complexity"
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

        let msgs = crate::agent::runner::build_initial_messages(input);

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

        // ── Discuss mode: force-inject skill with fallback ──
        if input.discuss_mode {
            let mut skill_content = Self::load_skill("discuss", &working_dir).await;
            if skill_content.is_empty() {
                let fallback = if input.task_description.contains("bug")
                    || input.task_description.contains("修复")
                {
                    "workflow_bugfix"
                } else if input.task_description.contains("重构") {
                    "workflow_refactor"
                } else if input.task_description.contains("优化") {
                    "workflow_optimize"
                } else {
                    "clarify"
                };
                log_console!(
                    "[内阁] discuss skill not found, falling back to {}",
                    fallback
                );
                skill_content = Self::load_skill(fallback, &working_dir).await;
            }
            if !skill_content.is_empty() {
                session.inject_skill("discuss", &skill_content);
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
                    let latest_soul =
                        crate::agent::runner::load_role_soul(&working_dir, "Neige").await;
                    session.replace_soul("Neige", &latest_soul);
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

                let latest_soul =
                    crate::agent::runner::load_role_soul(&working_dir, &role_name).await;
                ctx = ctx.with_refreshed_soul(&role_name, &latest_soul);

                let mut msgs = ctx.to_messages();
                msgs.push(serde_json::json!({"role": "user", "content": format!("New instruction from Emperor: {}", input.task_description)}));
                let snap = crate::api::session::SessionSnapshot::from_messages(msgs);
                session.restore(&snap);
            }
        } // end if !resumed

        // ── Pipeline mode: 内阁不再使用 WorkflowResolver/skill 注入 ──
        // 内阁现在直接分析任务并产出 PipelinePlan 供 PipelineEngine 执行
        // discuss_mode 中保留了 inject_workflow_preset（在上面已执行）
        if !input.discuss_mode && !resumed {
            // 简明提示：不使用 workflow preset，内阁自主规划
            session.inject("[System] Please analyze the task scope and directly plan executable steps. Call submit_pipeline_plan to submit a machine-executable JSON plan. No <skill> tags are needed.");
        }
        if resumed && !input.discuss_mode {
            session.inject(
                "[System] 皇帝 has replied. Please continue planning and submit the pipeline plan.",
            );
        }

        let mut controller = crate::api::control::AgentController::new();

        // ── Mid-run compaction ──
        {
            let client = self.client.clone();
            let model = self.model.clone();
            let wd = working_dir.clone();
            let role = role_name.clone();
            let cfg = input.runtime_config.clone();
            // Capture context_config snapshot at registration time (same as
            // run_actor's per-task caching), avoiding disk reads on every compact.
            let ctx_roles = input.context_window_config.clone();
            controller.set_compact_handler(
                Box::new(move |messages: Vec<serde_json::Value>| {
                    let client = client.clone();
                    let model = model.clone();
                    let role = role.clone();
                    let cfg = cfg.clone();
                    let ctx_roles = ctx_roles.clone();
                    let wd = wd.clone();
                    Box::pin(async move {
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

        crate::api::control::setup_agent_step_emitter(
            &mut controller,
            &input.dept_step_tx,
            self.role().name(),
        );

        let cancel_map = self.cancel_map.clone();
        let fast_txs: Option<crate::FastTxMap> = self.fast_txs.clone();
        let config = input.runtime_config.clone();
        let esaa_enabled = config.esaa.enabled;
        let esaa_full_log = config.esaa.full_intent_log;
        let checkers: std::sync::Arc<Vec<Box<dyn crate::api::intent::IntentChecker>>> =
            crate::api::intent::build_default_checkers(esaa_enabled, &working_dir);
        let wd = working_dir.clone();

        // Shared skill state (保留用于 must-approve 循环，不再用于 skill 切换门控).
        let _skill_state: Arc<Mutex<String>> =
            Arc::new(Mutex::new(input.current_skill.clone().unwrap_or_default()));

        let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
            let name = name.to_owned();
            let args = args.clone();
            let ctx = crate::tool::ToolContext {
                working_dir: wd.clone(),
                cancel_map: cancel_map.clone(),
                client: Some(client.clone()),
                model: Some(model.clone()),
                fast_txs: fast_txs.clone(),
                peers: None,
                workflow_graph: None,
            };
            let checkers = checkers.clone();
            Box::pin(async move {
                if let Some(result) =
                    crate::tool::tool_handle_neige_special(&name, &args, &ctx).await
                {
                    result
                } else {
                    crate::api::intent::check_and_execute(
                        &name,
                        &args,
                        &ctx.working_dir,
                        "neige",
                        &checkers,
                        esaa_full_log,
                    )
                    .await
                }
            })
        };

        let before_len = session.messages().len();

        let (result, route, run_stopped) = if self.cancel.load(std::sync::atomic::Ordering::SeqCst)
        {
            log_console!("[内阁] interrupted");
            (String::new(), None, true)
        } else {
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
            let stopped = matches!(run_result, crate::api::control::RunResult::Stopped(_));
            match run_result {
                crate::api::control::RunResult::Done(text) => (text, None, stopped),
                crate::api::control::RunResult::Routed { text, route: r } => {
                    (text, Some(r), stopped)
                }
                crate::api::control::RunResult::Stopped(text) => (text, None, true),
            }
        };

        // ── Extract plan_json only from messages added this turn ──
        let plan_json = if input.allow_pipeline_plan && !run_stopped {
            extract_plan_json_from_messages(session.messages().iter().skip(before_len))
        } else {
            None
        };

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

        // ── Pause detection: pending revw awaiting emperor approval ──
        let has_pending = crate::tool::documents::get_first_pending_approval(&working_dir)
            .await
            .is_some();
        let should_pause = has_pending && route.is_none();

        if should_pause {
            let snap = session.snapshot();
            // Save raw session (bypasses PersistedContext compression)
            Self::save_paused_session(&snap.messages, &working_dir).await;
            log_console!("[内阁] paused awaiting emperor approval (pending={has_pending})");
        } else {
            // Normal: save to PersistedContext — borrow messages directly
            let ctx = crate::api::session::PersistedContext::from_messages(session.messages());
            ctx.save_to(&working_dir, &role_name).await;
        }

        let clean = strip_skill_tag(result);
        let mut output = AgentOutput::new(clean);
        output.paused = should_pause;
        output.route = route;
        output.plan_json = plan_json;
        output.decision_options = extract_decision_options(session.messages());
        crate::agent::runner::attach_run_documents(&mut output, &mut controller, &working_dir)
            .await;
        if should_pause {
            if let Some(pending_id) =
                crate::tool::documents::get_first_pending_approval(&working_dir).await
            {
                if let Some(doc) =
                    crate::tool::documents::chat_document_from_id(&working_dir, &pending_id).await
                {
                    if !output.documents.iter().any(|d| d.id == doc.id) {
                        output.documents.push(doc);
                    }
                }
            }
        }
        Ok(output)
    }
}

/// Extract plan_json from the most recent `submit_pipeline_plan` tool call.
pub(crate) fn extract_plan_json_from_messages<'a, I>(messages: I) -> Option<String>
where
    I: DoubleEndedIterator<Item = &'a serde_json::Value>,
{
    pipeline_plan::extract_plan_json_from_messages(messages)
}

fn extract_decision_options(messages: &[serde_json::Value]) -> Vec<String> {
    decision::extract_decision_options(messages)
}
