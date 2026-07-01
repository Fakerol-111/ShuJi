use std::path::Path;

use super::log;

/// Write a re-audit request file that the actor system can detect.
/// The `subject` is a document ID that 礼部 should re-audit.
pub async fn request_reauth(working_dir: &Path, subject: &str, reason: &str) -> String {
    let dir = working_dir.join(".shuji").join("audit");
    let _ = tokio::fs::create_dir_all(&dir).await;
    let path = dir.join("reauth_request.json");
    let request = serde_json::json!({
        "ts": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        "subject": subject,
        "reason": reason,
    });
    if let Ok(json) = serde_json::to_string_pretty(&request) {
        let _ = tokio::fs::write(&path, &json).await;
    }
    log::append(
        working_dir,
        "reauth_request",
        "system",
        subject,
        &format!("Requesting re-audit: {}", reason),
    )
    .await;
    format!("Re-audit request submitted: {} ({})", subject, reason)
}

/// Check if there's a pending re-auth request and clear it.
pub async fn consume_reauth_request(working_dir: &Path) -> Option<(String, String)> {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("reauth_request.json");
    let content = tokio::fs::read_to_string(&path).await.ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    let _ = tokio::fs::remove_file(&path).await;
    Some((
        v.get("subject")?.as_str()?.to_string(),
        v.get("reason")?.as_str()?.to_string(),
    ))
}
