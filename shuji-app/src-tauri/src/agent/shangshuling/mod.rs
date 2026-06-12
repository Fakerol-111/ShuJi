use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::actor::ActorMessage;
use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};
use crate::api::client::{AnthropicClient, ToolDefinition};
use crate::models::message::Message;
use crate::models::role::Role;
use crate::workflow::WorkflowGraph;

pub struct ShangshulingAgent {
    client: AnthropicClient,
    model: String,
    cancel: Arc<AtomicBool>,
    peers: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    workflow_graph: Option<Arc<tokio::sync::Mutex<WorkflowGraph>>>,
    fast_txs: Option<crate::FastTxMap>,
}

impl ShangshulingAgent {
    pub fn new(
        client: AnthropicClient,
        model: &str,
        cancel: Arc<AtomicBool>,
        peers: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
        workflow_graph: Option<Arc<tokio::sync::Mutex<WorkflowGraph>>>,
        fast_txs: Option<crate::FastTxMap>,
    ) -> Self {
        Self {
            client,
            model: model.to_string(),
            cancel,
            peers,
            workflow_graph,
            fast_txs,
        }
    }

    fn tools() -> Vec<ToolDefinition> {
        let mut tools = crate::tool::registry::doc_inspect_tools();
        // read_file for reading source code
        tools.push(crate::tool::read_file_tool_def(
            "读取源文件和 .shuji/ 中的普通文件",
        ));
        tools.extend(crate::tool::registry::document_tools());
        tools.extend(crate::tool::registry::reauth_tool());
        // 调度工具：向六部分派任务并等待完成
        tools.push(crate::tool::registry::assign_task_tool());
        tools
    }
}

#[async_trait::async_trait]
impl Agent for ShangshulingAgent {
    fn role(&self) -> Role {
        Role::Shangshuling
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

        // 执行链已移除 —— PipelineEngine 负责调度

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

        let config = input.runtime_config.clone();
        let esaa_enabled = config.esaa.enabled;
        let esaa_full_log = config.esaa.full_intent_log;
        let checkers: std::sync::Arc<Vec<Box<dyn crate::api::intent::IntentChecker>>> =
            crate::api::intent::build_default_checkers(esaa_enabled, &working_dir);
        let dept = role_name.clone();
        let wd = working_dir.clone();
        let peers = self.peers.clone();
        let wf_graph = self.workflow_graph.clone();
        let fast_txs = self.fast_txs.clone();
        let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
            let name = name.to_owned();
            let args = args.clone();
            let wd = wd.clone();
            let checkers = checkers.clone();
            let dept = dept.clone();
            let peers = peers.clone();
            let wf_graph = wf_graph.clone();
            let fast_txs = fast_txs.clone();
            Box::pin(async move {
                let ctx = crate::tool::ToolContext {
                    working_dir: wd.clone(),
                    cancel_map: None,
                    client: None,
                    model: None,
                    fast_txs,
                    peers: Some(peers),
                    workflow_graph: wf_graph,
                };
                if let Some(result) =
                    crate::tool::tool_handle_shangshuling_special(&name, &args, &ctx).await
                {
                    return result;
                }
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
        Ok(AgentOutput::new(result))
    }
}
