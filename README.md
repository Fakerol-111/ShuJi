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

## 开发者指南

### 配置文件关系

ShuJi 使用多层配置，理解优先级和关系很重要：

```
优先级高                         优先级低
  │                                  │
  ▼                                  ▼
config.local.toml    api_config.json    .env     config.toml
  ──────────────      ──────────────    ─────     ──────────
  本地覆盖              API 密钥/端点    向后兼容     运行时配置
  不提交到仓库          不提交到仓库      后备          提交到仓库
```

#### 1. `config.toml`（仓库中）

运行时行为配置：API 超时、重试次数、max_tokens、tool iteration 次数、压缩阈值、watchdog 参数等。版本控制跟踪，所有开发者共享默认值。

#### 2. `config.local.toml`（本地，不提交）

覆盖 `config.toml` 中的任何字段。只需写要覆盖的部分。例如只改超时：

```toml
[api]
timeout_secs = 300
```

#### 3. `.env`（本地，不提交）

API 密钥和厂商配置。模板见 `.env.template`。

**向后兼容后备**：当 `api_config.json` 不存在时，系统读取 `.env`。首次通过 UI（右上角 ⚙ 设置）保存配置后，自动迁移到 `api_config.json`，后续修改请使用 UI。

#### 4. `api_config.json`（本地，不提交，由 UI 管理）

前端设置面板保存的 API 配置（每个角色可独立设置不同的 key/url/model）。优先于 `.env`。

#### 5. `context_config.json`（本地，不提交）

每角色的压缩阈值覆盖。通过前端「上下文设置」Tab 管理。

**配置加载优先级**：`config.local.toml > config.toml`（运行时）；`api_config.json > .env`（API）；`context_config.json 角色级 > config.toml 角色级 > config.toml 全局默认`（压缩阈值）。

### 运行测试

#### 快速验证（推荐 PR 前）

```bash
# 后端
cargo test --manifest-path shuji-app/src-tauri/Cargo.toml --lib
cargo test --manifest-path shuji-app/src-tauri/Cargo.toml --tests -- --skip expand_requirements --test-threads=1

# 前端
npm --prefix shuji-app run lint          # tsc --noEmit
npm --prefix shuji-app test              # Vitest
npm --prefix shuji-app run format:check  # Prettier

# Rust lint
cargo clippy --manifest-path shuji-app/src-tauri/Cargo.toml --all-targets 2>&1 | grep "warning"
```

#### 测试分类

| 类别 | 命令 | 需要 API Key？ |
|------|------|---------------|
| 单元测试 | `cargo test --lib` | ❌ |
| 文件 CRUD | `cargo test --test tool_test` | ❌ |
| 路径安全 | `cargo test --test path_security_test` | ❌ |
| 文档系统 | `cargo test --test document_test` | ❌ |
| Actor 消息 | `cargo test --test actor_test` | ❌ |
| Session | `cargo test --test session_test` | ❌ |
| Session 控制 | `cargo test --test session_control_test` | ❌ |
| 配置覆盖 | `cargo test --test config_test` | ❌ |
| Workflow Profile | `cargo test --test workflow_profile_test` | ❌ |
| E2E 工作流 | `cargo test --test workflow_demo_test` | ❌（Mock LLM） |
| 审计 | `cargo test --test audit_test` | ❌ |
| Checkpoint | `cargo test --test checkpoint_test` | ❌ |
| expand_requirements | `cargo test --test expand_requirements_test` | ✅（默认跳过） |

#### 运行单个测试

```bash
cargo test --manifest-path shuji-app/src-tauri/Cargo.toml --test audit_test test_append_read_roundtrip -- --nocapture
npm --prefix shuji-app test -- src/hooks/useChat.test.ts
```

所有后端测试使用 `tempfile::TempDir` 隔离，`--test-threads=1` 规避并发状态竞争。

### 代码风格

- **Rust**：`cargo fmt`（4 空格缩进），提交前清理 `clippy` 警告，公开 API 需 doc comment，优先 `Result<_, String>` / `anyhow::Result<_>`，避免 `unwrap()`
- **TypeScript / React**：Prettier 格式化（`npm run format`），`ChatMessage.role` 类型为 `RoleName` 联合类型（`types.ts`），新通用组件放 `components/ui/` 并导出到 `index.ts`，新 hook 以 `use` 开头放 `hooks/`
- **事件命名**：Tauri 事件使用 kebab-case：`chat-message`、`dept-log`、`plan-update`、`project-update`

### 提交前检查

```bash
# 后端
cd shuji-app/src-tauri
cargo fmt --check
cargo clippy --all-targets
cargo test --lib
cargo test --tests -- --skip expand_requirements --test-threads=1

# 前端
cd ../../
npm run format:check
npm run lint
npm test
```

### Checkpoint 系统

Checkpoint 系统在 `.shuji/.git/` 中维护一个**完全独立于项目 git 仓库**的隔离 git 仓库：

```
项目根/
├── .git/                  ← 项目 git 仓库（不受影响）
├── .shuji/
│   ├── .git/              ← ShuJi 隔离 git 仓库
│   ├── checkpoints/
│   │   ├── index.json     ← 索引（最多 500 条）
│   │   ├── 内阁/
│   │   │   └── <hash>.json  ← 会话快照
```

工作树是项目根，但 checkout 仅影响 `.shuji/` 目录。首次 checkpoint 提交所有 `.shuji/` 文件。`.gitignore` 中的 `.shuji/` 确保它不被项目 git 跟踪。

---

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
