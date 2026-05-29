use crate::config::RuntimeConfig;
use crate::models::role::Role;
use crate::models::message::Message;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone)]
pub struct AgentInput {
    pub role: Role,
    pub task_description: String,
    pub context_messages: Vec<Message>,
    pub project_dir: PathBuf,
    pub working_dir: PathBuf,
    /// Active skill system messages (内阁 only), injected between base_prompt
    /// and history. Stored as a vec to allow multiple active skills and future
    /// context compression.
    pub skill_prompts: Vec<String>,
    /// Current skill name from previous turn (内阁 only).
    /// Used by skill guard to prevent false-positive retries across execution rounds.
    pub current_skill: Option<String>,
    /// When true, the agent should resume a previously paused session
    /// (内阁 waiting for emperor decision) instead of building a new context.
    pub resume_paused: bool,
    /// Runtime configuration
    pub runtime_config: Arc<RuntimeConfig>,
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
}

impl AgentOutput {
    pub fn new(content: String) -> Self {
        Self {
            content,
            route: None,
            skill: None,
            paused: false,
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
    fn plan_display(&self) -> String { "null".to_string() }
}
