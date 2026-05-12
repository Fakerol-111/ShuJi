#![allow(dead_code)]
use rand::Rng;
use crate::models::document::{Document, DocumentType};
use crate::models::role::Role;
use crate::agent::r#trait::{Agent, AgentInput, AgentOutput};

/// Mock agent — returns preset content, can randomly reject or inject issues.
pub struct MockAgent {
    role: Role,
    /// Overall reject probability (0.0 – 1.0) for 门下省
    reject_rate: f64,
    /// Block probability per execution step (0.0 – 1.0)
    block_rate: f64,
}

impl MockAgent {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            reject_rate: 0.3,
            block_rate: 0.15,
        }
    }

    pub fn with_rates(mut self, reject_rate: f64, block_rate: f64) -> Self {
        self.reject_rate = reject_rate;
        self.block_rate = block_rate;
        self
    }
}

#[async_trait::async_trait]
impl Agent for MockAgent {
    fn role(&self) -> Role {
        self.role
    }

    async fn execute(&self, input: &AgentInput) -> anyhow::Result<AgentOutput> {
        match self.role {
            Role::MenxiaShizhong => Ok(self.mock_mensheng(input)),
            Role::Neige => Ok(self.mock_neige(input)),
            Role::Shangshuling => Ok(self.mock_shangshu(input)),
            Role::LiBuShangshu => Ok(self.mock_libu_p(input)),
            Role::BingbuShangshu => Ok(self.mock_bingbu(input)),
            Role::GongbuShangshu => Ok(self.mock_gongbu(input)),
            Role::XingbuShangshu => Ok(self.mock_xingbu(input)),
            Role::LiBuRShangshu => Ok(self.mock_libu_r(input)),
            Role::Zhisi => Ok(self.mock_zhisi(input)),
            _ => Ok(AgentOutput::new("(已废弃)".to_string())),
        }
    }

    
}

impl MockAgent {
    fn extract_reject_count(&self, content: &str) -> u32 {
        // Look for pattern like "驳回(2)" or "驳回次数:2"
        if let Some(start) = content.find("驳回(") {
            let num_part = &content[start + 3..];
            if let Some(end) = num_part.find(')') {
                if let Ok(n) = num_part[..end].parse::<u32>() {
                    return n;
                }
            }
        }
        1
    }

    fn random_reject(&self) -> bool {
        let mut rng = rand::thread_rng();
        rng.gen_bool(self.reject_rate)
    }

    fn random_block(&self) -> Option<bool> {
        let mut rng = rand::thread_rng();
        if rng.gen_bool(self.block_rate) {
            Some(rng.gen_bool(0.3)) // 30% blocking, 70% minor
        } else {
            None
        }
    }

    fn mock_zhongshu(&self, input: &AgentInput) -> AgentOutput {
        let is_overall = input.task_description.contains("整体");
        let title = if is_overall { "整体方案设计" } else { "阶段详细设计" };
        let content = format!(
            r#"## {title}

**任务：** {}

## 架构建议

1. **技术栈**：Python + FastAPI + SQLite + Bootstrap 5
2. **模块划分**：
   - 商品管理模块
   - 采购管理模块
   - 销售管理模块
   - 库存管理模块
   - 报表模块
3. **数据流**：略

## 阶段规划

- 阶段一：基础 CRUD + 数据库
- 阶段二：采购销售流程
- 阶段三：报表和优化

---
<route to="门下省" priority="slow" subject="方案设计完成，请审查" />"#,
            input.task_description
        );
        let doc = Document {
            title: title.to_string(),
            content: content.clone(),
            doc_type: DocumentType::Design,
            path: None,
        };
        AgentOutput::new(content).with_document(doc)
    }

    fn mock_mensheng(&self, input: &AgentInput) -> AgentOutput {
        if self.random_reject() {
            let content = format!(
                r#"## 审查报告：驳回

**审查对象：** {}

**[驳回] 理由如下：**

1. 方案中未明确 API 接口定义，无法评估实现可行性
2. 缺少数据模型定义，存在设计模糊地带
3. 未见异常处理方案

---
<route to="内阁" priority="slow" subject="审查驳回通知" />
<route to="中书省" priority="slow" subject="方案需修改" />"#,
                input.task_description
            );
            AgentOutput::new(content)
        } else {
            let content = format!(
                r#"## 审查报告：通过

**审查对象：** {}

**审查结论：** 方案符合规范，准予通过。

**审查要点：**
- ✅ 架构设计合理
- ✅ 模块划分清晰
- ✅ 阶段规划可执行

---
<route to="内阁" priority="fast" subject="审查通过，请呈报皇帝" />"#,
                input.task_description
            );
            let doc = Document {
                title: "审查报告".to_string(),
                content: content.clone(),
                doc_type: DocumentType::Review,
                path: None,
            };
            AgentOutput::new(content).with_document(doc)
        }
    }

    fn mock_neige(&self, input: &AgentInput) -> AgentOutput {
        // Check if this is a reply (A/B/C) or an initial request
        let is_decision = input.task_description.contains("A. 同意")
            || input.task_description.contains("皇帝旨意");

        if is_decision || input.task_description.contains("项目") || input.task_description.contains("系统") || input.task_description.contains("请设计") {
            // Route to Zhongshu for design
            let content = format!(
                r#"收到。现将任务分配给中书省进行方案设计。

<route to="中书省" priority="slow" subject="请设计方案" />"#,
            );
            AgentOutput::new(content)
        } else {
            // General chat — respond directly
            let content = format!(
                r#"关于「{}」：

已收到您的想法。如果这是一个项目需求，我将安排中书省进行方案设计。
如果只是想讨论，我随时可以陪您聊。

---
<route to="中书省" priority="slow" subject="请设计方案" />"#,
                input.task_description
            );
            AgentOutput::new(content)
        }
    }

    fn mock_shangshu(&self, input: &AgentInput) -> AgentOutput {
        let content = format!(
            r#"## 尚书省执行令

**任务：** {}

**调度计划：**
1. 吏部 → 拆解任务
2. 兵部 → 编写测试
3. 工部 → 编码实现
4. 刑部 → 异常检查
5. 礼部 → 规范检查
6. 户部 → 记录归档"#,
            input.task_description
        );
        AgentOutput::new(content)
    }

    fn mock_libu_p(&self, input: &AgentInput) -> AgentOutput {
        let content = format!(
            r#"## 任务清单

**方案：** {}

| 任务 | 负责 | 预估 |
|------|------|------|
| 创建数据库模型 | 工部 | 2h |
| 实现商品 CRUD | 工部 | 3h |
| 实现采购流程 | 工部 | 4h |
| 编写测试 | 兵部 | 2h |
| 异常处理 | 刑部 | 1h |
| 规范检查 | 礼部 | 1h |"#,
            input.task_description
        );
        let doc = Document {
            title: "任务清单".to_string(),
            content: content.clone(),
            doc_type: DocumentType::TaskBreakdown,
            path: None,
        };
        AgentOutput::new(content).with_document(doc)
    }

    fn mock_bingbu(&self, _input: &AgentInput) -> AgentOutput {
        // Random chance of finding a blocking issue
        if let Some(is_blocking) = self.random_block() {
            let tag = if is_blocking { "[阻塞]" } else { "[需皇帝决策]" };
            let severity = if is_blocking { "严重缺陷" } else { "一般问题" };
            return AgentOutput::new(format!(
                "## 测试报告\n\n**{} 发现{}**\n\n**问题描述：** 在测试过程中发现数据库并发写入时存在数据一致性问题。\n\n**建议：** {}\n\n**详情：**\n- 场景：多用户同时写入\n- 预期：数据一致\n- 实际：出现脏读",
                tag, severity,
                if is_blocking { "阻塞执行，需要退回设计阶段修改" }
                else { "建议修改但可不阻塞执行" }
            ));
        }
        AgentOutput::new(
            r#"## 测试报告

**测试结果：** 全部通过 ✅

| 测试用例 | 结果 |
|----------|------|
| 创建商品 | 通过 |
| 查询商品列表 | 通过 |
| 库存调整 | 通过 |
| 采购单创建 | 通过 |"#.to_string()
        )
    }

    fn mock_gongbu(&self, _input: &AgentInput) -> AgentOutput {
        AgentOutput::new(
            r#"## 编码实现报告

**已完成文件：**
- `models/product.py`
- `routes/product_routes.py`
- `templates/product_list.html`

**状态：** 实现完成 ✅"#.to_string()
        )
    }

    fn mock_xingbu(&self, _input: &AgentInput) -> AgentOutput {
        AgentOutput::new(
            r#"## 刑部检查报告

**检查项：**
- ✅ 输入验证：所有用户输入已做校验
- ✅ 边界条件：数值边界已处理
- ✅ 异常路径：数据库操作有 try-catch
- ✅ 合规性：符合项目规范

**结论：** 通过 ✅"#.to_string()
        )
    }

    fn mock_libu_r(&self, _input: &AgentInput) -> AgentOutput {
        AgentOutput::new(
            r#"## 礼部检查报告

**检查项：**
- ✅ 编码规范：符合 PEP8
- ✅ 命名规范：命名清晰
- ✅ 文档格式：符合要求
- ✅ 注释规范：关键逻辑有注释

**结论：** 通过 ✅"#.to_string()
        )
    }

    fn mock_zhisi(&self, _input: &AgentInput) -> AgentOutput {
        AgentOutput::new(
            r#"## 制司审计

**审计结论：** 所有操作均在授权范围内，未发现越权行为。

**授权记录：** 无异常。"#.to_string()
        )
    }
}
