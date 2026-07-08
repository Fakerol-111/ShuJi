use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrices {
    pub input_per_m: f64,
    pub output_per_m: f64,
    #[serde(default = "default_cache_hit")]
    pub cache_hit_input_per_m: f64,
}

fn default_cache_hit() -> f64 {
    0.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingEntry {
    pub model_pattern: String,
    pub display_name: String,
    pub usd: ModelPrices,
    #[serde(default)]
    pub cny: Option<ModelPrices>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    #[serde(default = "default_rate")]
    pub usd_cny_rate: f64,
    pub entries: Vec<PricingEntry>,
}

fn default_rate() -> f64 {
    7.25
}

impl PricingConfig {
    pub fn default_v4() -> Self {
        Self {
            usd_cny_rate: 7.25,
            entries: vec![
                PricingEntry {
                    model_pattern: "deepseek-v4-flash".to_string(),
                    display_name: "DeepSeek V4 Flash".to_string(),
                    usd: ModelPrices {
                        input_per_m: 0.14,
                        output_per_m: 0.28,
                        cache_hit_input_per_m: 0.0028,
                    },
                    cny: None,
                },
                PricingEntry {
                    model_pattern: "deepseek-v4-pro".to_string(),
                    display_name: "DeepSeek V4 Pro".to_string(),
                    usd: ModelPrices {
                        input_per_m: 0.435,
                        output_per_m: 0.87,
                        cache_hit_input_per_m: 0.003625,
                    },
                    cny: None,
                },
                PricingEntry {
                    model_pattern: "deepseek-r1".to_string(),
                    display_name: "DeepSeek R1".to_string(),
                    usd: ModelPrices {
                        input_per_m: 0.55,
                        output_per_m: 2.19,
                        cache_hit_input_per_m: 0.14,
                    },
                    cny: None,
                },
                PricingEntry {
                    model_pattern: "deepseek-chat".to_string(),
                    display_name: "DeepSeek Chat".to_string(),
                    usd: ModelPrices {
                        input_per_m: 0.14,
                        output_per_m: 0.28,
                        cache_hit_input_per_m: 0.0028,
                    },
                    cny: None,
                },
                PricingEntry {
                    model_pattern: "deepseek".to_string(),
                    display_name: "DeepSeek (fallback)".to_string(),
                    usd: ModelPrices {
                        input_per_m: 0.14,
                        output_per_m: 0.28,
                        cache_hit_input_per_m: 0.0028,
                    },
                    cny: None,
                },
            ],
        }
    }

    pub fn find_entry(&self, model: &str) -> Option<&PricingEntry> {
        let model_lower = model.to_lowercase();
        self.entries
            .iter()
            .find(|e| model_lower.contains(&e.model_pattern.to_lowercase()))
    }

    pub fn get_prices_for(&self, entry: &PricingEntry, currency: &str) -> ModelPrices {
        match currency {
            "cny" | "CNY" | "¥" => entry.cny.clone().unwrap_or(ModelPrices {
                input_per_m: entry.usd.input_per_m * self.usd_cny_rate,
                output_per_m: entry.usd.output_per_m * self.usd_cny_rate,
                cache_hit_input_per_m: entry.usd.cache_hit_input_per_m * self.usd_cny_rate,
            }),
            _ => entry.usd.clone(),
        }
    }

    pub fn estimate_cost(
        &self,
        model: &str,
        prompt: u64,
        cached: u64,
        completion: u64,
        currency: &str,
    ) -> Option<f64> {
        let entry = self.find_entry(model)?;
        let prices = self.get_prices_for(entry, currency);
        let uncached = prompt.saturating_sub(cached);
        let input_cost = uncached as f64 / 1_000_000.0 * prices.input_per_m
            + cached as f64 / 1_000_000.0 * prices.cache_hit_input_per_m;
        let output_cost = completion as f64 / 1_000_000.0 * prices.output_per_m;
        Some(input_cost + output_cost)
    }

    /// Suggest a cheaper model for the given role based on current model and usage.
    ///
    /// Returns `(model_pattern, display_name, estimated_savings_pct)` if a cheaper
    /// alternative exists, or `None` if the current model is already the cheapest.
    pub fn suggest_downgrade(&self, current_model: &str) -> Option<(&str, &str, f64)> {
        let current_entry = self.find_entry(current_model)?;
        let current_cost = current_entry.usd.input_per_m + current_entry.usd.output_per_m;

        let mut best: Option<(&PricingEntry, f64)> = None;
        for entry in &self.entries {
            let entry_cost = entry.usd.input_per_m + entry.usd.output_per_m;
            if entry_cost < current_cost {
                let savings = (1.0 - entry_cost / current_cost) * 100.0;
                match best {
                    None => best = Some((entry, savings)),
                    Some((_, best_savings)) if savings > best_savings => {
                        best = Some((entry, savings));
                    }
                    _ => {}
                }
            }
        }
        best.map(|(entry, savings)| (&entry.model_pattern[..], &entry.display_name[..], savings))
    }
}

// ── Global cache ──

static PRICING_CACHE: Mutex<Option<PricingConfig>> = Mutex::new(None);

pub fn load_or_init(working_dir: &Path) -> PricingConfig {
    if let Ok(guard) = PRICING_CACHE.lock() {
        if let Some(ref cached) = *guard {
            return cached.clone();
        }
    }
    let config = load_from_file(working_dir).unwrap_or_else(PricingConfig::default_v4);
    if let Ok(mut guard) = PRICING_CACHE.lock() {
        *guard = Some(config.clone());
    }
    config
}

pub fn invalidate_cache() {
    if let Ok(mut guard) = PRICING_CACHE.lock() {
        *guard = None;
    }
}

pub fn load_from_file(working_dir: &Path) -> Option<PricingConfig> {
    let path = working_dir.join(".shuji").join("pricing.json");
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_to_file(working_dir: &Path, config: &PricingConfig) -> Result<(), String> {
    let dir = working_dir.join(".shuji");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("pricing.json");
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, &json).map_err(|e| e.to_string())
}

pub async fn refresh_deepseek(working_dir: &Path) -> Result<PricingConfig, String> {
    let url = "https://api-docs.deepseek.com/quick_start/pricing";
    let client = reqwest::Client::builder()
        .user_agent("shuji/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP 客户端创建失败: {}", e))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;

    let values = parse_dollar_values(&text);
    if values.len() < 6 {
        return Err(format!(
            "解析到 {} 个价格值，预期至少 6 个。页面格式可能有变化",
            values.len()
        ));
    }

    let config = PricingConfig {
        usd_cny_rate: 7.25,
        entries: vec![
            PricingEntry {
                model_pattern: "deepseek-v4-flash".to_string(),
                display_name: "DeepSeek V4 Flash".to_string(),
                usd: ModelPrices {
                    cache_hit_input_per_m: values[0],
                    input_per_m: values[1],
                    output_per_m: values[2],
                },
                cny: None,
            },
            PricingEntry {
                model_pattern: "deepseek-v4-pro".to_string(),
                display_name: "DeepSeek V4 Pro".to_string(),
                usd: ModelPrices {
                    cache_hit_input_per_m: values[3],
                    input_per_m: values[4],
                    output_per_m: values[5],
                },
                cny: None,
            },
            PricingEntry {
                model_pattern: "deepseek-r1".to_string(),
                display_name: "DeepSeek R1".to_string(),
                usd: ModelPrices {
                    input_per_m: 0.55,
                    output_per_m: 2.19,
                    cache_hit_input_per_m: 0.14,
                },
                cny: None,
            },
            PricingEntry {
                model_pattern: "deepseek-chat".to_string(),
                display_name: "DeepSeek Chat".to_string(),
                usd: ModelPrices {
                    cache_hit_input_per_m: values[0],
                    input_per_m: values[1],
                    output_per_m: values[2],
                },
                cny: None,
            },
            PricingEntry {
                model_pattern: "deepseek".to_string(),
                display_name: "DeepSeek (fallback)".to_string(),
                usd: ModelPrices {
                    cache_hit_input_per_m: values[0],
                    input_per_m: values[1],
                    output_per_m: values[2],
                },
                cny: None,
            },
        ],
    };

    save_to_file(working_dir, &config)?;
    invalidate_cache();
    Ok(config)
}

fn parse_dollar_values(text: &str) -> Vec<f64> {
    let mut values = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            let mut num_str = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_ascii_digit() || nc == '.' {
                    num_str.push(nc);
                    chars.next();
                } else {
                    break;
                }
            }
            if !num_str.is_empty() {
                if let Ok(v) = num_str.parse::<f64>() {
                    if v > 0.0 {
                        values.push(v);
                    }
                }
            }
        }
    }
    values
}
