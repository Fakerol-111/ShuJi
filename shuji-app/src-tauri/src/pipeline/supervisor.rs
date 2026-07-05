//! Pipeline supervisor — runs PipelineEngine in a background task so the
//! 内阁 actor mailbox stays free for cancel / new orders.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;

use crate::actor::{ActorMessage, ActorSystem};
use crate::api::control::RouteMsgType;
use crate::config::RuntimeConfig;
use crate::models::chat::ChatMessage;
use crate::models::role::Role;

use super::engine::{PipelineEngine, PipelineEngineContext};
use super::{PipelinePlan, PipelineResult, PlanRuntime};

/// Shared context for pipeline status reporting (chat + optional neige wake).
#[derive(Clone)]
pub struct PipelineNotifyContext {
    pub project_dir: PathBuf,
    pub working_dir: PathBuf,
    pub runtime_config: Arc<RuntimeConfig>,
    pub emperor_tx: tokio::sync::mpsc::Sender<ChatMessage>,
    pub talk_history: Arc<Mutex<Vec<String>>>,
}

/// Holds the current background pipeline task and a running flag.
pub struct PipelineSupervisor {
    running: Arc<AtomicBool>,
    task: Arc<AsyncMutex<Option<JoinHandle<()>>>>,
    /// plan_id currently executing (if any)
    running_plan_id: Arc<Mutex<Option<String>>>,
    /// last completed plan_id — blocks duplicate restart until user sends new message
    last_completed_plan_id: Arc<Mutex<Option<String>>>,
    /// timestamp of last abort — short latch blocks tail-restart after cancel
    last_aborted_at: Arc<Mutex<Option<Instant>>>,
}

/// How long after cancel to reject plan submissions from stale LLM tail responses.
const CANCEL_LATCH_SECS: u64 = 3;

struct PipelineSpawnContext {
    notify: PipelineNotifyContext,
    neige_tx: Option<tokio::sync::mpsc::UnboundedSender<ActorMessage>>,
    plan_id: String,
    running_plan_id: Arc<Mutex<Option<String>>>,
    last_completed: Arc<Mutex<Option<String>>>,
}

impl PipelineSupervisor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            task: Arc::new(AsyncMutex::new(None)),
            running_plan_id: Arc::new(Mutex::new(None)),
            last_completed_plan_id: Arc::new(Mutex::new(None)),
            last_aborted_at: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Clear plan dedup / cancel guards when the user explicitly sends a new message.
    pub fn clear_submission_guards(&self) {
        if let Ok(mut guard) = self.last_completed_plan_id.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.last_aborted_at.lock() {
            *guard = None;
        }
    }

    fn should_reject_plan(&self, plan_id: &str) -> Option<&'static str> {
        if let Ok(guard) = self.running_plan_id.lock() {
            if guard.as_deref() == Some(plan_id) && self.is_running() {
                return Some("already running");
            }
        }
        if let Ok(guard) = self.last_completed_plan_id.lock() {
            if guard.as_deref() == Some(plan_id) {
                return Some("duplicate completed");
            }
        }
        if let Ok(guard) = self.last_aborted_at.lock() {
            if let Some(at) = *guard {
                if at.elapsed() < Duration::from_secs(CANCEL_LATCH_SECS) {
                    return Some("cancel latch");
                }
            }
        }
        None
    }

    fn mark_plan_started(&self, plan_id: &str) {
        if let Ok(mut guard) = self.running_plan_id.lock() {
            *guard = Some(plan_id.to_string());
        }
    }

    #[cfg(test)]
    pub fn test_set_running(&self, value: bool) {
        self.running.store(value, Ordering::SeqCst);
    }

    #[cfg(test)]
    fn test_mark_completed(&self, plan_id: &str) {
        if let Ok(mut guard) = self.last_completed_plan_id.lock() {
            *guard = Some(plan_id.to_string());
        }
    }

    #[cfg(test)]
    fn test_mark_aborted(&self) {
        if let Ok(mut guard) = self.last_aborted_at.lock() {
            *guard = Some(Instant::now());
        }
    }

    #[cfg(test)]
    fn test_should_reject_plan(&self, plan_id: &str) -> Option<&'static str> {
        self.should_reject_plan(plan_id)
    }

    /// Abort any in-flight pipeline (sets global cancel on ActorSystem).
    pub async fn abort_current(&self, system: &ActorSystem) {
        system.cancel.store(true, Ordering::SeqCst);
        for flag in system.cancel_map.values() {
            flag.store(true, Ordering::SeqCst);
        }
        for tx in system.fast_txs.values() {
            let _ = tx.try_send(crate::actor::FastMessage::Interrupt);
        }
        if let Some(handle) = self.task.lock().await.take() {
            handle.abort();
        }
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.running_plan_id.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.last_aborted_at.lock() {
            *guard = Some(Instant::now());
        }
        // Do not reset system.cancel here — let cancel_processing finish its sweep first.
    }

    /// Start a freshly submitted plan (non-blocking).
    pub async fn start_plan(
        &self,
        plan: PipelinePlan,
        system: &ActorSystem,
        notify: PipelineNotifyContext,
    ) {
        let plan_id = plan.plan_id.clone();
        if let Some(reason) = self.should_reject_plan(&plan_id) {
            log_console!("[pipeline-supervisor] ignored {} plan: {}", reason, plan_id);
            return;
        }

        if self.is_running() {
            log_console!("[pipeline-supervisor] aborting previous run for new plan");
            self.abort_current(system).await;
        }

        self.mark_plan_started(&plan_id);

        system.cancel.store(false, Ordering::SeqCst);
        for flag in system.cancel_map.values() {
            flag.store(false, Ordering::SeqCst);
        }

        let context = PipelineEngineContext::from_actor_system(
            system,
            notify.project_dir.clone(),
            notify.runtime_config.clone(),
        );
        let mut engine = PipelineEngine::new(plan, context);
        engine.save().await.ok();
        engine.preview_pipeline_on_graph().await;

        let plan_msg = format!(
            "Pipeline plan submitted: {} ({} steps)",
            engine.runtime.plan.summary,
            engine.runtime.plan.steps.len(),
        );
        let _ = notify
            .emperor_tx
            .try_send(ChatMessage::new("内阁", &plan_msg));

        let plan_id_for_finish = plan_id.clone();
        let running_plan_id = self.running_plan_id.clone();
        let last_completed = self.last_completed_plan_id.clone();
        self.spawn_run(
            engine,
            system,
            notify,
            plan_id_for_finish,
            running_plan_id,
            last_completed,
        )
        .await;
    }

    /// Resume a paused runtime from user input (non-blocking).
    pub async fn resume_with_input(
        &self,
        project_dir: &Path,
        system: &ActorSystem,
        notify: PipelineNotifyContext,
        user_input: Option<&str>,
    ) -> Result<String, String> {
        if self.is_running() {
            return Err(
                "Pipeline is still running; wait for it to pause or use cancel first.".into(),
            );
        }

        let Some(engine) =
            PipelineEngine::load_from_disk(project_dir, system, notify.runtime_config.clone())
                .await
        else {
            return Err("pipeline runtime not found on disk".into());
        };

        let input_owned = user_input.map(|s| s.to_string());
        self.spawn_resume(engine, system, notify, input_owned).await;
        Ok("pipeline resume started".into())
    }

    async fn spawn_run(
        &self,
        mut engine: PipelineEngine,
        system: &ActorSystem,
        notify: PipelineNotifyContext,
        plan_id: String,
        running_plan_id: Arc<Mutex<Option<String>>>,
        last_completed: Arc<Mutex<Option<String>>>,
    ) {
        let neige_tx = system.senders.get(&Role::Neige).cloned();
        let context = PipelineSpawnContext {
            notify,
            neige_tx,
            plan_id,
            running_plan_id,
            last_completed,
        };
        self.spawn_task(system, context, async move { engine.run().await })
            .await;
    }

    async fn spawn_resume(
        &self,
        engine: PipelineEngine,
        system: &ActorSystem,
        notify: PipelineNotifyContext,
        user_input: Option<String>,
    ) {
        let neige_tx = system.senders.get(&Role::Neige).cloned();
        let plan_id = engine.runtime.plan.plan_id.clone();
        let running_plan_id = self.running_plan_id.clone();
        let last_completed = self.last_completed_plan_id.clone();
        let context = PipelineSpawnContext {
            notify,
            neige_tx,
            plan_id,
            running_plan_id,
            last_completed,
        };
        self.spawn_task(system, context, async move {
            engine.resume_with_input(user_input.as_deref()).await
        })
        .await;
    }

    async fn spawn_task<F>(&self, _system: &ActorSystem, context: PipelineSpawnContext, run: F)
    where
        F: std::future::Future<Output = PipelineResult> + Send + 'static,
    {
        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let PipelineSpawnContext {
            notify,
            neige_tx,
            plan_id,
            running_plan_id,
            last_completed,
        } = context;
        let project_dir = notify.project_dir.clone();

        let handle = tokio::spawn(async move {
            let result = run.await;
            let completed = matches!(result, PipelineResult::Complete { .. });
            report_pipeline_result(&result, &notify, neige_tx.as_ref()).await;
            if matches!(result, PipelineResult::Aborted { .. }) {
                PlanRuntime::cleanup(&project_dir).await;
            }
            if let Ok(mut guard) = running_plan_id.lock() {
                *guard = None;
            }
            if completed {
                if let Ok(mut guard) = last_completed.lock() {
                    *guard = Some(plan_id);
                }
            }
            running.store(false, Ordering::SeqCst);
            // 管道到达终态时停止前端计时器（排除暂停等待审批/用户输入的情况）
            if matches!(
                result,
                PipelineResult::Complete { .. }
                    | PipelineResult::Aborted { .. }
                    | PipelineResult::Deadlock { .. }
                    | PipelineResult::StepFailed { .. }
            ) {
                crate::round_metrics::reset_round();
            }
        });

        *self.task.lock().await = Some(handle);
    }
}

impl Default for PipelineSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

/// Emit pipeline outcome to chat; wake 内阁 for summary / failures when needed.
pub async fn report_pipeline_result(
    result: &PipelineResult,
    notify: &PipelineNotifyContext,
    neige_tx: Option<&tokio::sync::mpsc::UnboundedSender<ActorMessage>>,
) {
    match result {
        PipelineResult::Complete { runtime } => {
            let _ = notify.emperor_tx.try_send(ChatMessage::new(
                "内阁",
                &format!(
                    "Pipeline plan \"{}\" fully executed, generating summary...",
                    runtime.plan.summary
                ),
            ));
            PlanRuntime::cleanup(&notify.project_dir).await;

            if let Err(e) =
                crate::learning::LearningExtractor::from_pipeline_complete(&notify.project_dir)
                    .await
            {
                log_console!("[learning] pipeline extract skipped: {}", e);
            }

            if let Some(tx) = neige_tx {
                let summary_task = format!(
                    "Pipeline plan \"{}\" has been fully executed. Please review documents and reports produced by all departments, and present a complete task summary to the Emperor, explaining what was accomplished and what output was produced.",
                    runtime.plan.summary
                );
                let _ = tx.send(ActorMessage::pipeline_summary(summary_task));
            }
        }

        PipelineResult::AwaitingUserInput {
            step_id, question, ..
        } => {
            let _ = notify.emperor_tx.try_send(ChatMessage::new(
                "内阁",
                &format!(
                    "Pipeline waiting for user input (step {}):\n{}",
                    step_id, question
                ),
            ));
        }

        PipelineResult::AwaitingApproval {
            doc_id,
            step_id,
            runtime,
            ..
        } => {
            let step_desc = runtime
                .plan
                .steps
                .iter()
                .find(|s| s.step_id == *step_id)
                .map(|s| s.description.as_str())
                .unwrap_or(step_id.as_str());
            let content = format!(
                "【等待朱批】流程已暂停。\n\n\
                 待批文档：{doc_id}\n\
                 当前步骤：{step_id}（{step_desc}）\n\n\
                 请查阅门下侍中的审查报告，点击「准奏」后 pipeline 将自动继续。\n\
                 若不满意审查结果，请叫停诸司、恢复检查点，然后重新下诏。"
            );
            let mut msg = ChatMessage::new("系统", &content);
            if let Some(doc) =
                crate::tool::documents::chat_document_from_id(&notify.working_dir, doc_id).await
            {
                msg.documents = vec![doc];
            }
            let _ = notify.emperor_tx.try_send(msg);
        }

        PipelineResult::StepFailed {
            step_id, reason, ..
        } => {
            let _ = notify.emperor_tx.try_send(ChatMessage::new(
                "内阁",
                &format!("Pipeline step {} failed: {}", step_id, reason),
            ));
            if let Some(tx) = neige_tx {
                let wake = format!(
                    "Pipeline step {} failed: {}. Please analyze the failure and advise the Emperor.",
                    step_id, reason
                );
                let _ = tx.send(ActorMessage::new(wake, RouteMsgType::Task));
            }
        }

        PipelineResult::Aborted { .. } => {
            let _ = notify
                .emperor_tx
                .try_send(ChatMessage::new("内阁", "Pipeline execution aborted"));
        }

        PipelineResult::Deadlock { .. } => {
            let _ = notify.emperor_tx.try_send(ChatMessage::new(
                "内阁",
                "Pipeline deadlock: remaining steps have unmet dependencies. Please review the plan.",
            ));
        }
    }

    let status_msg = format_pipeline_status(result);
    let _ = notify
        .emperor_tx
        .try_send(ChatMessage::new("System", &status_msg));
    log_console!("[pipeline-supervisor] {}", status_msg);
}

fn format_pipeline_status(result: &PipelineResult) -> String {
    match result {
        PipelineResult::Complete { runtime } => {
            format!("✅ Pipeline execution complete: {}", runtime.plan.summary)
        }
        PipelineResult::AwaitingUserInput {
            step_id, question, ..
        } => format!(
            "⏳ Pipeline waiting for user input (step {}): {}",
            step_id, question
        ),
        PipelineResult::AwaitingApproval {
            doc_id, step_id, ..
        } => format!(
            "⏳ 等待朱批（步骤 {}，文档 {}）— 准奏后流程继续",
            step_id, doc_id
        ),
        PipelineResult::StepFailed {
            step_id, reason, ..
        } => format!("❌ Pipeline step {} failed: {}", step_id, reason),
        PipelineResult::Aborted { .. } => "🛑 Pipeline execution aborted".to_string(),
        PipelineResult::Deadlock { .. } => {
            "❌ Pipeline deadlock: remaining steps have unmet dependencies.".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_starts_not_running() {
        let sup = PipelineSupervisor::new();
        assert!(!sup.is_running());
    }

    #[test]
    fn supervisor_ignores_duplicate_completed_plan_id() {
        let sup = PipelineSupervisor::new();
        sup.test_mark_completed("plan-dup");
        assert_eq!(
            sup.test_should_reject_plan("plan-dup"),
            Some("duplicate completed")
        );
        assert!(sup.test_should_reject_plan("plan-new").is_none());
    }

    #[test]
    fn cancel_latch_blocks_tail_restarted_plan() {
        let sup = PipelineSupervisor::new();
        sup.test_mark_aborted();
        assert_eq!(
            sup.test_should_reject_plan("any-plan"),
            Some("cancel latch")
        );
        sup.clear_submission_guards();
        assert!(sup.test_should_reject_plan("any-plan").is_none());
    }

    #[test]
    fn pipeline_summary_message_disallows_plan() {
        let msg = ActorMessage::pipeline_summary("summarize");
        assert!(!msg.allow_pipeline_plan);
    }

    #[test]
    fn resume_rejects_while_supervisor_running() {
        let sup = PipelineSupervisor::new();
        sup.test_set_running(true);
        assert!(sup.is_running());
    }
}
