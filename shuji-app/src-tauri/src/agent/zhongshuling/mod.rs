use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::agent::util::{extract_skill, strip_skill_tag};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;

pub struct ZhongshulingAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl ZhongshulingAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self {
            client,
            model: model.to_string(),
            cancel,
        }
    }

    fn tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::inspect_tools();
        tools.extend(crate::tool::registry::document_tools());
        tools
    }

    async fn execute_tool(name: &str, args: &serde_json::Value, working_dir: &Path) -> String {
        crate::tool::execute_named_tool(name, working_dir, args, "zhongshuling").await
    }

    pub fn load_skill(name: &str) -> &'static str {
        match name {
            "overall_design" => include_str!("skills/overall_design.md"),
            "phase_plan" => include_str!("skills/phase_plan.md"),
            "phase_design" => include_str!("skills/phase_design.md"),
            "code_analysis" => include_str!("skills/code_analysis.md"),
            "optimization_plan" => include_str!("skills/optimization_plan.md"),
            "diagnosis" => include_str!("skills/diagnosis.md"),
            "impact_assessment" => include_str!("skills/impact_assessment.md"),
            _ => "",
        }
    }
}

#[async_trait::async_trait]
impl Agent for ZhongshulingAgent {
    fn role(&self) -> Role {
        Role::Zhongshuling
    }

    fn set_interrupt_flag(&mut self, flag: Arc<AtomicBool>) {
        self.cancel = flag;
    }

    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput> {
        let system_prompt = include_str!("prompt.md");
        let tools = Self::tools();
        let working_dir = input.working_dir.clone();

        let mut msgs = input.context_messages.clone();
        msgs.push(Message::user(&input.task_description));

        let client = Arc::new(self.client.clone());
        let mut session = crate::api::session::Session::new(
            system_prompt,
            &msgs,
            &self.model,
            &tools,
            &client,
            &input.skill_prompts,
            &input.runtime_config,
        )
        .with_role(self.role().name())
        .with_debug_dir(input.working_dir.clone());

        let role_name = self.role().name().to_string();

        let thresholds = input.runtime_config.resolve_compact_thresholds(
            self.role().name(),
            input.context_window_config.get(self.role().name()),
        );

        if let Some(mut ctx) =
            crate::api::session::PersistedContext::load_from(&working_dir, &role_name).await
        {
            // ── Context compaction (iterative: context → history → repeat) ──
            loop {
                let mut changed = false;

                if let Some(result) = crate::api::compact::maybe_compact_dept(
                    &self.client,
                    &self.model,
                    &ctx.history_messages,
                    &ctx.context_messages,
                    &thresholds,
                )
                .await
                {
                    ctx.history_messages = result.new_history;
                    ctx.context_messages = result.kept_context;
                    ctx.save_to(&working_dir, &role_name).await;
                    changed = true;
                }

                if let Some(merged) = crate::api::compact::maybe_compact_history(
                    &self.client,
                    &self.model,
                    &ctx.history_messages,
                    &thresholds,
                )
                .await
                {
                    ctx.history_messages = merged;
                    ctx.save_to(&working_dir, &role_name).await;
                    changed = true;
                }

                if !changed {
                    break;
                }
            }

            let mut msgs = ctx.to_messages();
            msgs.push(serde_json::json!({"role": "user", "content": input.task_description}));
            let snap = crate::api::session::SessionSnapshot::from_messages(msgs);
            session.restore(&snap);
        }

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
                        loop {
                            let mut changed = false;
                            if let Some(result) = crate::api::compact::maybe_compact_dept(
                                &client,
                                &model,
                                &ctx.history_messages,
                                &ctx.context_messages,
                                &thresholds,
                            )
                            .await
                            {
                                ctx.history_messages = result.new_history;
                                ctx.context_messages = result.kept_context;
                                changed = true;
                            }
                            if let Some(merged) = crate::api::compact::maybe_compact_history(
                                &client,
                                &model,
                                &ctx.history_messages,
                                &thresholds,
                            )
                            .await
                            {
                                ctx.history_messages = merged;
                                changed = true;
                            }
                            if !changed {
                                break;
                            }
                        }
                        ctx.save_to(&wd, &role).await;
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

        let config = input.runtime_config.clone();
        let wd = working_dir.clone();
        let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
            let name = name.to_owned();
            let args = args.clone();
            let wd = wd.clone();
            Box::pin(async move { Self::execute_tool(&name, &args, &wd).await })
        };

        let (mut result, mut route);
        let mut current_skill = String::new();
        loop {
            // Suspension point D: check cancel between controller.run() rounds
            if self.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                log_console!("[中书令] interrupted in outer skill loop");
                result = String::new();
                route = None;
                break;
            }

            (result, route) = controller
                .run(&mut session, &exec, &self.cancel, &tools, None, &config, Some(&*input.fast_cancel))
                .await?
                .into_tuple();

            if route.is_some() {
                break;
            }

            match extract_skill(&result) {
                Some(skill_name) if !Self::load_skill(&skill_name).is_empty() => {
                    if skill_name == current_skill {
                        log_console!(
                            "[中书令] skill {} already loaded, prompting continue",
                            skill_name
                        );
                        session.inject(&format!("[系统] 技能 {} 已在当前会话中。请直接继续执行该技能的指令，不要重复输出 <skill> 标签。", skill_name));
                        continue;
                    }
                    current_skill = skill_name.clone();
                    log_console!("[中书令] replace skill: {}", skill_name);
                    session.replace_skill(&skill_name, Self::load_skill(&skill_name));
                    session.inject(&format!("[系统] 模式已切换为 {}。请立即按照该模式的指令开始设计工作，不要再输出 <skill> 标签。", skill_name));
                    continue;
                }
                _ => break,
            }
        }

        let snap = session.snapshot();
        let ctx = crate::api::session::PersistedContext::from_messages(&snap.messages);
        ctx.save_to(&working_dir, self.role().name()).await;

        let clean = strip_skill_tag(result);
        let mut output = AgentOutput::new(clean);
        output.route = route;
        if !current_skill.is_empty() {
            output.skill = Some(current_skill);
        }
        Ok(output)
    }
}
