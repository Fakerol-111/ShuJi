//! Skill loading — disk-based skill files.
//! Moved from neige/mod.rs for separation of concerns.

use std::path::Path;

/// Load skill content from `.shuji/skills/{name}.md` on disk.
/// Returns empty string if the skill is not found.
pub async fn load_skill(name: &str, working_dir: &Path) -> String {
    let disk_path = working_dir
        .join(".shuji")
        .join("skills")
        .join(format!("{}.md", name));
    if let Ok(content) = tokio::fs::read_to_string(&disk_path).await {
        if !content.trim().is_empty() {
            log_console!("[内阁] load skill from disk: {}", name);
            return content;
        }
    }
    String::new()
}
