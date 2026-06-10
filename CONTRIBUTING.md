# Contributing to ShuJi

## 目录

1. [本地开发环境](#本地开发环境)
2. [配置文件关系](#配置文件关系)
3. [运行测试](#运行测试)
4. [代码风格](#代码风格)
5. [提交前检查](#提交前检查)
6. [Checkpoint 系统说明](#checkpoint-系统说明)
7. [文档规范](#文档规范)

---

## 本地开发环境

### 依赖

| 依赖 | 版本要求 | 用途 |
|------|----------|------|
| Rust | 1.75+ | 后端（Tauri v2） |
| Node.js | 18+ | 前端（Vite + React） |
| Git | 2.20+ | checkpoint 系统（隔离仓库） |

### 首次构建

```bash
# 1. 克隆后安装前端依赖
cd shuji-app
npm install

# 2. 复制并配置环境变量
cp .env.template .env
# 编辑 .env，填入 API Key（至少 DEFAULT_API_KEY）

# 3. 复制并配置运行时配置（可选）
cp src-tauri/config.toml.template src-tauri/config.toml

# 4. 启动开发模式
npm run tauri dev
```

### 开发命令

| 命令 | 说明 |
|------|------|
| `npm run tauri dev` | 启动 Tauri 桌面开发（带热重载） |
| `npm run dev` | 仅前端（浏览器中预览 UI） |
| `npm run tauri build` | 构建生产版本 |
| `npx tsc --noEmit` | 前端 TypeScript 类型检查 |
| `npm test` | 前端 Vitest 单元测试 |
| `cargo check` (在 `src-tauri/`) | 后端类型检查（推荐，比 build 快） |
| `cargo clippy` (在 `src-tauri/`) | 后端 lint |
| `cargo fmt --check` (在 `src-tauri/`) | 后端格式检查 |

---

## 配置文件关系

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

### 1. `config.toml`（仓库中）

运行时行为配置：API 超时、重试次数、max_tokens、tool iteration 次数、压缩阈值、watchdog 参数等。版本控制跟踪，所有开发者共享默认值。

### 2. `config.local.toml`（本地，不提交）

覆盖 `config.toml` 中的任何字段。只需写要覆盖的部分。例如只改超时：

```toml
[api]
timeout_secs = 300
```

### 3. `.env`（本地，不提交）

API 密钥和厂商配置。模板见 `.env.template`。

**向后兼容后备**：当 `api_config.json` 不存在时，系统读取 `.env`。首次通过 UI（右上角 ⚙ 设置）保存配置后，自动迁移到 `api_config.json`，后续修改请使用 UI。

### 4. `api_config.json`（本地，不提交，由 UI 管理）

前端设置面板保存的 API 配置（每个角色可独立设置不同的 key/url/model）。优先于 `.env`。

### 5. `context_config.json`（本地，不提交）

每角色的压缩阈值覆盖。通过前端「上下文设置」Tab 管理。

### 配置加载优先级

```
config.local.toml > config.toml           （运行时配置覆盖）
api_config.json  > .env                   （API 端点覆盖）
context_config.json 角色级 > config.toml 角色级 > config.toml 全局默认  （压缩阈值覆盖）
```

### 何时需要 API Key

- **日常开发/调试**：至少设置 `DEFAULT_API_KEY`，或通过 UI 添加
- **本地测试**：`cargo test --lib` + 大多数集成测试**不需要** API Key
- **expand_requirements_test**：需要真实 API Key，默认**跳过**
- **E2E 工作流测试**：使用 Mock LLM，**不需要** API Key

---

## 运行测试

### 快速验证（推荐 PR 前）

```bash
# 后端（从项目根目录）
cargo test --manifest-path shuji-app/src-tauri/Cargo.toml --lib
cargo test --manifest-path shuji-app/src-tauri/Cargo.toml --tests -- --skip expand_requirements --test-threads=1

# 前端
npm --prefix shuji-app run lint          # tsc --noEmit
npm --prefix shuji-app test              # Vitest
npm --prefix shuji-app run format:check  # Prettier

# Rust lint
cargo clippy --manifest-path shuji-app/src-tauri/Cargo.toml --all-targets 2>&1 | Select-String "warning"
```

### 测试分类

| 类别 | 命令 | 测试数 | 需要 API Key？ |
|------|------|--------|---------------|
| 单元测试 | `cargo test --lib` | ~96 | ❌ 不需要 |
| 文件 CRUD | `cargo test --test tool_test` | 13 | ❌ 不需要 |
| 路径安全 | `cargo test --test path_security_test` | 19 | ❌ 不需要 |
| 文档系统 | `cargo test --test document_test` | 24 | ❌ 不需要 |
| Actor 消息 | `cargo test --test actor_test` | 25 | ❌ 不需要 |
| Session | `cargo test --test session_test` | 8 | ❌ 不需要 |
| Session 控制 | `cargo test --test session_control_test` | 14 | ❌ 不需要 |
| 配置覆盖 | `cargo test --test config_test` | 8 | ❌ 不需要 |
| Workflow Profile | `cargo test --test workflow_profile_test` | 13 | ❌ 不需要 |
| E2E 工作流 | `cargo test --test workflow_demo_test` | 2 | ❌ 不需要（Mock LLM） |
| 审计 | `cargo test --test audit_test` | 24 | ❌ 不需要 |
| Checkpoint | `cargo test --test checkpoint_test` | 8 | ❌ 不需要 |
| expand_requirements | `cargo test --test expand_requirements_test` | 1 | ✅ **需要**（默认跳过） |

### 运行单个测试

```bash
# 后端
cargo test --manifest-path shuji-app/src-tauri/Cargo.toml --test audit_test test_append_read_roundtrip -- --nocapture

# 前端
npm --prefix shuji-app test -- src/hooks/useChat.test.ts
```

### 测试隔离

所有后端测试使用 `tempfile::TempDir` 创建隔离的临时目录。测试之间不共享文件系统状态。前端测试使用 Vitest 的 jsdom 环境 + mock Tauri API。

---

## 代码风格

### Rust

- 使用 `cargo fmt`（4 空格缩进，标准 Rust 风格）
- `clippy` 警告请在提交前清理
- 公开 API 函数需有 doc comment（中文或英文均可）
- 错误优先：优先返回 `Result<_, String>` 或 `anyhow::Result<_>`，避免 `unwrap()`

### TypeScript / React

- 使用 Prettier 格式化（`npm run format`）
- `ChatMessage.role` 的类型为 `RoleName` 联合类型（`types.ts`）
- 新的通用组件放在 `components/ui/` 并导出到 `index.ts`
- 新的 hook 文件以 `use` 开头，放在 `hooks/`

### 事件命名

Tauri 事件名称使用 kebab-case：
- `chat-message` — 聊天消息推送
- `dept-log` — 部门活动日志
- `plan-update` — 计划进度更新
- `project-update` — 项目状态更新

---

## 提交前检查

```bash
# ── 后端 ──
cd shuji-app/src-tauri
cargo fmt --check
cargo clippy --all-targets -- -W clippy::all 2>&1 | Select-String "warning"
cargo test --lib
cargo test --tests -- --skip expand_requirements --test-threads=1

# ── 前端 ──
cd ../../
npm run format:check
npm run lint
npm test
npm run build
```

> **注意**：Prettier 格式检查当前有 62 个文件的存量问题（`format:check` 会列出它们）。如果你只改了特定文件，可以先对改过的文件运行 `npx prettier --write src/your-file.ts`。

---

## Checkpoint 系统说明

### 工作原理

Checkpoint 系统在 `.shuji/.git/` 中维护一个**完全独立于项目 git 仓库**的隔离 git 仓库：

```
项目根/
├── .git/                  ← 你的项目 git 仓库（不受影响）
├── .shuji/
│   ├── .git/              ← ShuJi 的隔离 git 仓库
│   ├── checkpoints/
│   │   ├── index.json     ← checkpoint 索引（最多 500 条）
│   │   ├── 内阁/
│   │   │   └── <hash>.json  ← 会话快照
│   │   └── ...
│   └── ...
```

### 重要注意事项

1. **工作树是项目根**。`git --work-tree=.` 的设置意味着 checkpoint 的 `git add -A` 会作用于整个项目根目录内的所有文件变更（包括你的代码修改）。但因为这是隔离仓库，`git checkout` 操作**只会影响 `.shuji/` 目录**（这个目录是唯一在 checkpoint git 历史中的路径）。

2. **首次 checkpoint 会提交所有 `.shuji/` 文件**。初始化时 `.shuji/` 目录已有文件，第一次 checkpoint 会将它们全部提交。

3. **不会影响你的项目 git 仓库**。`.shuji/.git/` 是完全独立的。你的 `.gitignore` 中的 `.shuji/` 确保它不会被你的项目 git 仓库跟踪。

4. **Checkpoint 恢复**会尝试：
   - 暂存未提交的变更（`git stash`）
   - `git checkout --detach <commit_hash>` 到检查点
   - 将会话快照写回 `.shuji/context/`
   - 不自动 `git stash pop`（恢复后需要手动确认）

### 当 checkpoint 保存失败时

常见原因：
- **没有 git 用户配置**：ShuJi 会在初始化时设置本地 `user.name` 和 `user.email`，但如果环境没有 `git` 可用，checkpoint 不会工作
- **权限问题**：`.shuji/` 目录需要读写权限
- 失败会被记录到日志（`[checkpoint]` 前缀），不会中断程序运行

---

## 文档规范

- `ARCHITECTURE.md` — 当前架构的权威描述，新增功能或重构后同步更新
- `OPTIMIZATION_PLAN.md` — 优化路线图和执行进度，完成任务后更新完成表
- `CLAUDE.md` / `MEMORY.md` — AI 辅助开发的工作记忆，不提交到仓库（在 `.gitignore` 中）
- `docs/` — 设计文档和未来方案，不提交到仓库
