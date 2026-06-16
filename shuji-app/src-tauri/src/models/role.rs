use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Zhongshuling,   // Chief Architect (design)
    MenxiaShizhong, // Chief Reviewer (review)
    Neige,          // Grand Secretariat (orchestrator)
    Shangshuling,   // Chief Executive (execution dispatch)
    LiBuShangshu,   // Ministry of Personnel (detailed design)
    LiBuRShangshu,  // Ministry of Rites (standards check)
    BingbuShangshu, // Ministry of War (tests + contracts)
    XingbuShangshu, // Ministry of Justice (test verification)
    GongbuShangshu, // Ministry of Works (coding implementation)
}

impl Role {
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

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "Zhongshuling" | "中书令" => Some(Role::Zhongshuling),
            "Menxiashizhong" | "门下侍中" => Some(Role::MenxiaShizhong),
            "Neige" | "内阁" => Some(Role::Neige),
            "Shangshuling" | "尚书令" => Some(Role::Shangshuling),
            "Libushangshu" | "吏部" => Some(Role::LiBuShangshu),
            "Liburshangshu" | "礼部" => Some(Role::LiBuRShangshu),
            "Bingbushangshu" | "兵部" => Some(Role::BingbuShangshu),
            "Xingbushangshu" | "刑部" => Some(Role::XingbuShangshu),
            "Gongbushangshu" | "工部" => Some(Role::GongbuShangshu),
            _ => None,
        }
    }

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
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}
