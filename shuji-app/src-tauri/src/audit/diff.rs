use std::path::Path;

/// Save a unified diff patch to `.shuji/audit/diffs/` for document change tracking.
/// `old_body` and `new_body` are the document body text before and after the change.
/// `doc_id` is the document ID (e.g. "dsgn_003"), and `event` is e.g. "modify_document".
pub async fn save_diff(
    working_dir: &Path,
    doc_id: &str,
    event: &str,
    old_body: &str,
    new_body: &str,
) {
    let diff_dir = working_dir.join(".shuji").join("audit").join("diffs");
    let _ = tokio::fs::create_dir_all(&diff_dir).await;
    let ts = chrono::Local::now().format("%Y%m%d%H%M%S");
    let diff_path = diff_dir.join(format!("{}_{}_{}.patch", doc_id, event, ts));
    let patch = diffy::create_patch(old_body, new_body);
    let patch_str = patch.to_string();
    // Only save if there's actual change content
    if patch_str.lines().count() > 2 {
        let _ = tokio::fs::write(&diff_path, &patch_str).await;
    }
}
