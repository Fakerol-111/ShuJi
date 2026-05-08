use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Zhongshu,
    Menxia,
    Neige,
    Shangshu,
    LiBuP,   // 吏部
    Hubu,    // 户部
    LiBuR,   // 礼部
    Bingbu,  // 兵部
    Xingbu,  // 刑部
    Gongbu,  // 工部
    Zhisi,   // 制司
}

impl Role {
    pub fn name(&self) -> &str {
        match self {
            Role::Zhongshu => "中书省",
            Role::Menxia => "门下省",
            Role::Neige => "内阁",
            Role::Shangshu => "尚书省",
            Role::LiBuP => "吏部",
            Role::Hubu => "户部",
            Role::LiBuR => "礼部",
            Role::Bingbu => "兵部",
            Role::Xingbu => "刑部",
            Role::Gongbu => "工部",
            Role::Zhisi => "制司",
        }
    }

    pub fn context_file(&self) -> String {
        match self {
            Role::Zhongshu => "zhongshu.json",
            Role::Menxia => "menxia.json",
            Role::Neige => "neige.json",
            Role::Shangshu => "shangshu.json",
            Role::LiBuP => "libu_p.json",
            Role::Hubu => "hubu.json",
            Role::LiBuR => "libu_r.json",
            Role::Bingbu => "bingbu.json",
            Role::Xingbu => "xingbu.json",
            Role::Gongbu => "gongbu.json",
            Role::Zhisi => "zhisi.json",
        }
        .to_string()
    }

    pub fn system_prompt(&self) -> &str {
        match self {
            Role::Zhongshu => "你是中书省，负责需求分析和方案设计。你的任务是根据皇帝的目标产出完整的设计方案。",
            Role::Menxia => "你是门下省，负责依据皇明祖训审查设计方案。你的任务是对方案进行严格审查，给出通过或驳回的意见。",
            Role::Neige => "你是内阁，皇帝的秘书班子。你的任务是将门下省通过的方案整理为奏折呈送皇帝批阅。",
            Role::Shangshu => "你是尚书省，负责执行管理。你的任务是调度六部执行已批准的设计方案。",
            Role::LiBuP => "你是吏部，负责任务拆解和分配。你的任务是将设计方案拆解为可执行的任务清单。",
            Role::Hubu => "你是户部，负责日志管理和记录。你的任务是记录各部门的执行日志。",
            Role::LiBuR => "你是礼部，负责规范检查。你的任务是检查代码和文档是否符合规范。",
            Role::Bingbu => "你是兵部，负责测试和安全。你的任务是编写测试用例并执行测试。",
            Role::Xingbu => "你是刑部，负责异常处理和合规检查。你的任务是对代码进行边界检查和异常路径分析。",
            Role::Gongbu => "你是工部，负责编码实现。你的任务是根据设计方案编写代码。",
            Role::Zhisi => "你是制司，负责权限管理。你的任务是审批权限申请，对越权行为进行审计。",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}
