use std::collections::HashMap;

use crate::config::{RoleContextConfig, RuntimeConfig};
use crate::models::message::Message;
use crate::models::role::Role;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AgentInput {
    pub role: Role,
    pub task_description: String,
    pub context_messages: Vec<Message>,
    pub project_dir: PathBuf,
    pub working_dir: PathBuf,
    /// Current skill name from previous turn (内阁 only).
    /// Used by skill guard to prevent false-positive retries across execution rounds.
    pub current_skill: Option<String>,
    /// When true, the agent should resume a previously paused session
    /// (内阁 waiting for emperor decision) instead of building a new context.
    pub resume_paused: bool,
    /// Per-role context window overrides (from context_config.json)
    pub context_window_config: Arc<HashMap<String, RoleContextConfig>>,
    /// Runtime configuration
    pub runtime_config: Arc<RuntimeConfig>,
    /// When true, the agent should restrict itself to read-only tools
    /// (discuss mode — no document creation, routing, or file writes).
    pub discuss_mode: bool,
    /// Fast interrupt flag: set to true when the actor's fast mailbox
    /// receives an Interrupt signal. AgentController::run() checks this
    /// before each tool execution and between iterations.
    pub fast_cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct AgentOutput {
    pub content: String,
    pub route: Option<crate::api::control::RouteTo>,
    /// Current skill name, used for cross-turn persistence (内阁 only).
    pub skill: Option<String>,
    /// When true, the agent has presented <options> to the emperor and is
    /// waiting for a decision. The actor should pause the exec loop and
    /// resume on the next emperor message with resume_paused=true.
    pub paused: bool,
    /// NEW: when 内阁 calls submit_pipeline_plan, the JSON string is captured here.
    /// PipelineEngine consumes this field.
    pub plan_json: Option<String>,
}

impl AgentOutput {
    pub fn new(content: String) -> Self {
        Self {
            content,
            route: None,
            skill: None,
            paused: false,
            plan_json: None,
        }
    }

    pub fn with_route(mut self, route: crate::api::control::RouteTo) -> Self {
        self.route = Some(route);
        self
    }

    pub fn with_paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }
}

/// Returned by `after_execute`: whether the actor should continue the loop.
pub enum LoopDecision {
    /// Stop the loop, break out
    Done,
    /// Continue with a context message for the next round
    Continue(String),
}

#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    fn role(&self) -> Role;
    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput>;

    /// Called after each execute() round. Default: stop the loop.
    fn after_execute(&self, _output: &AgentOutput) -> LoopDecision {
        LoopDecision::Done
    }

    /// Override the agent's cancel flag with the per-actor flag
    /// from the actor system. Called during actor system startup
    /// so that Interrupt/cancel_agent reach AgentController.run().
    fn set_interrupt_flag(&mut self, _flag: Arc<AtomicBool>) {}

    /// Reset per-agent plan state (e.g. when a new task arrives).
    fn reset_plan(&self) {}

    /// Return plan display JSON for the frontend progress card.
    fn plan_display(&self) -> String {
        "null".to_string()
    }
}
