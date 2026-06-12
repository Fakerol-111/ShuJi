use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use chrono::{Timelike, Utc};
use serde::{Deserialize, Serialize};

/// 常见模型价格（美元 / 1M tokens）
#[derive(Debug, Clone)]
struct ModelPrice {
    input_per_m: f64,
    output_per_m: f64,
}

/// 根据模型名返回价格。未知模型返回 None。
fn get_model_price(model: &str) -> Option<ModelPrice> {
    let model_lower = model.to_lowercase();
    if model_lower.contains("deepseek") {
        if model_lower.contains("reasoner") || model_lower.contains("r1") {
            return Some(ModelPrice {
                input_per_m: 0.55,
                output_per_m: 2.19,
            });
        }
        return Some(ModelPrice {
            input_per_m: 0.14,
            output_per_m: 0.28,
        });
    }
    if model_lower.contains("claude") {
        if model_lower.contains("3.5") {
            return Some(ModelPrice {
                input_per_m: 3.00,
                output_per_m: 15.00,
            });
        }
        if model_lower.contains("3") && model_lower.contains("opus") {
            return Some(ModelPrice {
                input_per_m: 15.00,
                output_per_m: 75.00,
            });
        }
        if model_lower.contains("3") {
            return Some(ModelPrice {
                input_per_m: 3.00,
                output_per_m: 15.00,
            });
        }
        return Some(ModelPrice {
            input_per_m: 3.00,
            output_per_m: 15.00,
        });
    }
    if model_lower.contains("gpt-4o") {
        return Some(ModelPrice {
            input_per_m: 2.50,
            output_per_m: 10.00,
        });
    }
    if model_lower.contains("gpt-4") && model_lower.contains("mini") {
        return Some(ModelPrice {
            input_per_m: 0.15,
            output_per_m: 0.60,
        });
    }
    if model_lower.contains("gpt-4") {
        return Some(ModelPrice {
            input_per_m: 30.00,
            output_per_m: 60.00,
        });
    }
    if model_lower.contains("gpt-3.5") {
        return Some(ModelPrice {
            input_per_m: 0.50,
            output_per_m: 1.50,
        });
    }
    if model_lower.contains("gemini") {
        return Some(ModelPrice {
            input_per_m: 0.075,
            output_per_m: 0.30,
        });
    }
    None
}

fn estimate_cost(prompt: u64, completion: u64, model: &str) -> Option<f64> {
    let price = get_model_price(model)?;
    let input_cost = prompt as f64 / 1_000_000.0 * price.input_per_m;
    let output_cost = completion as f64 / 1_000_000.0 * price.output_per_m;
    Some(input_cost + output_cost)
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    #[serde(default)]
    pub cached_prompt_tokens: u64,
    #[serde(default)]
    pub uncached_prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub call_count: u64,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub estimated_cost: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenRecord {
    role: String,
    prompt_tokens: u64,
    #[serde(default)]
    cached_prompt_tokens: u64,
    #[serde(default)]
    uncached_prompt_tokens: u64,
    completion_tokens: u64,
    model: String,
    estimated_cost: Option<f64>,
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
pub fn record(role: &str, prompt: u64, cached: u64, completion: u64, model: &str) {
    let uncached = prompt.saturating_sub(cached);
    let cost = estimate_cost(prompt, completion, model);
    // Also update live round metrics
    crate::round_metrics::add_tokens(prompt, cached, completion);

    let mut lock = match RECORDS.lock() {
        Ok(l) => l,
        Err(_) => return,
    };
    let records = lock.get_or_insert_with(Vec::new);
    records.push(TokenRecord {
        role: role.to_string(),
        prompt_tokens: prompt,
        cached_prompt_tokens: cached,
        uncached_prompt_tokens: uncached,
        completion_tokens: completion,
        model: model.to_string(),
        estimated_cost: cost,
        timestamp: Utc::now(),
    });
    // Persist to file after each record
    if let Ok(path_lock) = STORAGE_PATH.lock() {
        if let Some(ref path) = *path_lock {
            let _ = save_to_file(Path::new(path), records);
        }
    }
}

/// Time window for aggregating token stats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenWindow {
    Today,
    Last3Days,
    Last7Days,
    #[allow(dead_code)] // reserved for custom date-range queries
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
            Some(
                now - chrono::Duration::hours(now.hour() as i64)
                    - chrono::Duration::minutes(now.minute() as i64)
                    - chrono::Duration::seconds(now.second() as i64),
            )
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
        let entry = map.entry(rec.role.clone()).or_default();
        entry.prompt_tokens += rec.prompt_tokens;
        entry.cached_prompt_tokens += rec.cached_prompt_tokens;
        entry.uncached_prompt_tokens += rec.uncached_prompt_tokens;
        entry.completion_tokens += rec.completion_tokens;
        entry.total_tokens += rec.prompt_tokens + rec.completion_tokens;
        entry.call_count += 1;
        entry.model = rec.model.clone();
        entry.estimated_cost = entry.estimated_cost.or(rec.estimated_cost);
    }
    map
}

#[allow(dead_code)] // convenience wrapper; snapshot_grouped covers current UI
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
    for window in &[
        TokenWindow::Today,
        TokenWindow::Last3Days,
        TokenWindow::Last7Days,
        TokenWindow::All,
    ] {
        result.insert(window.label().to_string(), snapshot_window(*window));
    }
    result
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
