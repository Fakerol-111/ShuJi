//! 角色枚举定义。
//!
//! 九品中正制 → 9 个部门角色，映射到三省六部制的组织结构。
//! `Role` 是全局唯一身份标识——用于 mailbox 路由、API key 查找、
//! cancel_map 索引、日志写入路径等。

use serde::{Deserialize, Serialize};
use std::fmt;

/// 9 部门角色枚举，全局唯一身份标识。
///
/// **组织架构**:
/// ```text
/// 内阁 (Neige) ──── 编排中枢，皇帝的直接接口
///   ├─ 中书令 (Zhongshuling)    — 三省之一：总设计
///   ├─ 门下侍中 (MenxiaShizhong) — 三省之一：总审查
///   └─ 尚书令 (Shangshuling)    — 三省之一：执行调度
///       ├─ 吏部 (LiBu)    — 六部之一：详细设计
///       ├─ 兵部 (Bingbu)  — 六部之一：测试+接口契约
///       ├─ 工部 (Gongbu)  — 六部之一：编码
///       ├─ 刑部 (Xingbu)  — 六部之一：测试验证
///       └─ 礼部 (LiBuR)   — 六部之一：规范检查+审计
/// ```
///
/// 序列化为字符串时使用 `Role::name()` 的返回值（英文），
/// 反序列化时 `from_name()` 支持中英双语（"内阁"/"Neige" 均可）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Zhongshuling,   // 中书令 — Chief Architect: 方案设计、阶段规划
    MenxiaShizhong, // 门下侍中 — Chief Reviewer: 审查设计提案
    Neige,          // 内阁 — Grand Secretariat: 编排中枢、用户接口
    Shangshuling,   // 尚书令 — Chief Executive: 执行调度、向六部分发任务
    LiBuShangshu,   // 吏部尚书 — Minister of Personnel: 详细设计、任务分解
    LiBuRShangshu,  // 礼部尚书 — Minister of Rites: 规范合规检查、审计
    BingbuShangshu, // 兵部尚书 — Minister of War: 编写测试、生成接口契约
    XingbuShangshu, // 刑部尚书 — Minister of Justice: 执行测试、验证代码质量
    GongbuShangshu, // 工部尚书 — Minister of Works: TDD 编码实现
}

impl Role {
    /// 返回角色的英文名（PascalCase），用于：
    /// - `config.for_role(name)` 查找 API key/url/model
    /// - `role.name()` 字符串比较
    /// - 日志和审计中的角色标识
    pub fn name(&self) -> &str {
        match self {
            Role::Zhongshuling => "Zhongshuling",
            Role::MenxiaShizhong => "Menxiashizhong",
            Role::Neige => "Neige",
            Role::Shangshuling => "Shangshuling",
            Role::LiBuShangshu => "Libushangshu",
            Role::LiBuRShangshu => "Liburshangshu",
            Role::BingbuShangshu => "Bingbushangshu",
            Role::XingbuShangshu => "Xingbushangshu",
            Role::GongbuShangshu => "Gongbushangshu",
        }
    }

    /// 从字符串解析角色名（中英双语支持）。
    ///
    /// 为什么需要双语？LLM 输出可能混用中英文角色名
    /// （如 `<route_to to="工部">` 和 `route_to("Gongbushangshu")` 都可能出现），
    /// 此方法统一解析。不区分大小写（通过 match 直连）。
    /// 从字符串解析角色名（中英双语支持）。
    ///
    /// 为什么需要双语？LLM 输出可能混用中英文角色名
    /// （如 `<route_to to="工部">` 和 `route_to("Gongbushangshu")` 都可能出现），
    /// 此方法统一解析。不区分大小写（通过 to_lowercase 处理）。
    pub fn from_name(s: &str) -> Option<Self> {
        let lower = s.to_lowercase();
        let trimmed = lower.trim();
        let matched = match trimmed {
            "zhongshuling" | "中书令" | "chief architect" | "architect" | "设计" | "overall" => {
                Some(Role::Zhongshuling)
            }

            "menxiashizhong" | "门下侍中" | "gate reviewer" | "reviewer" | "review" | "审核"
            | "审查" => Some(Role::MenxiaShizhong),

            "neige" | "内阁" | "cabinet" | "grand secretariat" => Some(Role::Neige),

            "shangshuling" | "尚书令" | "chief executor" | "executor" | "执行" | "dispatch"
            | "调度" => Some(Role::Shangshuling),

            "libushangshu" | "吏部" | "吏部尚书" | "personnel" | "人事" => {
                Some(Role::LiBuShangshu)
            }

            "liburshangshu" | "礼部" | "礼部尚书" | "rites" | "礼仪" => {
                Some(Role::LiBuRShangshu)
            }

            "bingbushangshu" | "兵部" | "兵部尚书" | "war" | "测试" | "contract" => {
                Some(Role::BingbuShangshu)
            }

            "xingbushangshu" | "刑部" | "刑部尚书" | "justice" | "刑法" | "validate" => {
                Some(Role::XingbuShangshu)
            }

            "gongbushangshu" | "工部" | "工部尚书" | "works" | "ministry of works" | "编码"
            | "实现" | "engineering" => Some(Role::GongbuShangshu),

            _ => None,
        };
        if matched.is_some() {
            return matched;
        }
        None
    }

    /// 返回角色的英文系统提示（system prompt 片段）。
    ///
    /// 这只是一句角色定义——完整的 system prompt 由 `base_prompt (prompt.md)`
    /// 提供，包含工具引用、部门表、协作规则等。此方法用于 prompt 组装时的
    /// 角色身份注入。
    pub fn system_prompt(&self) -> &str {
        match self {
            Role::Zhongshuling => "You are the Chief Architect (中书令), responsible for overall design, phase planning, and phase design.",
            Role::MenxiaShizhong => "You are the Chief Reviewer (门下侍中), responsible for reviewing design proposals.",
            Role::Neige => "You are the Grand Secretariat (内阁), responsible for direct communication with the user, consolidating reports from all departments, and presenting them to the user.",
            Role::Shangshuling => "You are the Chief Executive (尚书令), responsible for execution management, dispatching departments to execute approved design proposals.",
            Role::LiBuShangshu => "You are the Minister of Personnel (吏部尚书), responsible for detailed design and task decomposition.",
            Role::LiBuRShangshu => "You are the Minister of Rites (礼部尚书), responsible for standards and compliance checks.",
            Role::BingbuShangshu => "You are the Minister of War (兵部尚书), responsible for writing tests and producing interface contracts.",
            Role::XingbuShangshu => "You are the Minister of Justice (刑部尚书), responsible for executing tests and verifying code quality.",
            Role::GongbuShangshu => "You are the Minister of Works (工部尚书), responsible for coding implementation.",
        }
    }

    /// 返回角色的中文显示名（如 "内阁"、"中书令"），用于前端 UI 展示。
    pub fn chinese_name(&self) -> &'static str {
        match self {
            Role::Zhongshuling => "中书令",
            Role::MenxiaShizhong => "门下侍中",
            Role::Neige => "内阁",
            Role::Shangshuling => "尚书令",
            Role::LiBuShangshu => "吏部尚书",
            Role::LiBuRShangshu => "礼部尚书",
            Role::BingbuShangshu => "兵部尚书",
            Role::XingbuShangshu => "刑部尚书",
            Role::GongbuShangshu => "工部尚书",
        }
    }

    /// 返回角色的组合显示名（如 "内阁 (Neige)"），用于日志和审计中
    /// 同时展示中英文名称，方便阅读和检索。
    pub fn display_name(&self) -> String {
        format!("{} ({})", self.chinese_name(), self.name())
    }

    /// 返回所有角色的列表，按组织架构顺序排列。
    /// 用于需要遍历所有角色的场景（如初始化 actor 系统、生成配置模板等）。
    pub fn all() -> &'static [Role] {
        &[
            Role::Neige,
            Role::Zhongshuling,
            Role::MenxiaShizhong,
            Role::Shangshuling,
            Role::LiBuShangshu,
            Role::BingbuShangshu,
            Role::GongbuShangshu,
            Role::XingbuShangshu,
            Role::LiBuRShangshu,
        ]
    }
}

/// Display 实现等同于 `name()`，用于 `println!/format!("{}", role)`。
impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_name_returns_correct_values() {
        assert_eq!(Role::Neige.chinese_name(), "内阁");
        assert_eq!(Role::Zhongshuling.chinese_name(), "中书令");
        assert_eq!(Role::MenxiaShizhong.chinese_name(), "门下侍中");
        assert_eq!(Role::Shangshuling.chinese_name(), "尚书令");
        assert_eq!(Role::LiBuShangshu.chinese_name(), "吏部尚书");
        assert_eq!(Role::LiBuRShangshu.chinese_name(), "礼部尚书");
        assert_eq!(Role::BingbuShangshu.chinese_name(), "兵部尚书");
        assert_eq!(Role::XingbuShangshu.chinese_name(), "刑部尚书");
        assert_eq!(Role::GongbuShangshu.chinese_name(), "工部尚书");
    }

    #[test]
    fn display_name_combines_chinese_and_english() {
        assert_eq!(Role::Neige.display_name(), "内阁 (Neige)");
        assert_eq!(Role::Zhongshuling.display_name(), "中书令 (Zhongshuling)");
        assert_eq!(
            Role::LiBuRShangshu.display_name(),
            "礼部尚书 (Liburshangshu)"
        );
    }

    #[test]
    fn from_name_handles_chinese_aliases() {
        assert_eq!(Role::from_name("内阁"), Some(Role::Neige));
        assert_eq!(Role::from_name("中书令"), Some(Role::Zhongshuling));
        assert_eq!(Role::from_name("门下侍中"), Some(Role::MenxiaShizhong));
        assert_eq!(Role::from_name("尚书令"), Some(Role::Shangshuling));
        assert_eq!(Role::from_name("吏部"), Some(Role::LiBuShangshu));
        assert_eq!(Role::from_name("礼部"), Some(Role::LiBuRShangshu));
        assert_eq!(Role::from_name("兵部"), Some(Role::BingbuShangshu));
        assert_eq!(Role::from_name("刑部"), Some(Role::XingbuShangshu));
        assert_eq!(Role::from_name("工部"), Some(Role::GongbuShangshu));
    }

    #[test]
    fn from_name_case_insensitive() {
        assert_eq!(Role::from_name("neige"), Some(Role::Neige));
        assert_eq!(Role::from_name("NEIGE"), Some(Role::Neige));
        assert_eq!(Role::from_name("Neige"), Some(Role::Neige));
    }

    #[test]
    fn from_name_rejects_unknown() {
        assert_eq!(Role::from_name("unknown"), None);
        assert_eq!(Role::from_name(""), None);
    }

    #[test]
    fn all_returns_nine_roles_in_org_order() {
        let roles = Role::all();
        assert_eq!(roles.len(), 9);
        assert_eq!(roles[0], Role::Neige);
        // Neige first, then three 省, then six 部
        assert!(roles.contains(&Role::Zhongshuling));
        assert!(roles.contains(&Role::GongbuShangshu));
    }

    #[test]
    fn name_and_chinese_name_are_consistent() {
        // Every role should have a non-empty name and chinese_name
        for role in Role::all() {
            assert!(!role.name().is_empty(), "{:?} has empty name", role);
            assert!(
                !role.chinese_name().is_empty(),
                "{:?} has empty chinese_name",
                role
            );
        }
    }
}
