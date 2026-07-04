pub mod approval;
pub mod compaction;
pub mod esaa_contract;
pub mod reasoning;
pub mod types;

// Re-export all public types at crate::config level for backward compatibility
pub use approval::{ApprovalConfig, ApprovalMode};
pub use compaction::{CompactThresholds, RoleContextConfig};
pub use reasoning::{
    ReasoningConfig, ReasoningEffort, ReasoningPhase, ResolvedReasoningPolicy, RoleReasoningConfig,
};
pub use types::{
    ActorConfig, ApiConfig, CheckpointConfig, ContextCompactionConfig, EsaaConfig, MaxTokensConfig,
    RetryConfig, RuntimeConfig, StreamingConfig, ToolIterationsConfig, WatchdogConfig,
};

// Re-export the standalone function
pub use compaction::default_compact_thresholds_for_role;
