use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single audit log entry persisted to .shuji/audit.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub ts: String,
    pub event: String,
    pub role: String,
    pub doc_id: String,
    pub detail: String,
    #[serde(default)]
    pub prev_hash: String,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub seq: u64,
}

/// Result of verifying the hash chain integrity of the audit log.
#[derive(Debug, Clone, Serialize)]
pub struct VerificationReport {
    pub total_entries: u64,
    pub chain_intact: bool,
    pub first_entry_hash: String,
    pub last_entry_hash: String,
    pub broken_links: Vec<BrokenLink>,
    pub first_tampered_seq: Option<u64>,
    /// Number of entries before hash chain was established (pre-upgrade records).
    pub pre_chain_entries: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrokenLink {
    pub seq: u64,
    pub expected_prev_hash: String,
    pub actual_prev_hash: String,
}

pub(crate) const ZERO_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Compute SHA-256 of the canonical JSON + prev_hash chain.
pub(crate) fn compute_hash(json: &str, prev_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    hasher.update(b"\n");
    hasher.update(prev_hash.as_bytes());
    format!("{:064x}", hasher.finalize())
}

/// Get the hash of the last entry in the audit log.
async fn get_last_hash(path: &Path) -> String {
    let content = tokio::fs::read_to_string(path).await.unwrap_or_default();
    content
        .lines()
        .last()
        .and_then(|line| {
            serde_json::from_str::<AuditEntry>(line)
                .ok()
                .filter(|e| !e.hash.is_empty())
                .map(|e| e.hash)
        })
        .unwrap_or_else(|| ZERO_HASH.to_string())
}

/// Count the number of lines in the audit log.
async fn count_lines(path: &Path) -> u64 {
    let content = tokio::fs::read_to_string(path).await.unwrap_or_default();
    content.lines().count() as u64
}

/// Append a single audit entry to .shuji/audit.jsonl with hash chain.
pub async fn append(working_dir: &Path, event: &str, role: &str, doc_id: &str, detail: &str) {
    let path = working_dir.join(".shuji").join("audit.jsonl");
    if let Some(parent) = path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            log_console!("[audit] failed to create directory: {}", e);
        }
    }

    let prev_hash = get_last_hash(&path).await;
    let seq = count_lines(&path).await + 1;

    let mut entry = AuditEntry {
        ts: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        event: event.to_string(),
        role: role.to_string(),
        doc_id: doc_id.to_string(),
        detail: detail.to_string(),
        prev_hash: prev_hash.clone(),
        hash: String::new(),
        seq,
    };

    // Compute hash from entry JSON (without hash field) + prev_hash
    let json_no_hash = serde_json::to_string(&serde_json::json!({
        "ts": entry.ts,
        "event": entry.event,
        "role": entry.role,
        "doc_id": entry.doc_id,
        "detail": entry.detail,
        "prev_hash": entry.prev_hash,
        "seq": entry.seq,
    }))
    .unwrap_or_default();
    entry.hash = compute_hash(&json_no_hash, &prev_hash);

    if let Ok(json) = serde_json::to_string(&entry) {
        use tokio::io::AsyncWriteExt;
        match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            Ok(mut f) => {
                if let Err(e) = f.write_all(format!("{}\n", json).as_bytes()).await {
                    log_console!("[audit] failed to write audit.jsonl: {}", e);
                }
            }
            Err(e) => {
                log_console!("[audit] failed to open audit.jsonl: {}", e);
            }
        }
    } else {
        log_console!("[audit] failed to serialize audit entry");
    }
}

/// Read all audit entries from .shuji/audit.jsonl.
pub async fn read_all(working_dir: &Path) -> Vec<AuditEntry> {
    let path = working_dir.join(".shuji").join("audit.jsonl");
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Verify the SHA-256 hash chain integrity of the entire audit log.
/// Returns a report with all broken links detected.
///
/// Entries without hash fields (pre-upgrade records) are reported separately
/// and the chain is considered to start from the first entry with a non-empty hash.
pub async fn verify_audit_trail(working_dir: &Path) -> Result<VerificationReport, String> {
    let entries = read_all(working_dir).await;
    if entries.is_empty() {
        return Ok(VerificationReport {
            total_entries: 0,
            chain_intact: true,
            first_entry_hash: String::new(),
            last_entry_hash: String::new(),
            broken_links: vec![],
            first_tampered_seq: None,
            pre_chain_entries: 0,
        });
    }

    let mut pre_chain = 0u64;
    let mut expected_prev = ZERO_HASH.to_string();
    let mut broken_links = Vec::new();

    for entry in &entries {
        if entry.hash.is_empty() {
            // Pre-upgrade entry - skip chain check
            pre_chain += 1;
            continue;
        }

        // Recompute expected hash
        let json_no_hash = serde_json::to_string(&serde_json::json!({
            "ts": entry.ts,
            "event": entry.event,
            "role": entry.role,
            "doc_id": entry.doc_id,
            "detail": entry.detail,
            "prev_hash": entry.prev_hash,
            "seq": entry.seq,
        }))
        .unwrap_or_default();
        let expected_hash = compute_hash(&json_no_hash, &expected_prev);

        if entry.prev_hash != expected_prev {
            broken_links.push(BrokenLink {
                seq: entry.seq,
                expected_prev_hash: expected_prev.clone(),
                actual_prev_hash: entry.prev_hash.clone(),
            });
        }

        if entry.hash != expected_hash {
            broken_links.push(BrokenLink {
                seq: entry.seq,
                expected_prev_hash: expected_hash,
                actual_prev_hash: entry.hash.clone(),
            });
        }

        expected_prev = entry.hash.clone();
    }

    let chain_intact = broken_links.is_empty();
    let last_entry = entries.iter().rfind(|e| !e.hash.is_empty());
    let first_entry = entries.iter().find(|e| !e.hash.is_empty());

    Ok(VerificationReport {
        total_entries: entries.len() as u64,
        chain_intact,
        first_entry_hash: first_entry.map(|e| e.hash.clone()).unwrap_or_default(),
        last_entry_hash: last_entry.map(|e| e.hash.clone()).unwrap_or_default(),
        first_tampered_seq: broken_links.first().map(|b| b.seq),
        broken_links,
        pre_chain_entries: pre_chain,
    })
}
