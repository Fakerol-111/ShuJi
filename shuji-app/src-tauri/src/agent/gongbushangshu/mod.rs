use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput, LoopDecision};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;

// ── Plan state ───────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanBatch {
    pub name: String,
    pub goal: String,
}

#[derive(Debug, Clone)]
pub struct PlanState {
    pub batches: Vec<PlanBatch>,
    pub current: usize,
    pub complete: bool,
    /// True when the batch just advanced — next execute() starts fresh.
    pub fresh_batch: bool,
}

impl PlanState {
    fn from_batches(batches: Vec<PlanBatch>) -> Self {
        Self {
            batches,
            current: 0,
            complete: false,
            fresh_batch: true,
        }
    }

    fn current_batch(&self) -> Option<&PlanBatch> {
        self.batches.get(self.current)
    }

    fn advance(&mut self) -> bool {
        self.current += 1;
        self.fresh_batch = true;
        if self.current >= self.batches.len() {
            self.complete = true;
            true
        } else {
            false
        }
    }
}

// ── Agent ────────────────────────────────────────────────────

pub struct GongbuShangshuAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
    plan: Arc<Mutex<Option<PlanState>>>,
    last_stopped: AtomicBool,
}

impl GongbuShangshuAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self {
            client,
            model: model.to_string(),
            cancel,
            plan: Arc::new(Mutex::new(None)),
            last_stopped: AtomicBool::new(false),
        }
    }

    fn tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::code_inspect_tools();
        // list_dir_tree is already included in code_inspect_tools
        tools.extend(crate::tool::registry::file_write_tools_for_code());
        // Documents for report writing only
        tools.push(crate::tool::documents::create_document_tool_def());
        tools.push(crate::tool::documents::append_document_tool_def());
        tools.extend(crate::tool::registry::run_tests_tool());
        tools.push(crate::tool::lint_ops::run_lint_tool_def());
        tools.push(crate::tool::registry::submit_batch_plan_tool());
        tools.push(crate::tool::registry::complete_task_tool());
        // route_tool 已移除 —— PipelineEngine 负责调度
        tools
    }
}

#[async_trait::async_trait]
impl Agent for GongbuShangshuAgent {
    fn role(&self) -> Role {
        Role::GongbuShangshu
    }

    fn set_interrupt_flag(&mut self, flag: Arc<AtomicBool>) {
        self.cancel = flag;
    }

    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput> {
        let system_prompt = include_str!("prompt.md");
        let tools = Self::tools();
        let working_dir = input.working_dir.clone();
        let role_name = self.role().name().to_string();

        let mut msgs = input.context_messages.clone();
        msgs.push(Message::user(&input.task_description));

        let client = Arc::new(self.client.clone());
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
            &crate::agent::runner::load_role_soul(&working_dir, &role_name).await,
        )
        .with_debug_dir(input.working_dir.clone());

        // Phase-based reasoning control
        let has_plan = self.plan.lock().unwrap().is_some();
        if has_plan {
            session.set_reasoning_phase(crate::config::ReasoningPhase::Execution);
        } else {
            session.set_reasoning_phase(crate::config::ReasoningPhase::Planning);
        }

        // Track and consume fresh_batch flag atomically
        let is_fresh = {
            let mut plan_guard = self.plan.lock().unwrap();
            if let Some(ref mut plan) = *plan_guard {
                if plan.fresh_batch {
                    plan.fresh_batch = false;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        // Extract plan info before any async operations
        let (plan_complete, plan_current, plan_total, batch_name, batch_goal) = {
            let plan_guard = self.plan.lock().unwrap();
            match *plan_guard {
                Some(ref p) => (
                    p.complete,
                    p.current,
                    p.batches.len(),
                    p.current_batch().map(|b| b.name.clone()),
                    p.current_batch().map(|b| b.goal.clone()),
                ),
                None => (true, 0, 0, None, None),
            }
        };

        if has_plan {
            if plan_complete {
                session.inject("All batches completed. Please create a report document and route back to 尚书令.");
            } else {
                if let Some(mut ctx) =
                    crate::api::session::PersistedContext::load_from(&working_dir, &role_name).await
                {
                    ctx.trim_tool_results(2000);
                    let latest_soul =
                        crate::agent::runner::load_role_soul(&working_dir, &role_name).await;
                    ctx = ctx.with_refreshed_soul(&role_name, &latest_soul);
                    let mut msgs = ctx.to_messages();
                    if is_fresh {
                        if let (Some(ref name), Some(ref goal)) = (batch_name, batch_goal) {
                            msgs.push(serde_json::json!({"role": "user", "content": format!(
                                "Current batch ({}/{}): {} — {}. Complete this batch, then call complete_task.",
                                plan_current + 1, plan_total, name, goal,
                            )}));
                            log_console!(
                                "[工部] batch {}/{} started, appended instruction only",
                                plan_current + 1,
                                plan_total
                            );
                        }
                    }
                    msgs.push(
                        serde_json::json!({"role": "user", "content": input.task_description}),
                    );
                    let snap = crate::api::session::SessionSnapshot::from_messages(msgs);
                    session.restore(&snap);
                }
            }
        } else {
            // No plan yet: remove stale context
            let ctx_path = working_dir
                .join(".shuji/context")
                .join(format!("{}.json", role_name));
            let _ = tokio::fs::remove_file(&ctx_path).await;
        }

        let mut controller = crate::api::control::AgentController::new();

        let (compact_fn, compact_interval) = crate::agent::runner::build_compact_handler(
            self.client.clone(),
            self.model.clone(),
            working_dir.clone(),
            role_name.clone(),
            input.runtime_config.clone(),
            false,
            input.context_window_config.clone(),
        );
        controller.set_compact_handler(compact_fn, compact_interval);

        controller.set_checkpoint_handler(crate::agent::runner::build_checkpoint_handler(
            working_dir.clone(),
            role_name.clone(),
            input.task_description.clone(),
        ));

        crate::api::control::setup_agent_step_emitter(
            &mut controller,
            &input.dept_step_tx,
            self.role().name(),
        );

        let config = input.runtime_config.clone();
        let esaa_enabled = config.esaa.enabled;
        let esaa_full_log = config.esaa.full_intent_log;
        let checkers: std::sync::Arc<Vec<Box<dyn crate::api::intent::IntentChecker>>> =
            crate::api::intent::build_default_checkers(esaa_enabled, &working_dir);
        let dept = role_name.clone();
        let plan_ref = self.plan.clone();
        let wd = working_dir.clone();
        let force_stop = Arc::new(AtomicBool::new(false));
        let force_stop_clone = force_stop.clone();
        let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
            let name = name.to_owned();
            let args = args.clone();
            let plan_ref = plan_ref.clone();
            let force_stop_clone = force_stop_clone.clone();
            let wd = wd.clone();
            let checkers = checkers.clone();
            let dept = dept.clone();
            Box::pin(async move {
                match name.as_str() {
                    "submit_plan" => {
                        {
                            let guard = plan_ref.lock().unwrap();
                            if guard.is_some() {
                                return r#"{"ok":false,"message":"A plan already exists. Use complete_task to advance batches, do not resubmit a plan."}"#.to_string();
                            }
                        }
                        let batches: Vec<PlanBatch> = match args.get("batches") {
                            Some(b) => serde_json::from_value(b.clone()).unwrap_or_default(),
                            None => vec![],
                        };
                        if batches.is_empty() {
                            return r#"{"ok":false,"message":"batches parameter is empty or malformed"}"#
                                .to_string();
                        }
                        let count = batches.len();
                        let mut guard = plan_ref.lock().unwrap();
                        *guard = Some(PlanState::from_batches(batches));
                        force_stop_clone.store(true, Ordering::SeqCst);
                        log_console!(
                            "[工部] submit_plan: {} batches — force-stopping controller",
                            count
                        );
                        serde_json::json!({"ok":true,"message":format!("Plan submitted: {} batches. Waiting for system to advance to the first batch.", count)}).to_string()
                    }
                    "complete_task" => {
                        let mut guard = plan_ref.lock().unwrap();
                        match *guard {
                            Some(ref mut plan) => {
                                if plan.complete {
                                    return r#"{"ok":false,"message":"All batches completed. Write a report and route."}"#.to_string();
                                }
                                let all_done = plan.advance();
                                if all_done {
                                    force_stop_clone.store(true, Ordering::SeqCst);
                                    log_console!("[工部] complete_task: all batches done");
                                    r#"{"ok":true,"message":"All batches completed. Create a report and route back to 尚书令."}"#.to_string()
                                } else {
                                    force_stop_clone.store(true, Ordering::SeqCst);
                                    log_console!("[工部] complete_task: batch {}/{} done — force-stopping",
                                        plan.current, plan.batches.len());
                                    serde_json::json!({"ok":true,"message":format!("Batch {} completed, advanced to batch {}. Waiting for system to inject next batch context.", plan.current, plan.current + 1)}).to_string()
                                }
                            }
                            None => r#"{"ok":false,"message":"No active plan. To batch, call submit_plan first."}"#.to_string(),
                        }
                    }
                    _ => {
                        crate::api::intent::check_and_execute(
                            &name,
                            &args,
                            &wd,
                            &dept,
                            &checkers,
                            esaa_full_log,
                        )
                        .await
                    }
                }
            })
        };
        let run_result = controller
            .run(
                &mut session,
                &exec,
                &self.cancel,
                &tools,
                Some(&force_stop),
                &config,
                Some(&*input.fast_cancel),
            )
            .await?;

        let stopped = matches!(run_result, crate::api::control::RunResult::Stopped(_));
        if stopped {
            self.last_stopped.store(true, Ordering::SeqCst);
        }
        let result = run_result.into_text();

        // Persist for continuation within the batch
        let snap = session.snapshot();
        let mut ctx = crate::api::session::PersistedContext::from_messages(&snap.messages);
        ctx.trim_tool_results(2000);
        ctx.save_to(&working_dir, &role_name).await;

        // route_to 已移除 —— PipelineEngine 负责所有调度
        let mut output = AgentOutput::new(result);
        crate::agent::runner::attach_run_documents(&mut output, &mut controller, &working_dir)
            .await;
        Ok(output)
    }

    fn after_execute(&self, _output: &AgentOutput) -> LoopDecision {
        if self.last_stopped.swap(false, Ordering::SeqCst) {
            return LoopDecision::Done;
        }
        let plan_guard = self.plan.lock().unwrap();
        match *plan_guard {
            Some(ref p) if !p.complete => LoopDecision::Continue(
                "Please continue with the current batch task. Call complete_task when done."
                    .to_string(),
            ),
            _ => LoopDecision::Done,
        }
    }

    fn reset_plan(&self) {
        let mut guard = self.plan.lock().unwrap();
        *guard = None;
        self.last_stopped.store(false, Ordering::SeqCst);
        log_console!("[工部] plan cleared for new task");
    }

    fn plan_display(&self) -> String {
        let guard = self.plan.lock().unwrap();
        match *guard {
            Some(ref plan) => {
                let batches: Vec<serde_json::Value> = plan
                    .batches
                    .iter()
                    .enumerate()
                    .map(|(i, b)| {
                        let status = if i < plan.current {
                            "done"
                        } else if i == plan.current && !plan.complete {
                            "current"
                        } else {
                            "pending"
                        };
                        serde_json::json!({"name": b.name, "goal": b.goal, "status": status})
                    })
                    .collect();
                serde_json::json!({"batches": batches, "current": plan.current, "complete": plan.complete}).to_string()
            }
            None => "null".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::r#trait::AgentOutput;

    #[test]
    fn gongbu_stopped_does_not_continue_batch() {
        let cancel = Arc::new(AtomicBool::new(false));
        let client = AnthropicClient::new(String::new(), String::new());
        let agent = GongbuShangshuAgent::new(client, "test", cancel);

        {
            let mut guard = agent.plan.lock().unwrap();
            *guard = Some(PlanState {
                batches: vec![PlanBatch {
                    name: "b1".into(),
                    goal: "g1".into(),
                }],
                current: 0,
                complete: false,
                fresh_batch: false,
            });
        }
        agent.last_stopped.store(true, Ordering::SeqCst);

        let decision = agent.after_execute(&AgentOutput::new(String::new()));
        assert!(matches!(decision, LoopDecision::Done));
    }
}
