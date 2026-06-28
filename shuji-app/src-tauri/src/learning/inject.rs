use std::path::Path;

use super::store::SoulStore;

/// Build the markdown block injected as `[soul: Role]`.
pub async fn load_role_soul(working_dir: &Path, role_name: &str) -> String {
    let cfg = SoulStore::config();
    SoulStore::load_for_injection(working_dir, role_name, cfg.global_enabled)
        .await
        .unwrap_or_default()
}
