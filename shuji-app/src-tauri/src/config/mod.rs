use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// 系统运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub api: ApiConfig,
    pub tool_iterations: ToolIterationsConfig,
    pub context_compaction: ContextCompactionConfig,
    pub actor: ActorConfig,
    pub watchdog: WatchdogConfig,
    pub checkpoint: CheckpointConfig,
}

/// API 相关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API 请求超时时间（秒）
    #[serde(default = "default_api_timeout")]
    pub timeout_secs: u64,

    /// API 请求最大重试次数
    #[serde(default = "default_api_max_retries")]
    pub max_retries: u32,

    /// 截断响应最大重试次数
    #[serde(default = "default_length_max_retries")]
    pub length_max_retries: u32,

    /// 不同类型 agent 的 max_tokens 配置；设为 0 时不发送该字段，让模型/服务端使用可用上限
    pub max_tokens: MaxTokensConfig,

    /// 推理/思考模式配置
    #[serde(default)]
    pub reasoning: ReasoningConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaxTokensConfig {
    /// 写文件类 agent (工部、兵部)
    #[serde(default = "default_write_file_tokens")]
    pub write_file: u32,

    /// 追加文档类 agent (中书令、吏部、刑部)
    #[serde(default = "default_append_document_tokens")]
    pub append_document: u32,

    /// 只读类 agent (礼部)
    #[serde(default = "default_readonly_tokens")]
    pub readonly: u32,

    /// 纯文本类 agent
    #[serde(default = "default_text_only_tokens")]
    pub text_only: u32,
}

/// 工具迭代次数限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolIterationsConfig {
    /// 只读操作的最大迭代次数
    #[serde(default = "default_readonly_iterations")]
    pub readonly: usize,

    /// 写文件密集操作的最大迭代次数
    #[serde(default = "default_write_heavy_iterations")]
    pub write_heavy: usize,

    /// 文档操作密集的最大迭代次数
    #[serde(default = "default_document_heavy_iterations")]
    pub document_heavy: usize,
}

/// 上下文压缩配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCompactionConfig {
    /// 触发压缩的字符数阈值
    #[serde(default = "default_compact_char_threshold")]
    pub char_threshold: usize,

    /// 压缩后保留的最近消息数
    #[serde(default = "default_keep_recent_count")]
    pub keep_recent_count: usize,

    /// 历史消息压缩的字符数阈值
    #[serde(default = "default_history_compact_threshold")]
    pub history_char_threshold: usize,

    /// 是否启用运行中上下文压缩（tool 循环中的 mid-run compact）
    #[serde(default = "default_compact_mid_run_enabled")]
    pub mid_run_compact: bool,
}

/// Actor 系统配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorConfig {
    /// 单个 actor 执行的最大迭代次数
    #[serde(default = "default_max_exec_iterations")]
    pub max_exec_iterations: u32,

    /// 工部计划循环的最大迭代次数
    #[serde(default = "default_max_plan_iterations")]
    pub max_plan_iterations: u32,
}

/// Watchdog 监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogConfig {
    /// 连续错误的最大次数
    #[serde(default = "default_max_consecutive_errors")]
    pub max_consecutive_errors: u32,

    /// 触发警告的重复工具调用次数
    #[serde(default = "default_same_tool_warning_count")]
    pub same_tool_warning_count: u32,

    /// 触发警告的连续读取次数（无写入）
    #[serde(default = "default_read_without_write_warning")]
    pub read_without_write_warning: u32,
}

/// 每个角色可选的上下文窗口覆盖配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleContextConfig {
    #[serde(default)]
    pub char_threshold: Option<usize>,
    #[serde(default)]
    pub keep_recent_count: Option<usize>,
    #[serde(default)]
    pub history_char_threshold: Option<usize>,
    #[serde(default)]
    pub mid_run_compact: Option<bool>,
}

/// 已解析的上下文压缩阈值（合并了全局默认值与角色覆盖后）
#[derive(Debug, Clone, Copy)]
pub struct CompactThresholds {
    pub char_threshold: usize,
    pub keep_recent_count: usize,
    pub history_char_threshold: usize,
    pub mid_run_compact: bool,
}

/// Checkpoint 保存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// 自动 checkpoint 间隔（秒），0 = 不启用自动保存
    #[serde(default = "default_checkpoint_interval")]
    pub interval_secs: u64,
}

/// Checkpoint 默认值
fn default_checkpoint_interval() -> u64 {
    300
}

/// 推理/思考模式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// 是否启用思考模式
    #[serde(default = "default_reasoning_enabled")]
    pub enabled: bool,
    /// 思考预算 token 数（0 = 使用模型默认值；仅 Anthropic API 有效）
    #[serde(default = "default_reasoning_budget")]
    pub budget_tokens: u32,
}

// ── 默认值函数 ────────────────────────────────────────────────

fn default_api_timeout() -> u64 {
    180
}
fn default_api_max_retries() -> u32 {
    3
}
fn default_length_max_retries() -> u32 {
    5
}

fn default_write_file_tokens() -> u32 {
    0
} // keep unlimited for code generation
fn default_append_document_tokens() -> u32 {
    4096
}
fn default_readonly_tokens() -> u32 {
    2048
}
fn default_text_only_tokens() -> u32 {
    1024
}

fn default_readonly_iterations() -> usize {
    80
}
fn default_write_heavy_iterations() -> usize {
    60
}
fn default_document_heavy_iterations() -> usize {
    100
}

fn default_compact_char_threshold() -> usize {
    80_000
} // compact earlier (was 160K)
fn default_keep_recent_count() -> usize {
    10
} // keep more context after compaction (was 6)
fn default_history_compact_threshold() -> usize {
    2_000
}

fn default_compact_mid_run_enabled() -> bool {
    true
}

fn default_max_exec_iterations() -> u32 {
    20
}
fn default_max_plan_iterations() -> u32 {
    6
}

fn default_reasoning_enabled() -> bool {
    true
}
fn default_reasoning_budget() -> u32 {
    0
}

fn default_max_consecutive_errors() -> u32 {
    5
}
fn default_same_tool_warning_count() -> u32 {
    3
}
fn default_read_without_write_warning() -> u32 {
    5
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            interval_secs: default_checkpoint_interval(),
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
            tool_iterations: ToolIterationsConfig::default(),
            context_compaction: ContextCompactionConfig::default(),
            actor: ActorConfig::default(),
            watchdog: WatchdogConfig::default(),
            checkpoint: CheckpointConfig::default(),
        }
    }
}

impl Default for ReasoningConfig {
    fn default() -> Self {
        Self {
            enabled: default_reasoning_enabled(),
            budget_tokens: default_reasoning_budget(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_api_timeout(),
            max_retries: default_api_max_retries(),
            length_max_retries: default_length_max_retries(),
            max_tokens: MaxTokensConfig::default(),
            reasoning: ReasoningConfig::default(),
        }
    }
}

impl Default for MaxTokensConfig {
    fn default() -> Self {
        Self {
            write_file: default_write_file_tokens(),
            append_document: default_append_document_tokens(),
            readonly: default_readonly_tokens(),
            text_only: default_text_only_tokens(),
        }
    }
}

impl Default for ToolIterationsConfig {
    fn default() -> Self {
        Self {
            readonly: default_readonly_iterations(),
            write_heavy: default_write_heavy_iterations(),
            document_heavy: default_document_heavy_iterations(),
        }
    }
}

impl Default for ContextCompactionConfig {
    fn default() -> Self {
        Self {
            char_threshold: default_compact_char_threshold(),
            keep_recent_count: default_keep_recent_count(),
            history_char_threshold: default_history_compact_threshold(),
            mid_run_compact: default_compact_mid_run_enabled(),
        }
    }
}

impl Default for ActorConfig {
    fn default() -> Self {
        Self {
            max_exec_iterations: default_max_exec_iterations(),
            max_plan_iterations: default_max_plan_iterations(),
        }
    }
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            max_consecutive_errors: default_max_consecutive_errors(),
            same_tool_warning_count: default_same_tool_warning_count(),
            read_without_write_warning: default_read_without_write_warning(),
        }
    }
}

impl RuntimeConfig {
    /// 从 TOML 文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: RuntimeConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// 加载配置，如果文件不存在则使用默认值
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        let path_ref = path.as_ref();
        match Self::from_file(path_ref) {
            Ok(config) => {
                log_console!("[config] 已从 {} 加载配置", path_ref.display());
                log_console!(
                    "[debug] 配置值: api.timeout={}, api.max_retries={}, compact.char_threshold={}, compact.mid_run_compact={}",
                    config.api.timeout_secs,
                    config.api.max_retries,
                    config.context_compaction.char_threshold,
                    config.context_compaction.mid_run_compact,
                );
                config
            }
            Err(e) => {
                // Only log error if it's not just "file not found"
                if path_ref.exists() {
                    log_console!("[config] 配置文件解析失败，使用默认值: {}", e);
                } else {
                    log_console!(
                        "[config] 配置文件不存在，使用默认值 (可创建 {} 自定义配置)",
                        path_ref.display()
                    );
                }
                Self::default()
            }
        }
    }

    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 获取 API 超时时间
    pub fn api_timeout(&self) -> Duration {
        Duration::from_secs(self.api.timeout_secs)
    }

    /// 为指定角色解析上下文压缩阈值，合并全局默认值与角色覆盖。
    pub fn resolve_compact_thresholds(
        &self,
        _role_name: &str,
        role_config: Option<&RoleContextConfig>,
    ) -> CompactThresholds {
        let ov = role_config;
        CompactThresholds {
            char_threshold: ov
                .and_then(|o| o.char_threshold)
                .unwrap_or(self.context_compaction.char_threshold),
            keep_recent_count: ov
                .and_then(|o| o.keep_recent_count)
                .unwrap_or(self.context_compaction.keep_recent_count),
            history_char_threshold: ov
                .and_then(|o| o.history_char_threshold)
                .unwrap_or(self.context_compaction.history_char_threshold),
            mid_run_compact: ov
                .and_then(|o| o.mid_run_compact)
                .unwrap_or(self.context_compaction.mid_run_compact),
        }
    }
}
