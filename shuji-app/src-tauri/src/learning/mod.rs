pub mod config;
pub mod entry;
pub mod extract;
pub mod helpers;
pub mod inject;
pub mod role;
pub mod store;

pub use config::{load_config, save_config, set_global_enabled, set_test_home_dir, LearningConfig};
pub use entry::{LearningEntry, LearningKind, LearningScope};
pub use extract::LearningExtractor;
pub use inject::load_role_soul;
pub use role::{ensure_canonical_role, normalize_role_name};
pub use store::{
    SoulStore, GLOBAL_INJECT_BUDGET, MAX_ENTRY_CHARS, MAX_INJECTED_CHARS, MAX_SOUL_FILE_BYTES,
    PROJECT_INJECT_BUDGET,
};
