use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::agent::util::{extract_skill, strip_skill_tag};
use crate::api::client::{LlmClient, ToolDefinition};
use crate::models::role::Role;

pub struct ZhongshulingAgent {
    client: LlmClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl ZhongshulingAgent {
    pub fn new(client: LlmClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self {
            client,
            model: model.to_string(),
            cancel,
        }
    }

    fn tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::doc_inspect_tools();
        // read_file for reading source code and .shuji/ files (prompt references it)
        tools.push(crate::tool::read_file_tool_def(
            "read source files and .shuji/ regular files",
        ));
        tools.extend(crate::tool::registry::document_tools());
        // route_tool 已移除 —— PipelineEngine 负责调度
        tools
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

        let msgs = crate::agent::runner::build_initial_messages(input);

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

        // 技能门禁已移除 —— PipelineEngine 负责调度

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
        let wd = working_dir.clone();
        let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
            let name = name.to_owned();
            let args = args.clone();
            let wd = wd.clone();
            let checkers = checkers.clone();
            let dept = dept.clone();
            Box::pin(async move {
                crate::api::intent::check_and_execute(
                    &name,
                    &args,
                    &wd,
                    &dept,
                    &checkers,
                    esaa_full_log,
                )
                .await
            })
        };

        // ── 自动技能注入 ──
        // 根据任务描述自动选择并注入匹配的技能 markdown，取代 LLM 主动加载 <skill> 标签
        let task_lower = input.task_description.to_lowercase();
        let initial_skill = if task_lower.contains("architecture") || task_lower.contains("overall")
        {
            Some("overall_design")
        } else if task_lower.contains("phase")
            && (task_lower.contains("plan") || task_lower.contains("分阶段"))
        {
            Some("phase_plan")
        } else if task_lower.contains("detail")
            || task_lower.contains("详细设计")
            || task_lower.contains("详细")
        {
            Some("phase_design")
        } else if task_lower.contains("analysis")
            || task_lower.contains("分析")
            || task_lower.contains("代码分析")
        {
            Some("code_analysis")
        } else if task_lower.contains("diagnos")
            || task_lower.contains("诊断")
            || task_lower.contains("bug")
            || task_lower.contains("故障")
        {
            Some("diagnosis")
        } else if task_lower.contains("impact")
            || task_lower.contains("影响评估")
            || task_lower.contains("影响")
        {
            Some("impact_assessment")
        } else {
            None
        };

        let mut current_skill = String::new();

        if let Some(skill_name) = initial_skill {
            log_console!("[中书令] auto-injecting skill: {}", skill_name);
            session.inject_skill(skill_name, Self::load_skill(skill_name));
            current_skill = skill_name.to_string();
        }

        let (mut result, mut route);
        loop {
            if self.cancel.load(std::sync::atomic::Ordering::SeqCst) {
                log_console!("[中书令] interrupted in outer skill loop");
                result = String::new();
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
                        session.inject(&format!("[System] Skill {} is already loaded in the current session. Please continue executing this skill's instructions without repeating the <skill> tag.", skill_name));
                        continue;
                    }
                    current_skill = skill_name.clone();
                    log_console!("[中书令] replace skill: {}", skill_name);
                    session.inject_skill(skill_name, Self::load_skill(skill_name));
                    session.inject(&format!("[System] Mode switched to {}. Please immediately follow this mode's instructions for design work and do not output the <skill> tag again.", skill_name));
                    continue;
                }
                _ => break,
            }
        }

        crate::agent::runner::save_context(&session, &working_dir, self.role().name()).await;

        let clean = strip_skill_tag(result);
        let mut output = AgentOutput::new(clean);
        crate::agent::runner::attach_run_documents(&mut output, &mut controller, &working_dir)
            .await;
        Ok(output)
    }
}
