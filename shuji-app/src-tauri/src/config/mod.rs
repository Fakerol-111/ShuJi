pub mod esaa_contract;

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// 系统运行时配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    pub api: ApiConfig,
    pub tool_iterations: ToolIterationsConfig,
    pub context_compaction: ContextCompactionConfig,
    pub actor: ActorConfig,
    pub watchdog: WatchdogConfig,
    #[serde(default)]
    pub checkpoint: CheckpointConfig,
    #[serde(default)]
    pub esaa: EsaaConfig,
    #[serde(default)]
    pub approval: ApprovalConfig,
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

    /// LLM 流式响应（工具 agent）。默认关闭，失败时回退 step()。
    #[serde(default)]
    pub streaming: StreamingConfig,
}

/// Agent 流式输出配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamingConfig {
    #[serde(default)]
    pub enabled: bool,
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
    /// 触发压缩的 token 阈值（cl100k，仅计量 context_messages）
    #[serde(default = "default_compact_token_threshold", alias = "char_threshold")]
    pub token_threshold: usize,

    /// 压缩后保留的最近消息数
    #[serde(default = "default_keep_recent_count")]
    pub keep_recent_count: usize,

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

    /// 触发警告的 delete-create 循环次数
    /// 当同一路径被 delete_file 后紧接着 create_file 的次数达到此值，注入干预提示
    #[serde(default = "default_delete_create_warning_count")]
    pub delete_create_warning_count: u32,

    /// 测试僵局阈值：同一 role 连续多少次 run_tests 失败触发 hint
    #[serde(default = "default_test_stalemate_threshold")]
    pub test_stalemate_threshold: u32,
}

/// 每个角色可选的上下文窗口覆盖配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleContextConfig {
    #[serde(default, alias = "char_threshold")]
    pub token_threshold: Option<usize>,
    #[serde(default)]
    pub keep_recent_count: Option<usize>,
    #[serde(default)]
    pub mid_run_compact: Option<bool>,
}

/// 已解析的上下文压缩阈值（合并了全局默认值与角色覆盖后）
#[derive(Debug, Clone, Copy)]
pub struct CompactThresholds {
    pub token_threshold: usize,
    pub keep_recent_count: usize,
    pub mid_run_compact: bool,
}

/// ESAA 意图拦截层配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsaaConfig {
    /// 是否启用 intent 校验链
    #[serde(default = "default_esaa_enabled")]
    pub enabled: bool,
    /// 是否将每个已执行的 intent 记录到审计日志
    #[serde(default = "default_esaa_full_intent_log")]
    pub full_intent_log: bool,
}

fn default_esaa_enabled() -> bool {
    true
}
fn default_esaa_full_intent_log() -> bool {
    false
}

impl Default for EsaaConfig {
    fn default() -> Self {
        Self {
            enabled: default_esaa_enabled(),
            full_intent_log: default_esaa_full_intent_log(),
        }
    }
}

/// 朱批审批模式配置
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalMode {
    /// 等待用户朱批
    Manual,
    /// 自动放行
    Auto,
}

/// 朱批审批配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalConfig {
    /// 审批模式：manual = 等待用户朱批；auto = 自动放行
    #[serde(default = "default_approval_mode")]
    pub mode: ApprovalMode,
    /// auto 模式下，内阁连续多少轮未 request_decision 后自动放行（manual 模式忽略）
    #[serde(default = "default_approval_auto_retries")]
    pub auto_retries: u32,
}

fn default_approval_mode() -> ApprovalMode {
    ApprovalMode::Manual
}

fn default_approval_auto_retries() -> u32 {
    3
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            mode: default_approval_mode(),
            auto_retries: default_approval_auto_retries(),
        }
    }
}

/// Checkpoint 保存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// 自动 checkpoint 间隔（秒），0 = 不启用自动保存
    #[serde(default = "default_checkpoint_interval")]
    pub interval_secs: u64,
}

/// Checkpoint 默认值：0 = 禁用周期性自动保存（改用语义锚点）
fn default_checkpoint_interval() -> u64 {
    0
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
    100
}
fn default_document_heavy_iterations() -> usize {
    100
}

/// DeepSeek 1M token 窗口：接近上限再压缩。
/// `token_threshold` 仅计量 `context_messages`（cl100k），预留 system / tools / 输出余量。
fn default_compact_token_threshold() -> usize {
    750_000
}
fn default_keep_recent_count() -> usize {
    24
}
fn default_compact_mid_run_enabled() -> bool {
    false
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
fn default_delete_create_warning_count() -> u32 {
    2
}
fn default_test_stalemate_threshold() -> u32 {
    3
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            interval_secs: default_checkpoint_interval(),
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
            streaming: StreamingConfig::default(),
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
            token_threshold: default_compact_token_threshold(),
            keep_recent_count: default_keep_recent_count(),
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
            delete_create_warning_count: default_delete_create_warning_count(),
            test_stalemate_threshold: default_test_stalemate_threshold(),
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

    /// 加载配置，如果文件不存在则使用默认值。
    /// 同时尝试加载 `config.local.toml`（若存在）并合并覆盖默认值，
    /// 使开发者可以本地微调而不污染仓库中的 `config.toml`。
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        let path_ref = path.as_ref();
        let mut config = match Self::from_file(path_ref) {
            Ok(cfg) => {
                log_console!("[config] loaded config from {}", path_ref.display());
                cfg
            }
            Err(e) => {
                if path_ref.exists() {
                    log_console!("[config] config file parse error, using defaults: {}", e);
                } else {
                    log_console!(
                        "[config] config file not found, using defaults (create {} to customize)",
                        path_ref.display()
                    );
                }
                Self::default()
            }
        };

        // Try local override: config.local.toml in the same directory
        let local_path = path_ref.with_file_name("config.local.toml");
        if local_path.exists() {
            match Self::from_file(&local_path) {
                Ok(local) => {
                    log_console!(
                        "[config] loaded local override from {}",
                        local_path.display()
                    );
                    config.merge_from(local);
                }
                Err(e) => {
                    log_console!(
                        "[config] local config parse error, trying approval section only: {}",
                        e
                    );
                    if let Some(approval) = Self::load_approval_from_local(&local_path) {
                        config.approval = approval;
                    }
                }
            }
        }

        log_console!(
            "[debug] config values: api.timeout={}, api.max_retries={}, compact.token_threshold={}, compact.mid_run_compact={}",
            config.api.timeout_secs,
            config.api.max_retries,
            config.context_compaction.token_threshold,
            config.context_compaction.mid_run_compact,
        );
        config
    }

    /// 将另一个配置的非默认/非零字段合并到当前配置。
    /// 用于 `config.local.toml` 部分覆盖。
    fn merge_from(&mut self, other: RuntimeConfig) {
        // API config
        if other.api.timeout_secs != default_api_timeout() {
            self.api.timeout_secs = other.api.timeout_secs;
        }
        if other.api.max_retries != default_api_max_retries() {
            self.api.max_retries = other.api.max_retries;
        }
        if other.api.length_max_retries != default_length_max_retries() {
            self.api.length_max_retries = other.api.length_max_retries;
        }
        if other.api.max_tokens.write_file != default_write_file_tokens() {
            self.api.max_tokens.write_file = other.api.max_tokens.write_file;
        }
        if other.api.max_tokens.append_document != default_append_document_tokens() {
            self.api.max_tokens.append_document = other.api.max_tokens.append_document;
        }
        if other.api.max_tokens.readonly != default_readonly_tokens() {
            self.api.max_tokens.readonly = other.api.max_tokens.readonly;
        }
        if other.api.max_tokens.text_only != default_text_only_tokens() {
            self.api.max_tokens.text_only = other.api.max_tokens.text_only;
        }
        if other.api.reasoning.enabled != default_reasoning_enabled() {
            self.api.reasoning.enabled = other.api.reasoning.enabled;
        }
        if other.api.reasoning.budget_tokens != default_reasoning_budget() {
            self.api.reasoning.budget_tokens = other.api.reasoning.budget_tokens;
        }
        if other.api.streaming.enabled {
            self.api.streaming.enabled = other.api.streaming.enabled;
        }

        // Tool iterations
        if other.tool_iterations.readonly != default_readonly_iterations() {
            self.tool_iterations.readonly = other.tool_iterations.readonly;
        }
        if other.tool_iterations.write_heavy != default_write_heavy_iterations() {
            self.tool_iterations.write_heavy = other.tool_iterations.write_heavy;
        }
        if other.tool_iterations.document_heavy != default_document_heavy_iterations() {
            self.tool_iterations.document_heavy = other.tool_iterations.document_heavy;
        }

        // Context compaction
        if other.context_compaction.token_threshold != default_compact_token_threshold() {
            self.context_compaction.token_threshold = other.context_compaction.token_threshold;
        }
        if other.context_compaction.keep_recent_count != default_keep_recent_count() {
            self.context_compaction.keep_recent_count = other.context_compaction.keep_recent_count;
        }
        if other.context_compaction.mid_run_compact != default_compact_mid_run_enabled() {
            self.context_compaction.mid_run_compact = other.context_compaction.mid_run_compact;
        }

        // Actor
        if other.actor.max_exec_iterations != default_max_exec_iterations() {
            self.actor.max_exec_iterations = other.actor.max_exec_iterations;
        }
        if other.actor.max_plan_iterations != default_max_plan_iterations() {
            self.actor.max_plan_iterations = other.actor.max_plan_iterations;
        }

        // Watchdog
        if other.watchdog.max_consecutive_errors != default_max_consecutive_errors() {
            self.watchdog.max_consecutive_errors = other.watchdog.max_consecutive_errors;
        }
        if other.watchdog.same_tool_warning_count != default_same_tool_warning_count() {
            self.watchdog.same_tool_warning_count = other.watchdog.same_tool_warning_count;
        }
        if other.watchdog.read_without_write_warning != default_read_without_write_warning() {
            self.watchdog.read_without_write_warning = other.watchdog.read_without_write_warning;
        }
        if other.watchdog.delete_create_warning_count != default_delete_create_warning_count() {
            self.watchdog.delete_create_warning_count = other.watchdog.delete_create_warning_count;
        }
        if other.watchdog.test_stalemate_threshold != default_test_stalemate_threshold() {
            self.watchdog.test_stalemate_threshold = other.watchdog.test_stalemate_threshold;
        }

        // Checkpoint
        if other.checkpoint.interval_secs != default_checkpoint_interval() {
            self.checkpoint.interval_secs = other.checkpoint.interval_secs;
        }

        // Approval
        if other.approval.mode != default_approval_mode() {
            self.approval.mode = other.approval.mode;
        }
        if other.approval.auto_retries != default_approval_auto_retries() {
            self.approval.auto_retries = other.approval.auto_retries;
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

    /// Resolve `config.toml` path (CWD, then parent directory).
    pub fn config_toml_path() -> std::path::PathBuf {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let candidate = cwd.join("config.toml");
        if candidate.exists() {
            return candidate;
        }
        if let Some(parent) = cwd.parent() {
            let parent_candidate = parent.join("config.toml");
            if parent_candidate.exists() {
                return parent_candidate;
            }
        }
        candidate
    }

    /// Load only the `[approval]` section from `config.local.toml`.
    pub fn load_approval_from_local(path: &Path) -> Option<ApprovalConfig> {
        let content = std::fs::read_to_string(path).ok()?;
        let doc: toml::Value = toml::from_str(&content).ok()?;
        let approval = doc.get("approval")?;
        approval.clone().try_into().ok()
    }

    /// Merge `[approval]` into `config.local.toml` (create file if missing).
    pub fn save_approval_to_local(
        base_config_path: &Path,
        approval: &ApprovalConfig,
    ) -> anyhow::Result<()> {
        let local_path = base_config_path.with_file_name("config.local.toml");
        let mut doc: toml::Value = if local_path.exists() {
            let content = std::fs::read_to_string(&local_path)?;
            toml::from_str(&content).unwrap_or(toml::Value::Table(toml::map::Map::new()))
        } else {
            toml::Value::Table(toml::map::Map::new())
        };
        if let toml::Value::Table(ref mut table) = doc {
            table.insert(
                "approval".to_string(),
                toml::Value::try_from(approval.clone())?,
            );
        }
        std::fs::write(&local_path, toml::to_string_pretty(&doc)?)?;
        Ok(())
    }

    /// 为指定角色解析上下文压缩阈值。
    /// 优先级：`context_config.json` 字段覆盖 > 部门内置推荐值 > `[context_compaction]` 全局默认。
    pub fn resolve_compact_thresholds(
        &self,
        role_name: &str,
        role_config: Option<&RoleContextConfig>,
    ) -> CompactThresholds {
        let base = default_compact_thresholds_for_role(role_name).unwrap_or(CompactThresholds {
            token_threshold: self.context_compaction.token_threshold,
            keep_recent_count: self.context_compaction.keep_recent_count,
            mid_run_compact: self.context_compaction.mid_run_compact,
        });

        let ov = role_config;
        CompactThresholds {
            token_threshold: ov
                .and_then(|o| o.token_threshold)
                .unwrap_or(base.token_threshold),
            keep_recent_count: ov
                .and_then(|o| o.keep_recent_count)
                .unwrap_or(base.keep_recent_count),
            mid_run_compact: ov
                .and_then(|o| o.mid_run_compact)
                .unwrap_or(base.mid_run_compact),
        }
    }
}

/// 接近 1M 窗口上限再压缩（各部门统一策略，可用 `context_config.json` 覆盖）。
fn near_window_compact_thresholds() -> CompactThresholds {
    CompactThresholds {
        token_threshold: default_compact_token_threshold(),
        keep_recent_count: default_keep_recent_count(),
        mid_run_compact: default_compact_mid_run_enabled(),
    }
}

/// 各部门内置上下文压缩推荐值（中文角色名，与 `Role::name()` 一致）。
/// 无 `context_config.json` 覆盖时生效。
pub fn default_compact_thresholds_for_role(role_name: &str) -> Option<CompactThresholds> {
    match role_name {
        "工部" | "刑部" | "中书令" | "吏部" | "内阁" | "兵部" | "门下侍中" | "尚书令" | "礼部" => {
            Some(near_window_compact_thresholds())
        }
        _ => None,
    }
}
