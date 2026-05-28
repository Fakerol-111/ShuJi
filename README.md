# 枢机 (ShuJi)

> 基于中国古代三省六部制的 AI 驱动自动化软件开发系统。每个部门是一个 LLM agent，通过角色分工和文档化通信，模拟从需求分析到编码测试的完整软件工程流程。

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
                                                     │   ├─ 工部尚书 → TDD 测试+编码 (分批计划循环)
                                                     │   ├─ 刑部尚书 → 跑测试，贴原始输出
                                                     │   └─ 礼部尚书 → 规范检查 + 测试覆盖审计
                                                     ├─ 制司 → 独立调查/Bug 诊断
                                                     └─ expand_requirements → 需求展开 sub-agent
```

### 核心技术

- **Actor 模型** — tokio actor + mpsc 通道异步通信
- **Skill 系统** — 内阁/中书令/门下侍中通过 `<skill>` 标签按需加载技能
- **文档中心通信** — YAML frontmatter + 自动 ID，部门间只传文档 ID
- **4 层上下文持久化** — base_prompt / skill_prompts / history_messages / context_messages 分层存储，独立管理
- **双层上下文压缩** — 超阈值时自动调用 LLM 压缩早期对话为摘要（context 层），多摘要合并（history 层）
- **Session / AgentController 分离** — 纯 LLM 层 + 驱动循环层，支持 cancel/interrupt/watchdog
- **批量计划循环** — 工部尚书可将大任务拆分为多批次执行，每批注入独立上下文

### 新增特性

| 特性 | 说明 |
|------|------|
| **Soul 系统** | 内阁拥有可运行时演进的 `soul.md`，通过 `update_soul` 工具持久化经验教训 |
| **取消代理** | 内阁可通过 `cancel_agent` 工具中断其他部门操作 |
| **需求展开** | `expand_requirements` sub-agent 自动从用户视角展开任务需求 |
| **参与者模式** | 支持 `/level-1`(全自动) `/level-2`(关键节点确认) `/level-3`(逐步审核) |
| **每角色独立 API** | 每个部门可独立配置 API Key / URL / Model |
| **Thinking 模式** | 非 Anthropic API 自动启用 reasoning tokens，留更多空间给工具调用 |
| **删除/重命名文件** | 代理新增文件删除和重命名工具 |
| **日志汇总** | `summarize_logs` 工具读取 activity.log 生成项目进展报告 |
| **路径安全** | 完善 `resolve_scoped_path` 路径规范化，防止目录遍历攻击 |
| **工具调用日志** | 所有工具调用按部门分文件完整记录参数 |

## 快速开始

### 环境要求

- Node.js >= 18
- Rust >= 1.70

### 配置

在 `shuji-app/` 下创建 `.env`（参考 `.env.template`）：

```env
# 全局默认
DEFAULT_API_KEY=sk-your-key
DEFAULT_API_URL=https://api.deepseek.com/chat/completions
DEFAULT_MODEL=deepseek-v4-flash

# 参与者模式（可选）
PARTICIPATION_LEVEL=1   # 1=全自动 2=关键节点确认 3=逐步审核
```

支持为每个角色单独配置（`NEIGE_API_KEY`, `ZHONGSHULING_API_KEY` 等），未配置的角色回退到 `DEFAULT_*`。

URL 包含 `anthropic.com` → Anthropic Messages API，否则 → OpenAI Chat Completions（自动启用 Thinking 模式）。

### 运行

```bash
cd shuji-app
npm install
npm run tauri dev

# 仅前端开发
npm run dev

# 生产构建
npm run tauri build
```

## 消息流

1. 用户输入 → `send_message` Tauri command → `ActorSystem` 路由到内阁
2. 内阁根据 `<skill>` 选择工作流，可使用 `cancel_agent` 中断其他部门
3. 各部门通过文档（YAML frontmatter + 自动 ID）通信
4. `emperor_tx` → 前端 `chat-message` 事件（ChatBubble 渲染）
5. `dept_log_tx` → 前端 `dept-log` 事件（DeptStatusPanel 实时面板）
6. `milestone_tx` → 持久化项目状态里程碑到 `.shuji/state.json`

## 项目状态

原型阶段。核心流水线可端到端运行，前端具备完整交互能力。

### 已实现

- **12 个部门 agent**（内阁、中书令、门下侍中、尚书令、吏部、兵部、工部、刑部、礼部、制司 + expand_requirements sub-agent）
- **Skill 系统**（内阁 7 个、中书令 3 个、门下侍中 2 个），按需加载
- **文档中心通信**（YAML frontmatter + 自动 ID + 状态机）
- **4 层上下文持久化 + 双层压缩**（context + history 独立压缩）
- **Soul 系统** — 内阁可运行时学习并持久化经验
- **批量计划执行** — 工部尚书支持大任务分批次实现
- **项目级对话/日志持久化**，打开不同项目互不串线
- **参与者模式** — 全自动 / 关键节点确认 / 逐步审核三级
- **每角色独立 API 配置** — 支持 frontend 配置面板读写 `.env`
- **Token 用量统计**（今日/近3天/近7天/汇总）
- **双格式 API**（OpenAI Chat Completions / Anthropic Messages）
- **自动 Thinking 模式** — 为 DeepSeek 等 API 启用 reasoning tokens
- **取消/中断机制** — 全局 cancel + 内阁可中断特定部门
- **工具调用日志**按部门分文件记录完整参数
- **路径安全防护** — canonicalize 检测 symlink 逃逸、禁止 `..` 和绝对路径
- **截断自动恢复** — `finish_reason=length` 时自动翻倍 max_tokens 重试
- **所有工具返回结构化 JSON**（`ToolOutput { ok, operation, path, message, error_code }`）

### 待办

- 容器化测试执行环境
- 多流水线并行
- 错误恢复硬状态机

## 设计理念

- **文档是契约** — 部门间通过文档通信，不靠 LLM 对话传递上下文
- **流程适配任务** — 内阁根据复杂度选择最轻量的流程
- **职责隔离** — 设计的不写代码，编码的不做审查，测试的不分析
- **Soul 学习** — 内阁在实践中积累经验，跨越任务持久化
