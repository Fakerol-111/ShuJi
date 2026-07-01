//! Reasoning/thinking mode configuration and resolution.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 推理/思考强度等级
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    None,
    Low,
    #[default]
    Medium,
    High,
}

impl std::fmt::Display for ReasoningEffort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

/// 单个角色的推理配置覆盖（可选字段，None 表示继承全局）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleReasoningConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub effort: Option<ReasoningEffort>,
    #[serde(default)]
    pub budget_tokens: Option<u32>,
}

/// 推理/思考模式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// 是否启用思考模式
    #[serde(default = "default_reasoning_enabled")]
    pub enabled: bool,
    /// 思考强度：none / low / medium / high
    #[serde(default = "default_reasoning_effort")]
    pub effort: ReasoningEffort,
    /// 思考预算 token 数（0 = 使用模型默认值；仅 Anthropic API 有效）
    #[serde(default = "default_reasoning_budget")]
    pub budget_tokens: u32,
    /// 按角色覆盖推理策略
    #[serde(default)]
    pub roles: HashMap<String, RoleReasoningConfig>,
}

impl Default for ReasoningConfig {
    fn default() -> Self {
        Self {
            enabled: default_reasoning_enabled(),
            effort: default_reasoning_effort(),
            budget_tokens: default_reasoning_budget(),
            roles: HashMap::new(),
        }
    }
}

/// 推理阶段（用于工部等需要按阶段切换策略的角色）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningPhase {
    Default,
    Planning,
    Execution,
    WrapUp,
}

/// 已解析的推理策略（最终生效的值）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedReasoningPolicy {
    pub enabled: bool,
    pub effort: ReasoningEffort,
    pub budget_tokens: u32,
}

impl ResolvedReasoningPolicy {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            effort: ReasoningEffort::None,
            budget_tokens: 0,
        }
    }
}

// ── Default value functions ─────────────────────────────────

pub(crate) fn default_reasoning_enabled() -> bool {
    true
}
pub(crate) fn default_reasoning_effort() -> ReasoningEffort {
    ReasoningEffort::Medium
}
pub(crate) fn default_reasoning_budget() -> u32 {
    0
}

// ── Resolution helpers ──────────────────────────────────────

/// 工部阶段覆盖策略
pub(crate) fn resolve_phase_override(
    role: &str,
    phase: ReasoningPhase,
) -> Option<ResolvedReasoningPolicy> {
    if role != "工部" {
        return None;
    }
    match phase {
        ReasoningPhase::Planning => Some(ResolvedReasoningPolicy {
            enabled: true,
            effort: ReasoningEffort::High,
            budget_tokens: 0,
        }),
        ReasoningPhase::Execution => Some(ResolvedReasoningPolicy::disabled()),
        _ => None,
    }
}

/// 内置角色默认推理策略
pub(crate) fn builtin_role_reasoning(role: &str) -> Option<ResolvedReasoningPolicy> {
    let effort = match role {
        "内阁" | "Zhongshuling" | "中书令" => ReasoningEffort::High,
        "门下侍中" | "尚书令" | "吏部尚书" | "兵部尚书" | "工部尚书" => {
            ReasoningEffort::Medium
        }
        "刑部尚书" | "礼部尚书" => ReasoningEffort::Low,
        _ => return None,
    };
    Some(ResolvedReasoningPolicy {
        enabled: effort != ReasoningEffort::None,
        effort,
        budget_tokens: 0,
    })
}
