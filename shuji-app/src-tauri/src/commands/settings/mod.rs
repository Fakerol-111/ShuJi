//! Settings commands — facade re-exporting all submodules.

pub mod api_config;
pub mod approval;
pub mod connection;
pub mod context;
pub mod diagnostics;
pub mod learning;
pub mod model_preset;
pub mod paths;
pub mod reasoning;
pub mod soul;
pub mod workflow_preset;

// Re-export types used by other crate modules
pub use api_config::{get_config, AppConfig};
pub use context::ContextWindowConfig;
