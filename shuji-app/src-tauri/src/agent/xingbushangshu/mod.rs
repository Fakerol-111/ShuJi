use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::api::client::{LlmClient, ToolDefinition};
use crate::models::role::Role;

pub struct XingbuShangshuAgent {
    client: LlmClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl XingbuShangshuAgent {
    pub fn new(client: LlmClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self {
            client,
            model: model.to_string(),
            cancel,
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
        tools.extend(crate::tool::registry::check_compile_tool());
        tools.push(crate::tool::test_env::setup_test_env_tool_def());
        // route_tool 已移除 —— PipelineEngine 负责调度
        tools
    }
}

#[async_trait::async_trait]
impl Agent for XingbuShangshuAgent {
    fn role(&self) -> Role {
        Role::XingbuShangshu
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

        // ── Explicit persisted context restoration ──
        // Unlike the generic load_and_compact_context, this path trims verbose
        // tool results and refreshes the soul — ensuring that interrupted test
        // runs can resume with the prior session intact.
        let has_persisted = working_dir
            .join(".shuji/context")
            .join(format!("{}.json", role_name))
            .exists();

        if has_persisted {
            if let Some(mut ctx) =
                crate::api::session::PersistedContext::load_from(&working_dir, &role_name).await
            {
                ctx.trim_tool_results(2000);
                let latest_soul =
                    crate::agent::runner::load_role_soul(&working_dir, &role_name).await;
                ctx = ctx.with_refreshed_soul(&role_name, &latest_soul);
                let mut msgs = ctx.to_messages();
                // Append current task description as new user message
                msgs.push(serde_json::json!({"role": "user", "content": input.task_description}));
                let snap = crate::api::session::SessionSnapshot::from_messages(msgs);
                session.restore(&snap);
                log_console!("[刑部] restored persisted context from disk");
            }
        } else {
            // First execution — use standard load_and_compact_context
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
        let (result, _route) = controller
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

        crate::agent::runner::save_context(&session, &working_dir, &role_name).await;

        // route_to 已移除 —— PipelineEngine 负责所有调度
        let mut output = AgentOutput::new(result);
        crate::agent::runner::attach_run_documents(&mut output, &mut controller, &working_dir)
            .await;
        Ok(output)
    }
}
