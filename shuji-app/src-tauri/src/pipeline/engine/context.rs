//! PipelineEngineContext: subsystem dependencies bundled for engine construction.
//!
//! Extracted from `mod.rs`. Kept as its own file since it is used by multiple
//! constructors (new, from_actor_system, from_runtime, from_runtime_context,
//! load_from_disk).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::actor::ActorMessage;
use crate::config::RuntimeConfig;
use crate::models::role::Role;
use crate::workflow::WorkflowGraph;
use tokio::sync::mpsc;

/// Bundled subsystem references needed to construct a `PipelineEngine`.
/// Created by `PipelineEngineContext::from_actor_system()` or the
/// test helper `lightweight_for_tests()`.
pub struct PipelineEngineContext {
    pub actor_txs: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    pub fast_txs: crate::FastTxMap,
    pub cancel_map: crate::CancelMap,
    pub cancel: Arc<AtomicBool>,
    pub project_dir: PathBuf,
    pub workflow_graph: Option<Arc<tokio::sync::Mutex<WorkflowGraph>>>,
    pub runtime_config: Arc<RuntimeConfig>,
}

impl PipelineEngineContext {
    pub fn from_actor_system(
        actor_system: &crate::actor::ActorSystem,
        project_dir: PathBuf,
        runtime_config: Arc<RuntimeConfig>,
    ) -> Self {
        Self {
            actor_txs: actor_system.senders.clone(),
            fast_txs: Arc::new(actor_system.fast_txs.clone()),
            cancel_map: actor_system.cancel_map.clone(),
            cancel: actor_system.cancel.clone(),
            project_dir,
            workflow_graph: Some(actor_system.workflow_graph.clone()),
            runtime_config,
        }
    }

    pub fn lightweight_for_tests(
        actor_txs: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
        project_dir: PathBuf,
        runtime_config: Arc<RuntimeConfig>,
    ) -> Self {
        Self {
            actor_txs,
            fast_txs: Arc::new(HashMap::new()),
            cancel_map: Arc::new(HashMap::<Role, Arc<AtomicBool>>::new()),
            cancel: Arc::new(AtomicBool::new(false)),
            project_dir,
            workflow_graph: None,
            runtime_config,
        }
    }
}
