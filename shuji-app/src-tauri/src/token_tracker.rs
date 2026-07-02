use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use chrono::{Timelike, Utc};
use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub estimated_cost_cny: Option<f64>,
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
    #[serde(default)]
    estimated_cost_cny: Option<f64>,
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
/// Uses the pricing module (with .shuji/pricing.json) for cost estimation.
pub fn record(role: &str, prompt: u64, cached: u64, completion: u64, model: &str) {
    let uncached = prompt.saturating_sub(cached);
    let (cost_usd, cost_cny) = estimate_dual_cost(prompt, cached, completion, model);
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
        estimated_cost: cost_usd,
        estimated_cost_cny: cost_cny,
        timestamp: Utc::now(),
    });
    if let Ok(path_lock) = STORAGE_PATH.lock() {
        if let Some(ref path) = *path_lock {
            let _ = save_to_file(Path::new(path), records);
        }
    }
    crate::usage_notify::notify(role, crate::usage_notify::UsageUpdateKind::Token);
}

/// Estimate cost in both USD and CNY using the pricing module.
/// Only DeepSeek models get cost estimation; all others track tokens only.
fn estimate_dual_cost(
    prompt: u64,
    cached: u64,
    completion: u64,
    model: &str,
) -> (Option<f64>, Option<f64>) {
    if !model.to_lowercase().contains("deepseek") {
        return (None, None);
    }

    let working_dir = {
        let path_lock = match STORAGE_PATH.lock() {
            Ok(p) => p.clone(),
            Err(_) => None,
        };
        path_lock.and_then(|p| {
            Path::new(&p)
                .parent()
                .and_then(|parent| parent.parent())
                .map(|p| p.to_path_buf())
        })
    };

    let config = working_dir
        .as_ref()
        .map(|wd| crate::pricing::load_or_init(wd));

    match config {
        Some(ref cfg) => {
            let usd = cfg.estimate_cost(model, prompt, cached, completion, "usd");
            let cny = cfg.estimate_cost(model, prompt, cached, completion, "cny");
            (usd, cny)
        }
        None => {
            let usd = fallback_deepseek_cost(prompt, completion, model);
            let cny = usd.map(|v| v * 7.25);
            (usd, cny)
        }
    }
}

/// Fallback cost for DeepSeek when pricing config is not available.
fn fallback_deepseek_cost(prompt: u64, completion: u64, model: &str) -> Option<f64> {
    let model_lower = model.to_lowercase();
    let (input_pm, output_pm) = if model_lower.contains("reasoner") || model_lower.contains("r1") {
        (0.55, 2.19)
    } else {
        (0.14, 0.28)
    };
    let input_cost = prompt as f64 / 1_000_000.0 * input_pm;
    let output_cost = completion as f64 / 1_000_000.0 * output_pm;
    Some(input_cost + output_cost)
}

/// Time window for aggregating token stats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenWindow {
    Today,
    Last3Days,
    Last7Days,
    #[allow(dead_code)]
    LastNDays(u64),
    All,
}

impl TokenWindow {
    pub fn label(&self) -> String {
        match self {
            TokenWindow::Today => "Today".to_string(),
            TokenWindow::Last3Days => "Last 3 Days".to_string(),
            TokenWindow::Last7Days => "Last 7 Days".to_string(),
            TokenWindow::LastNDays(n) => format!("Last {} Days", n),
            TokenWindow::All => "All Time".to_string(),
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
        entry.estimated_cost =
            Some(entry.estimated_cost.unwrap_or(0.0) + rec.estimated_cost.unwrap_or(0.0))
                .filter(|&c| c > 0.0);
        entry.estimated_cost_cny =
            Some(entry.estimated_cost_cny.unwrap_or(0.0) + rec.estimated_cost_cny.unwrap_or(0.0))
                .filter(|&c| c > 0.0);
    }
    map
}

#[allow(dead_code)]
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

/// Recalculate all existing token records using the current pricing config.
/// Called after refreshing pricing from the provider website.
pub fn recalculate_all(working_dir: &Path) -> Result<(), String> {
    let config = crate::pricing::load_or_init(working_dir);
    let path_lock = STORAGE_PATH.lock().map_err(|e| e.to_string())?;
    let storage_path = match path_lock.as_ref() {
        Some(p) => Path::new(p).to_path_buf(),
        None => return Err("token_tracker not initialized".to_string()),
    };
    drop(path_lock);

    let mut records = load_from_file(&storage_path).unwrap_or_default();
    let mut changed = false;
    for rec in &mut records {
        let (usd, cny) = if rec.model.to_lowercase().contains("deepseek") {
            let u = config.estimate_cost(
                &rec.model,
                rec.prompt_tokens,
                rec.cached_prompt_tokens,
                rec.completion_tokens,
                "usd",
            );
            let c = config.estimate_cost(
                &rec.model,
                rec.prompt_tokens,
                rec.cached_prompt_tokens,
                rec.completion_tokens,
                "cny",
            );
            (u, c)
        } else {
            (None, None)
        };
        if rec.estimated_cost != usd || rec.estimated_cost_cny != cny {
            rec.estimated_cost = usd;
            rec.estimated_cost_cny = cny;
            changed = true;
        }
    }

    // Update in-memory cache
    if let Ok(mut lock) = RECORDS.lock() {
        *lock = Some(records.clone());
    }

    if changed {
        save_to_file(&storage_path, &records)?;
    }
    Ok(())
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
