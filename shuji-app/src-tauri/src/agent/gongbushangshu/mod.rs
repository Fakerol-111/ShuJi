use std::path::Path;
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
}

impl GongbuShangshuAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self {
            client,
            model: model.to_string(),
            cancel,
            plan: Arc::new(Mutex::new(None)),
        }
    }

    fn tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::code_inspect_tools();
        tools.push(crate::tool::list_dir_tool_def());
        tools.extend(crate::tool::registry::file_write_tools_for_code());
        // Documents for report writing only
        tools.push(crate::tool::documents::create_document_tool_def());
        tools.push(crate::tool::documents::append_document_tool_def());
        tools.extend(crate::tool::registry::run_tests_tool());
        tools.push(crate::tool::registry::submit_plan_tool());
        tools.push(crate::tool::registry::complete_task_tool());
        tools.push(crate::tool::registry::route_tool());
        tools
    }

    async fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &Path) -> String {
        crate::tool::execute_named_tool(name, working_dir, args, "gongbushangshu").await
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
        .with_debug_dir(input.working_dir.clone());

        // Phase-based reasoning control
        let has_plan = self.plan.lock().unwrap().is_some();
        if has_plan {
            session.set_reasoning(false);
            log_console!("[工部] 执行阶段：关闭 reasoning，不限制 max_tokens");
        } else {
            session.set_reasoning(true);
            log_console!("[工部] 规划阶段：开启 reasoning，不限制 max_tokens");
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
                session.inject("所有批次任务已完成。请创建报告文档并路由回尚书令。");
            } else {
                if let Some(mut ctx) =
                    crate::api::session::PersistedContext::load_from(&working_dir, &role_name).await
                {
                    ctx.trim_tool_results(2000);
                    let mut msgs = ctx.to_messages();
                    if is_fresh {
                        if let (Some(ref name), Some(ref goal)) = (batch_name, batch_goal) {
                            msgs.push(serde_json::json!({"role": "user", "content": format!(
                                "当前批次（{}/{}）：{} — {}。请完成本批任务，完成后调用 complete_task。",
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

        let config = input.runtime_config.clone();
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
            Box::pin(async move {
                match name.as_str() {
                    "submit_plan" => {
                        {
                            let guard = plan_ref.lock().unwrap();
                            if guard.is_some() {
                                return r#"{"ok":false,"message":"计划已存在。请使用 complete_task 推进批次，不要重复提交计划。"}"#.to_string();
                            }
                        }
                        let batches: Vec<PlanBatch> = match args.get("batches") {
                            Some(b) => serde_json::from_value(b.clone()).unwrap_or_default(),
                            None => vec![],
                        };
                        if batches.is_empty() {
                            return r#"{"ok":false,"message":"batches 参数为空或格式错误"}"#
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
                        serde_json::json!({"ok":true,"message":format!("计划已提交：{} 个批次。请等待系统推进到第一个批次。", count)}).to_string()
                    }
                    "complete_task" => {
                        let mut guard = plan_ref.lock().unwrap();
                        match *guard {
                            Some(ref mut plan) => {
                                if plan.complete {
                                    return r#"{"ok":false,"message":"所有批次已完成，请写报告并路由"}"#.to_string();
                                }
                                let all_done = plan.advance();
                                if all_done {
                                    log_console!("[工部] complete_task: all batches done");
                                    r#"{"ok":true,"message":"所有批次已完成。请创建报告并路由回尚书令。"}"#.to_string()
                                } else {
                                    force_stop_clone.store(true, Ordering::SeqCst);
                                    log_console!("[工部] complete_task: batch {}/{} done — force-stopping",
                                        plan.current, plan.batches.len());
                                    serde_json::json!({"ok":true,"message":format!("第 {} 批完成，已推进到第 {} 批。请等待系统注入下一批次上下文。", plan.current, plan.current + 1)}).to_string()
                                }
                            }
                            None => r#"{"ok":false,"message":"没有活跃计划。如需分批，先调用 submit_plan。"}"#.to_string(),
                        }
                    }
                    _ => Self::execute_tool(&name, &args, &wd).await,
                }
            })
        };
        let (result, route) = controller
            .run(
                &mut session,
                &exec,
                &self.cancel,
                &tools,
                Some(&force_stop),
                &config,
                Some(&*input.fast_cancel),
            )
            .await?
            .into_tuple();

        // Persist for continuation within the batch
        let snap = session.snapshot();
        let mut ctx = crate::api::session::PersistedContext::from_messages(&snap.messages);
        ctx.trim_tool_results(2000);
        ctx.save_to(&working_dir, &role_name).await;

        let mut output = AgentOutput::new(result);
        output.route = route;
        Ok(output)
    }

    fn after_execute(&self, _output: &AgentOutput) -> LoopDecision {
        let plan_guard = self.plan.lock().unwrap();
        match *plan_guard {
            Some(ref p) if !p.complete => LoopDecision::Continue(
                "请继续完成当前批次任务。完成后调用 complete_task。".to_string(),
            ),
            _ => LoopDecision::Done,
        }
    }

    fn reset_plan(&self) {
        let mut guard = self.plan.lock().unwrap();
        *guard = None;
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
