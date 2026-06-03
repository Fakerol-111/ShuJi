# 枢机 (ShuJi)

> 基于中国古代三省六部制的 AI 驱动自动化软件开发系统。每个部门是一个 LLM agent，通过角色分工和文档化通信，模拟从需求分析到编码测试的完整软件工程流程。

## 测试状态

Rust 后端提供 **138 个测试**（26 个单元测试 + 112 个集成测试），覆盖：

- **单元测试** (`cargo test --lib`): token 计数、工具函数（skill/route 提取）、文档 ID 解析、routing 规则引擎
- **集成测试** (`cargo test --tests`): 文件 CRUD、路径安全（19 个细粒度测试覆盖遍历/符号链接/Unicode/空字节）、文档系统、Actor 消息、Session sanitize、PersistedContext round-trip、RunResult 枚举、配置覆盖、E2E 工作流（Mock LLM 路线）+ 自校正循环

所有测试使用临时目录隔离，`--test-threads=1` 规避并发状态竞争。

## 架构

```
皇帝 → send_message → 内阁(actor) → route_to → 各部门(actor)
                                                     ├─ 中书令 → 方案设计
                                                     ├─ 门下侍中 → 审查
                                                     ├─ 尚书令 → 执行调度
                                                     │   ├─ 吏部尚书 → 详细设计
                                                     │   ├─ 兵部尚书 → 测试+接口契约
                                                     │   ├─ 工部尚书 → TDD 编码 (分批计划循环)
                                                     │   ├─ 刑部尚书 → 运行测试验证
                                                     │   └─ 礼部尚书 → 规范检查+审计
                                                     └─ expand_requirements → 需求展开 sub-agent
```

### 核心技术

- **Actor 模型** — tokio actor + mpsc 通道异步通信，支持 `FastMessage` 精确中断、`FastChannel` 快速邮箱
- **Skill 系统** — 内阁/中书令/门下侍中通过 `<skill>` 标签按需加载技能，运行时动态创建
- **文档中心通信** — YAML frontmatter + 自动 ID，部门间只传文档 ID，不靠 LLM 对话传递上下文；plan/revw 类文档需皇帝朱批方可继续（`route_to` 和 `append_document` 硬门禁）
- **3 层上下文持久化** — base_prompt / soul_prompt / context_messages，skills 作为普通 system 消息存储在 context_messages 中，消除消息顺序漂移
- **单层上下文压缩** — 超阈值时自动调用 LLM 压缩早期对话为摘要；skill 消息在压缩前后剥离并重新追加，保持缓存命中率；支持运行时 mid-run 压缩
- **Session / AgentController 分离** — 纯 LLM 层 + 驱动循环层，支持 cancel/interrupt/watchdog/checkpoint
- **Agent 共享执行框架** — `agent/runner.rs` 提供统一 compact/build/checkpoint/context 加载，8 个非内阁 Agent 共享同一套执行逻辑
- **Watchdog 自愈** — 检测同工具重复、只读不写等异常模式，向 tool result 注入干预提示引导 LLM 自纠正
- **批量计划循环** — 工部尚书可将大任务拆分为多批次，批间上下文轻量恢复，不注入计划全文以减少缓存漂移
- **Soul 系统** — 内阁拥有可运行时演进的 `soul.md`，条目 500 字上限，文件 8KB 上限（超限自动 LLM 压缩）
- **Checkpoint 系统** — 定时 git commit + 会话快照，存储在 `.shuji/checkpoints/`，支持 Web UI 浏览和恢复
- **审计系统** — 事件 JSONL 持久化、文档血缘追溯（递归 refs 构建树）、时间线聚合、交付报告、变更 diff 保存、双向追踪（谁引用我/我引用谁）
- **反向引用索引** — `RefIndex` 维护文档引用关系的双向索引，`check_immutability` 可检测修改已审批文档是否影响下游
- **API 厂商可配置** — 支持为每个部门独立配置不同的 API 厂商（Anthropic / OpenAI / DeepSeek / 自定义），通过 UI 管理，URL 自动检测格式
- **Token 缓存跟踪** — 前后端完整支持缓存命中/未命中分开统计和展示（`token_tracker.rs` + `round_metrics.rs`）
- **config.local.toml 覆盖** — 开发者可创建本地配置文件覆盖默认设置，不污染仓库中的 `config.toml`

## 快速开始

### 环境要求

- Node.js >= 18
- Rust >= 1.70

### 配置

启动应用后进入设置面板（右上角 ⚙），配置以下信息：

- **API 密钥** — 你的 API Key
- **API URL** — 服务商地址，支持：
  - `https://api.anthropic.com/v1/messages`
  - `https://api.deepseek.com/chat/completions`
  - `https://api.openai.com/v1/chat/completions`
  - 或任意 OpenAI 兼容接口
- **模型** — 选择或手动输入模型名

配置保存在工作目录的 `api_config.json` 中。支持预设（balanced/economy/quality）一键切换，也支持为每个部门独立配置不同厂商。

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
2. 内阁根据 `<skill>` 选择工作流，可使用 `cancel_agent` 通过 `FastMessage` 精确中断指定部门
3. 各部门通过文档（YAML frontmatter + 自动 ID）通信；plan/revw 文档需皇帝朱批方可继续
4. `emperor_tx` → 前端 `chat-message` 事件
5. `dept_log_tx` → 前端 `dept-log` 事件（DeptStatusBar 底部状态栏实时面板）
6. `milestone_tx` → 持久化项目状态里程碑到 `.shuji/state.json`
7. `plan-update` 事件 → 前端 PlanInfo 面板（工部尚书批次进度）

## 设计理念

- **文档是契约** — 部门间通过文档通信，不靠 LLM 对话传递上下文
- **流程适配任务** — 内阁根据复杂度选择最轻量的流程（demo/simple/standard/complex）
- **职责隔离** — 设计的不写代码，编码的不做审查，测试的不分析
- **Soul 学习** — 内阁在实践中积累经验，跨越任务持久化
- **可审计性** — 所有关键步骤自动记录审计日志，支持文档变更 diff 和血缘追溯
