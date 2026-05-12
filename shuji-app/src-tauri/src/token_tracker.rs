#![allow(dead_code)]
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use chrono::{Timelike, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub call_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenRecord {
    role: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    timestamp: chrono::DateTime<Utc>,
}

static RECORDS: Mutex<Option<Vec<TokenRecord>>> = Mutex::new(None);
static STORAGE_PATH: Mutex<Option<String>> = Mutex::new(None);

/// Initialize the token tracker with a file path for persistence.
/// Loads existing records from the file if it exists.
pub fn init(file_path: &Path) {
    if let Ok(mut path) = STORAGE_PATH.lock() {
        *path = Some(file_path.to_string_lossy().to_string());
    }
    if let Ok(mut records) = RECORDS.lock() {
        *records = load_from_file(file_path);
    }
}

/// Record token usage for a given role.
pub fn record(role: &str, prompt: u64, completion: u64) {
    let mut lock = match RECORDS.lock() {
        Ok(l) => l,
        Err(_) => return,
    };
    let records = lock.get_or_insert_with(Vec::new);
    records.push(TokenRecord {
        role: role.to_string(),
        prompt_tokens: prompt,
        completion_tokens: completion,
        timestamp: Utc::now(),
    });
    // Persist to file after each record
    if let Ok(path_lock) = STORAGE_PATH.lock() {
        if let Some(ref path) = *path_lock {
            let _ = save_to_file(Path::new(path), &records);
        }
    }
}

/// Time window for aggregating token stats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenWindow {
    Today,
    Last3Days,
    Last7Days,
    LastNDays(u64),
    All,
}

impl TokenWindow {
    pub fn label(&self) -> String {
        match self {
            TokenWindow::Today => "今日".to_string(),
            TokenWindow::Last3Days => "近3天".to_string(),
            TokenWindow::Last7Days => "近7天".to_string(),
            TokenWindow::LastNDays(n) => format!("近{}天", n),
            TokenWindow::All => "汇总".to_string(),
        }
    }
}

fn aggregate(records: &[TokenRecord], window: TokenWindow) -> HashMap<String, TokenUsage> {
    let cutoff = match window {
        TokenWindow::Today => {
            let now = Utc::now();
            Some(now - chrono::Duration::hours(now.hour() as i64)
                        - chrono::Duration::minutes(now.minute() as i64)
                        - chrono::Duration::seconds(now.second() as i64))
        }
        TokenWindow::Last3Days => Some(Utc::now() - chrono::Duration::days(3)),
        TokenWindow::Last7Days => Some(Utc::now() - chrono::Duration::days(7)),
        TokenWindow::LastNDays(n) => Some(Utc::now() - chrono::Duration::days(n as i64)),
        TokenWindow::All => None,
    };

    let mut map: HashMap<String, TokenUsage> = HashMap::new();
    for rec in records {
        if let Some(c) = cutoff {
            if rec.timestamp < c {
                continue;
            }
        }
        let entry = map.entry(rec.role.clone()).or_insert_with(TokenUsage::default);
        entry.prompt_tokens += rec.prompt_tokens;
        entry.completion_tokens += rec.completion_tokens;
        entry.total_tokens += rec.prompt_tokens + rec.completion_tokens;
        entry.call_count += 1;
    }
    map
}

pub fn snapshot() -> HashMap<String, TokenUsage> {
    snapshot_window(TokenWindow::All)
}

pub fn snapshot_window(window: TokenWindow) -> HashMap<String, TokenUsage> {
    let lock = match RECORDS.lock() {
        Ok(l) => l,
        Err(_) => return HashMap::new(),
    };
    match lock.as_ref() {
        Some(records) => aggregate(records, window),
        None => HashMap::new(),
    }
}

pub fn snapshot_grouped() -> HashMap<String, HashMap<String, TokenUsage>> {
    let mut result = HashMap::new();
    for window in &[TokenWindow::Today, TokenWindow::Last3Days, TokenWindow::Last7Days, TokenWindow::All] {
        result.insert(window.label().to_string(), snapshot_window(*window));
    }
    result
}

/// Clear all tracked data (memory + file).
pub fn clear() {
    if let Ok(mut lock) = RECORDS.lock() {
        *lock = None;
    }
    if let Ok(path_lock) = STORAGE_PATH.lock() {
        if let Some(ref path) = *path_lock {
            let _ = std::fs::remove_file(Path::new(path));
        }
    }
}

fn load_from_file(path: &Path) -> Option<Vec<TokenRecord>> {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).ok(),
        Err(_) => None,
    }
}

fn save_to_file(path: &Path, records: &[TokenRecord]) -> Result<(), String> {
    let json = serde_json::to_string(records).map_err(|e| e.to_string())?;
    std::fs::write(path, &json).map_err(|e| e.to_string())
}
