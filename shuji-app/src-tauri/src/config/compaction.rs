//! Context compaction threshold configuration and resolution.

use serde::{Deserialize, Serialize};

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

/// 接近 1M 窗口上限再压缩（各部门统一策略，可用 `context_config.json` 覆盖）。
fn near_window_compact_thresholds() -> CompactThresholds {
    CompactThresholds {
        token_threshold: super::types::default_compact_token_threshold(),
        keep_recent_count: super::types::default_keep_recent_count(),
        mid_run_compact: super::types::default_compact_mid_run_enabled(),
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

/// 为指定角色解析上下文压缩阈值。
/// 优先级：`context_config.json` 字段覆盖 > 部门内置推荐值 > `[context_compaction]` 全局默认。
pub(crate) fn resolve(
    global_token_threshold: usize,
    global_keep_recent_count: usize,
    global_mid_run_compact: bool,
    role_name: &str,
    role_config: Option<&RoleContextConfig>,
) -> CompactThresholds {
    let base = default_compact_thresholds_for_role(role_name).unwrap_or(CompactThresholds {
        token_threshold: global_token_threshold,
        keep_recent_count: global_keep_recent_count,
        mid_run_compact: global_mid_run_compact,
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
