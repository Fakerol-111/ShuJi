use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;

pub struct LibuRShangshuAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
}

impl LibuRShangshuAgent {
    pub fn new(client: AnthropicClient, model: &str, cancel: Arc<AtomicBool>) -> Self {
        Self {
            client,
            model: model.to_string(),
            cancel,
        }
    }

    fn tools() -> Vec<ToolDefinition> {
        let mut tools: Vec<ToolDefinition> = Vec::new();
        // 礼部: read-only inspection — files + documents
        tools.push(crate::tool::read_file_tool_def("读取源文件内容"));
        tools.push(crate::tool::documents::read_document_tool_def());
        // Write: only create + append for audit reports
        tools.push(crate::tool::documents::create_document_tool_def());
        tools.push(crate::tool::documents::append_document_tool_def());
        // Audit checklist
        tools.extend(crate::tool::registry::audit_checklist_tools());
        tools.push(crate::tool::lint_ops::run_lint_tool_def());
        // route_tool 已移除 —— PipelineEngine 负责调度
        tools
    }
}

#[async_trait::async_trait]
impl Agent for LibuRShangshuAgent {
    fn role(&self) -> Role {
        Role::LiBuRShangshu
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
