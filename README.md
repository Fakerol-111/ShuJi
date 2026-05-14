# 枢机 (ShuJi)

基于中国古代三省六部制的 AI 驱动自动化软件开发系统。每个部门是一个 LLM agent，通过角色分工和文档化通信，模拟从需求分析到编码测试的完整软件工程流程。

## 概念

- 用户只提需求（"我要做一个记账软件"）
- 内阁根据复杂度选择协作流程，路由到对应部门
- 中书令负责系统设计，门下侍中负责审查
- 尚书令协调执行链：吏部(详细设计) → 兵部(契约) → 工部(TDD 测试+编码) → 礼部(规范+覆盖审计) → 刑部(跑测试贴输出)
- 所有部门通过 `.shuji/` 目录下的文档通信，`route_to` 只传文档 ID

## 架构

```
皇帝 → send_message → 内阁(actor) → route_to → 各部门(actor)
                                                     ├─ 中书令 → 系统设计 (3 skills)
                                                     ├─ 门下侍中 → 审查 (2 skills)
                                                     ├─ 尚书令 → 执行调度
                                                     │   ├─ 吏部尚书 → 详细设计
                                                     │   ├─ 兵部尚书 → 接口契约
                                                     │   ├─ 工部尚书 → TDD 测试+编码
                                                     │   ├─ 刑部尚书 → 跑测试，贴原始输出
                                                     │   └─ 礼部尚书 → 规范检查 + 测试覆盖审计
                                                     └─ 制司 → 独立调查
```

### 核心技术

- **Actor 模型** — tokio actor + mpsc 通道异步通信
- **Skill 系统** — 内阁/中书令/门下侍中通过 `<skill>` 标签按需加载技能
- **文档中心通信** — YAML frontmatter + 自动 ID，部门间只传文档 ID
- **4 层上下文持久化** — base_prompt / skill_prompts / history_messages / context_messages 分层存储
- **上下文压缩** — 超阈值时自动调用 LLM 压缩早期对话为摘要
- **Session / AgentController 分离** — 纯 LLM 层 + 驱动循环层，支持 cancel/interrupt/watchdog

## 快速开始

### 环境要求

- Node.js >= 18
- Rust >= 1.70

### 配置

在 `shuji-app/` 下创建 `.env`（参考 `.env.template`）：

```
DEFAULT_API_KEY=sk-your-key
DEFAULT_API_URL=https://api.deepseek.com/chat/completions
DEFAULT_MODEL=deepseek-chat
```

支持为每个角色单独配置 API key（`NEIGE_API_KEY`, `ZHONGSHULING_API_KEY` 等）。URL 包含 `anthropic.com` → Anthropic Messages API，否则 → OpenAI Chat Completions。

### 运行

```bash
cd shuji-app
npm install
npm run tauri dev
```

## 项目状态

原型阶段。核心流水线可端到端运行，前端具备完整交互能力。

### 已实现

- 9 个部门 agent（内阁、中书令、门下侍中、尚书令、吏部、兵部、工部、刑部、礼部、制司）
- Skill 系统（内阁 7 个、中书令 3 个、门下侍中 2 个），按需加载
- 文档中心通信（YAML frontmatter + 自动 ID + 状态机）
- Report 按部门分文件夹存储
- 4 层上下文持久化 + 双层压缩（context + history）
- 项目级对话/日志持久化，打开不同项目互不串线
- Token 用量统计（今日/近3天/近7天/汇总）
- 双格式 API（OpenAI / Anthropic）
- 取消/中断机制
- 工具调用日志（按部门分文件记录完整参数）

### 待办

- 容器化测试执行环境
- 多流水线并行
- 错误恢复硬状态机

## 设计理念

- **文档是契约** — 部门间通过文档通信，不靠 LLM 对话传递上下文
- **流程适配任务** — 内阁根据复杂度选择最轻量的流程
- **职责隔离** — 设计的不写代码，编码的不做审查，测试的不分析
