use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::agent::util::{extract_skill, strip_skill_tag};
use crate::api::client::{LlmClient, ToolDefinition};
use crate::models::role::Role;

pub struct MenxiaShizhongAgent {
    client: LlmClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl MenxiaShizhongAgent {
    pub fn new(client: LlmClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self {
            client,
            model: model.to_string(),
            cancel,
        }
    }

    fn tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::doc_inspect_tools();
        tools.extend(crate::tool::registry::document_tools());
        // route_tool 已移除 —— PipelineEngine 负责调度
        tools
    }

    pub fn load_skill(name: &str) -> &'static str {
        match name {
            "review_overall" => include_str!("skills/review_overall.md"),
            "review_phase" => include_str!("skills/review_phase.md"),
            _ => "",
        }
    }
}

#[async_trait::async_trait]
impl Agent for MenxiaShizhongAgent {
    fn role(&self) -> Role {
        Role::MenxiaShizhong
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
        // 根据任务描述自动选择并注入匹配的审核技能 markdown
        let task_lower = input.task_description.to_lowercase();
        let initial_skill = if task_lower.contains("overall")
            || task_lower.contains("整体")
            || task_lower.contains("全局")
            || task_lower.contains("architecture")
        {
            Some("review_overall")
        } else if task_lower.contains("phase")
            || task_lower.contains("阶段")
            || task_lower.contains("详细")
            || task_lower.contains("detail")
        {
            Some("review_phase")
        } else {
            None
        };

        let mut current_skill = String::new();

        if let Some(skill_name) = initial_skill {
            log_console!("[门下侍中] auto-injecting skill: {}", skill_name);
            session.inject_skill(skill_name, Self::load_skill(skill_name));
            current_skill = skill_name.to_string();
        }

        let (mut result, mut route);
        loop {
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
                            "[门下侍中] skill {} already loaded, prompting continue",
                            skill_name
                        );
                        session.inject(&format!("[系统] 技能 {} 已在当前会话中。请直接继续执行该技能的指令，不要重复输出 <skill> 标签。", skill_name));
                        continue;
                    }
                    current_skill = skill_name.clone();
                    log_console!("[门下侍中] replace skill: {}", skill_name);
                    session.inject_skill(skill_name, Self::load_skill(skill_name));
                    session.inject(&format!("[系统] 模式已切换为 {}。请立即按照该模式的指令执行审查，不要再输出 <skill> 标签。", skill_name));
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
