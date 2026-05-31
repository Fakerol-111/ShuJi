use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Zhongshuling,   // 中书令（设计）
    MenxiaShizhong, // 门下侍中（审查）
    Neige,
    Shangshuling,
    LiBuShangshu,   // 吏部
    LiBuRShangshu,  // 礼部尚书 (Ministry of Rites; 拼音: Lǐ Bù Shàng Shū)
    BingbuShangshu, // 兵部
    XingbuShangshu, // 刑部
    GongbuShangshu, // 工部
    Zhisi,          // 制司
}

impl Role {
    pub fn name(&self) -> &str {
        match self {
            Role::Zhongshuling => "中书令",
            Role::MenxiaShizhong => "门下侍中",
            Role::Neige => "内阁",
            Role::Shangshuling => "尚书令",
            Role::LiBuShangshu => "吏部",
            Role::LiBuRShangshu => "礼部",
            Role::BingbuShangshu => "兵部",
            Role::XingbuShangshu => "刑部",
            Role::GongbuShangshu => "工部",
            Role::Zhisi => "制司",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "中书令" => Some(Role::Zhongshuling),
            "门下侍中" => Some(Role::MenxiaShizhong),
            "内阁" => Some(Role::Neige),
            "尚书令" => Some(Role::Shangshuling),
            "吏部" => Some(Role::LiBuShangshu),
            "礼部" => Some(Role::LiBuRShangshu),
            "兵部" => Some(Role::BingbuShangshu),
            "刑部" => Some(Role::XingbuShangshu),
            "工部" => Some(Role::GongbuShangshu),
            "制司" => Some(Role::Zhisi),
            _ => None,
        }
    }

    pub fn system_prompt(&self) -> &str {
        match self {
            Role::Zhongshuling => "你是中书令，负责整体设计、阶段规划和阶段设计。",
            Role::MenxiaShizhong => "你是门下侍中，负责审查设计方案。",
            Role::Neige => "你是内阁，负责与用户直接沟通，整理各部门的汇报并呈现给用户。",
            Role::Shangshuling => "你是尚书令，负责执行管理，调度各部门执行已批准的设计方案。",
            Role::LiBuShangshu => "你是吏部尚书，负责详细设计和任务拆解。",
            Role::LiBuRShangshu => "你是礼部尚书，负责规范检查。",
            Role::BingbuShangshu => "你是兵部尚书，负责编写测试并产出接口契约。",
            Role::XingbuShangshu => "你是刑部尚书，负责执行测试验证代码质量。",
            Role::GongbuShangshu => "你是工部尚书，负责编码实现。",
            Role::Zhisi => "你是制司，负责权限管理，审批权限申请并对越权行为进行审计。",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}
