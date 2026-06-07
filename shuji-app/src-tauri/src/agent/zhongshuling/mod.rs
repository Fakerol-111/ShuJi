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
        let mut tools = crate::tool::registry::doc_inspect_tools();
        tools.extend(crate::tool::registry::document_tools());
        tools.push(crate::tool::registry::route_tool());
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

        let thresholds = input.runtime_config.resolve_compact_thresholds(
            self.role().name(),
            input.context_window_config.get(self.role().name()),
        );

        crate::agent::runner::load_and_compact_context(
            &self.client,
            &self.model,
            &working_dir,
            &role_name,
            &input.task_description,
            &mut session,
            &thresholds,
            false,
        )
        .await;

        // ── Skill Gate: profile-based skill restrictions ──
        if let Some(wf_state) = crate::workflow::WorkflowState::load_from(&working_dir).await {
            let mut hints = Vec::new();
            match wf_state.profile_id.as_str() {
                "brownfield_optimize" => {
                    hints.push("当前为存量优化模式（brownfield_optimize）");
                    hints.push("禁止使用 overall_design 技能（无需完整方案设计）");
                    hints.push("推荐使用 code_analysis 或 optimization_plan 技能");
                }
                "bugfix" | "demo" => {
                    hints.push("当前模式不需要方案设计");
                    hints.push("禁止使用 overall_design、phase_plan、phase_design 技能");
                    hints.push("如需分析可直接使用 diagnosis 或 impact_assessment");
                }
                _ => {}
            }
            if !hints.is_empty() {
                session.inject(&format!("[技能门禁]\n{}", hints.join("\n")));
            }
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
            if self.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                log_console!("[中书令] interrupted in outer skill loop");
                result = String::new();
                route = None;
                break;
            }

            (result, route) = controller
                .run(
                    &mut session,
                    &exec,
                    &self.cancel,
                    &tools,
                    None,
                    &config,
                    Some(&*input.fast_cancel),
                )
                .await?
                .into_tuple();

            if route.is_some() {
                break;
            }

            match extract_skill(&result) {
                Some(ref skill_name) if !Self::load_skill(skill_name).is_empty() => {
                    if skill_name == &current_skill {
                        log_console!(
                            "[中书令] skill {} already loaded, prompting continue",
                            skill_name
                        );
                        session.inject(&format!("[系统] 技能 {} 已在当前会话中。请直接继续执行该技能的指令，不要重复输出 <skill> 标签。", skill_name));
                        continue;
                    }
                    current_skill = skill_name.clone();
                    log_console!("[中书令] replace skill: {}", skill_name);
                    session.inject_skill(skill_name, Self::load_skill(skill_name));
                    session.inject(&format!("[系统] 模式已切换为 {}。请立即按照该模式的指令开始设计工作，不要再输出 <skill> 标签。", skill_name));
                    continue;
                }
                _ => break,
            }
        }

        crate::agent::runner::save_context(&session, &working_dir, self.role().name()).await;

        let clean = strip_skill_tag(result);
        let mut output = AgentOutput::new(clean);
        output.route = route;
        if !current_skill.is_empty() {
            output.skill = Some(current_skill);
        }
        Ok(output)
    }
}
