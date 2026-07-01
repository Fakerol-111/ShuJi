//! Approval mode configuration.

use serde::{Deserialize, Serialize};

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

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            mode: default_approval_mode(),
            auto_retries: default_approval_auto_retries(),
        }
    }
}

pub(crate) fn default_approval_mode() -> ApprovalMode {
    ApprovalMode::Manual
}

pub(crate) fn default_approval_auto_retries() -> u32 {
    3
}
