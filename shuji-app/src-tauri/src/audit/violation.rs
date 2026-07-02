use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub ts: String,
    pub severity: String, // error | warning | info
    pub rule_id: String,
    pub location: String,
    pub description: String,
    pub status: String, // open | fixed | waived
}

/// Record a violation to `.shuji/audit/violations.jsonl`.
pub async fn add_violation(
    working_dir: &Path,
    severity: &str,
    rule_id: &str,
    location: &str,
    description: &str,
) {
    let violation = Violation {
        ts: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        severity: severity.to_string(),
        rule_id: rule_id.to_string(),
        location: location.to_string(),
        description: description.to_string(),
        status: "open".to_string(),
    };
    let dir = working_dir.join(".shuji").join("audit");
    let _ = tokio::fs::create_dir_all(&dir).await;
    let path = dir.join("violations.jsonl");
    if let Ok(json) = serde_json::to_string(&violation) {
        use tokio::io::AsyncWriteExt;
        if let Ok(mut f) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            let _ = f.write_all(format!("{}\n", json).as_bytes()).await;
        }
    }
}

/// Read all violations.
pub async fn load_violations(working_dir: &Path) -> Vec<Violation> {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("violations.jsonl");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Update a violation's status (e.g. mark as fixed or waived).
pub async fn update_violation_status(
    working_dir: &Path,
    ts: &str,
    new_status: &str,
) -> Result<String, String> {
    let mut violations = load_violations(working_dir).await;
    if let Some(v) = violations.iter_mut().find(|v| v.ts == ts) {
        v.status = new_status.to_string();
        // Rewrite the file
        let path = working_dir
            .join(".shuji")
            .join("audit")
            .join("violations.jsonl");
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let mut content = String::new();
        for v in &violations {
            if let Ok(json) = serde_json::to_string(v) {
                content.push_str(&json);
                content.push('\n');
            }
        }
        let _ = tokio::fs::write(&path, &content).await;
        Ok(format!("Violation record updated to {}", new_status))
    } else {
        Err(format!("No matching violation record found (ts={})", ts))
    }
}
