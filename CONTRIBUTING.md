# 贡献指南

感谢你对枢机的关注。本文档面向希望本地开发、运行测试或提交 PR 的贡献者。

## 开发环境

```bash
cd shuji-app
npm install
npm run tauri dev      # 完整桌面应用（热重载）
npm run dev            # 仅前端（浏览器）
npm run tauri build    # 生产构建
```

环境要求：Node.js >= 18，Rust >= 1.70。

### Linux 系统依赖

在 Ubuntu/Debian 上运行 Tauri 桌面应用需安装 GTK/WebKit 开发库（与 CI 一致）：

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf \
  libgtk-3-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev
```

构建 AppImage 还需 `libfuse2`。其他发行版见 [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/)。

Python 项目测试需系统安装 `python3`（或 `python`）。

新人请先读 [ONBOARDING.md](ONBOARDING.md)。架构叙事见 [shuji-app/docs/ARCHITECTURE.md](shuji-app/docs/ARCHITECTURE.md)；文件级索引见 [shuji-app/docs/MAINTAINER_INDEX.md](shuji-app/docs/MAINTAINER_INDEX.md)；AI 维护入口见 `CLAUDE.md` / `AGENTS.md`（本地文件）。

## 仓库目录约定

```
ShuJi/                          # 仓库根
├── ONBOARDING.md               # 新人文档索引（从这里开始）
├── README.md                   # 项目介绍
├── CONTRIBUTING.md             # 本文件
├── CHANGELOG.md
├── scripts/                    # 仓库级辅助脚本（非应用构建）
├── assets/                     # 答辩、演示等非代码资产
└── shuji-app/                  # 唯一主应用
    ├── src/                    # React 前端
    ├── src-tauri/src/          # Rust 后端
    │   ├── actor/              # Actor 消息循环
    │   ├── agent/              # 各部门 Agent（含 prompt.md、skills/）
    │   ├── api/                # Session、AgentController、LLM 客户端、reasoning
    │   ├── tool/               # 工具注册与 dispatch
    │   ├── commands/           # Tauri invoke 入口
    │   ├── audit/              # 审计、血缘、diff
    │   ├── pipeline/           # Pipeline 引擎
    │   ├── workflow/           # Workflow Profile
    │   ├── validate/           # 交付验证：contract、lint、diff、tests_runner
    │   ├── learning/           # 角色学习：store、extract、inject
    │   └── config/             # RuntimeConfig (TOML)
    ├── src-tauri/tests/        # Rust 集成测试
```

### 新增内容应放哪里

| 类型 | 路径 |
|------|------|
| Tauri 命令 | `shuji-app/src-tauri/src/commands/` |
| 新 Agent | `shuji-app/src-tauri/src/agent/{角色}/`（至少 `mod.rs` + `prompt.md`） |
| 新工具 | `shuji-app/src-tauri/src/tool/`，并在 `registry.rs` 注册 |
| Rust 集成测试 | `shuji-app/src-tauri/tests/` |
| React 页面 | `shuji-app/src/pages/` |
| React 组件 | `shuji-app/src/components/`（通用 primitive → `components/ui/`） |
| 前端 hook | `shuji-app/src/hooks/` |
| 一次性脚本 | `scripts/` |

### 不要做的事

- 不要在仓库根再建第二个应用目录
- 不要向 Git 提交 `api_config.json`、`.env`、`config.local.toml`（含密钥或本机覆盖）

## 配置文件

ShuJi 使用多层配置，理解优先级很重要：

```
优先级高                         优先级低
  │                                  │
  ▼                                  ▼
config.local.toml    api_config.json    .env     config.toml
  ──────────────      ──────────────    ─────     ──────────
  本地覆盖              API 密钥/端点    向后兼容     运行时配置
  不提交到仓库          不提交到仓库      后备          提交到仓库
```

### `config.toml`（仓库中）

运行时行为：API 超时、重试、max_tokens、tool iteration 次数、压缩阈值、watchdog 参数等。版本控制跟踪，团队共享默认值。

### `config.local.toml`（本地，不提交）

覆盖 `config.toml` 任意字段，只写需要改的部分。例如：

```toml
[api]
timeout_secs = 300
```

### `.env`（本地，不提交）

API 密钥，模板见 `shuji-app/.env.template` 或 `shuji-app/src-tauri/.env.template`。

**向后兼容**：`api_config.json` 不存在时读取 `.env`；首次通过 UI 保存配置后自动迁移到 `api_config.json`。

### `api_config.json`（本地，不提交，UI 管理）

每角色可独立设置 key / url / model，优先于 `.env`。

### `context_config.json`（本地，不提交）

每角色压缩阈值覆盖，通过前端「上下文设置」管理。

**加载优先级**：`config.local.toml > config.toml`（运行时）；`api_config.json > .env`（API）；`context_config.json 角色级 > config.toml 角色级 > config.toml 全局`（压缩阈值）。

## 测试

Rust 后端与前端合计约 730 个测试（Rust 单元 + 集成 + 前端 Vitest，具体数量以 `scripts/count_tests.sh` 输出为准），覆盖 token 计数、路由规则、文档系统、Actor 消息、Session、配置、E2E 工作流（Mock LLM）、审计、Checkpoint 等。所有测试使用临时目录隔离，建议 `--test-threads=1` 规避并发状态竞争。

### 快速验证（PR 前推荐）

```bash
# 后端
cargo test --manifest-path shuji-app/src-tauri/Cargo.toml --lib
cargo test --manifest-path shuji-app/src-tauri/Cargo.toml --tests -- --skip expand_requirements --test-threads=1

# 前端
npm --prefix shuji-app run lint
npm --prefix shuji-app test
npm --prefix shuji-app run format:check

# Rust lint
cargo clippy --manifest-path shuji-app/src-tauri/Cargo.toml --all-targets
```

### 测试分类

| 类别 | 命令 | 需要 API Key？ |
|------|------|---------------|
| 单元测试 | `cargo test --lib` | ❌ |
| 文件 CRUD | `cargo test --test tool_test` | ❌ |
| 路径安全 | `cargo test --test path_security_test` | ❌ |
| 命令安全 | `cargo test --test command_security_test` | ❌ |
| 文档系统 | `cargo test --test document_test` | ❌ |
| Actor 消息 | `cargo test --test actor_test` | ❌ |
| Session | `cargo test --test session_test` | ❌ |
| Session 控制 | `cargo test --test session_control_test` | ❌ |
| 配置覆盖 | `cargo test --test config_test` | ❌ |
| Dispatch 门禁 | `cargo test --test dispatch_gate_test` | ❌ |
| Workflow mock | `cargo test --test workflow_mock_test` | ❌ |
| E2E 工作流 | `cargo test --test workflow_demo_test` | ❌（Mock LLM） |
| Pipeline 引擎 | `cargo test --test pipeline_test` | ❌ |
| 交付验证 | `cargo test --test validate_test` | ❌ |
| 审计 | `cargo test --test audit_test` | ❌ |
| Checkpoint | `cargo test --test checkpoint_test` | ❌ |
| 外部编辑器 | `cargo test --test editor_test` | ❌ |
| 角色学习 | `cargo test --test learning_test` | ❌ |
| 消息路由 | `cargo test --test send_message_routing_test` | ❌ |
| 场景回放 | `cargo test --test scenario_replay_test` | ❌ |
| expand_requirements | `cargo test --test expand_requirements_test` | ✅（默认跳过） |

### 运行单个测试

```bash
cargo test --manifest-path shuji-app/src-tauri/Cargo.toml --test audit_test test_append_read_roundtrip -- --nocapture
npm --prefix shuji-app test -- src/hooks/useChat.test.ts
```

## 代码风格

- **Rust**：`cargo fmt`（4 空格），提交前清理 `clippy` 警告，优先 `Result<_, String>` / `anyhow::Result<_>`，避免 `unwrap()`
- **TypeScript / React**：Prettier（`npm run format`），`ChatMessage.role` 为 `RoleName` 联合类型，新通用组件放 `components/ui/`，新 hook 以 `use` 开头放 `hooks/`
- **事件命名**：Tauri 事件使用 kebab-case：`chat-message`、`dept-log`、`plan-update`、`project-update`

## 提交前检查

```bash
cd shuji-app/src-tauri
cargo fmt --check
cargo clippy --all-targets
cargo test --lib
cargo test --tests -- --skip expand_requirements --test-threads=1

cd ../..
npm run format:check
npm run lint
npm test
```

## 架构与文件索引

架构叙事（主流程、9 Actors、Session/AgentController、审计、Checkpoint 等）见 **[shuji-app/docs/ARCHITECTURE.md](shuji-app/docs/ARCHITECTURE.md)**。

文件级索引、Session Limits、Edge Cases、Token Tracking 见 **[shuji-app/docs/MAINTAINER_INDEX.md](shuji-app/docs/MAINTAINER_INDEX.md)**。

### 消息流（快速参考）

1. 用户输入 → `send_message` → `ActorSystem` 路由到内阁
2. 内阁根据 `<skill>` 选择工作流，可用 `cancel_agent` 精确中断指定部门
3. 各部门通过文档（YAML frontmatter + 自动 ID）通信；plan/revw 文档需朱批方可继续
4. `emperor_tx` → 前端 `chat-message` 事件
5. `dept_log_tx` → 前端 `dept-log` 事件
6. `milestone_tx` → 持久化到 `.shuji/state.json`
7. `plan-update` → 工部批次进度面板

### Checkpoint 系统

Checkpoint 在 `.shuji/.git/` 维护**独立于项目 `.git/`** 的隔离仓库：

```
项目根/
├── .git/                  ← 项目 git（不受影响）
├── .shuji/
│   ├── .git/              ← ShuJi 隔离 git
│   ├── checkpoints/
│   │   ├── index.json     ← 索引（最多 500 条）
│   │   └── {角色}/
│   │       └── <hash>.json
```

工作树为项目根，checkout 仅影响 `.shuji/`。项目 `.gitignore` 中的 `.shuji/` 确保不被项目 git 跟踪。

## 提交 PR

1. Fork 仓库并创建功能分支
2. 确保测试与 lint 通过（见上方「提交前检查」）
3. 提交 PR 到 `main`，简要说明改动动机与测试情况

有问题可先开 [Issue](https://github.com/Fakerol-111/ShuJi/issues)。
