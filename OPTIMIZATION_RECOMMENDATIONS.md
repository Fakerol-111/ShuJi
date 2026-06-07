# 枢机（ShuJi）代码优化建议

> **说明**：本文档完全基于仓库实际代码分析（2026-06-07），不以 `CLAUDE.md`、`ARCHITECTURE.md` 等文档为准。  
> 分析范围：`shuji-app/src-tauri/src/`（Rust 后端）、`shuji-app/src/`（React 前端）、`shuji-app/src-tauri/tests/`、CI 配置。

---

## 执行摘要

当前系统核心链路（Actor → Agent → Session/Control → Tool）已能端到端运行，Rust 集成测试覆盖工具/会话/路径安全等基础能力。主要优化空间集中在：

1. **重复 I/O 与轮询** — 后端多次读 `context_config.json`、前端 7+ 路定时轮询叠加
2. **代码重复与死代码** — 6 个部门 agent 模板相同、`agent_builder.rs` / `api/persisted_context.rs` 未接入
3. **前后端类型漂移** — `types.ts` 与 Rust 模型不一致，无 codegen
4. **持久化策略** — token 统计每次 API 调用全量写文件、`state.json` 非原子写
5. **测试与 CI 缺口** — 前端 vitest 脚本断裂、Actor/Checkpoint/Compact 无集成测试

建议按 **P0 → P3** 分阶段实施，优先处理正确性与热路径性能，再推进架构收敛。

---

## P0 — 立即修复（正确性 / 热路径）

### 1. 修复 `load_and_compact_context` 双份任务注入

**位置**：`agent/runner.rs:107-108` + 各部门 `execute()`（如 `libushangshu/mod.rs:52-53`）

**现象**：`Session::new` 已把 `task_description` 放入 messages；若存在持久化 context，`load_and_compact_context` 会再 push 一次相同 user message。

**影响**：浪费 token，可能混淆 LLM（重复任务描述）。

**建议**：仅在 fresh start 路径注入 task；reload 路径跳过，或检测 messages 末尾是否已有相同内容。

---

### 2. 前端 sessionStorage 在 render 中同步写入

**位置**：`pages/ProjectDashboard.tsx:57`

```typescript
try { sessionStorage.setItem(STORAGE_KEY, JSON.stringify({ msgs: messages, discuss: discussMsgs })); } catch {}
```

**现象**：每次 re-render 都序列化并写入，消息变长后 I/O 成本显著。

**建议**：移入 `useEffect(() => { ... }, [messages, discussMsgs])`，或 debounce 300–500ms。

---

### 3. 消除重复 1s 轮询 `useActiveDepts`

**位置**：
- `pages/ProjectDashboard.tsx:47`
- `components/DeptStatusBar.tsx:15`
- `hooks/useActiveDepts.ts:24`（`POLL_INTERVAL_MS = 1000`）

**现象**：Dashboard 与 DeptStatusBar 各实例化一次 hook，产生 **2 个独立 1s IPC 轮询**（`getActiveRoles`）。

**建议**：
- 提升到 Context 共享状态；或
- 后端在 actor 启停时 emit `active-roles-changed` 事件（`round_metrics.rs` 已有 `ACTIVE_ROLES` 基础），前端改事件驱动。

---

### 4. 修复断裂的前端测试脚本

**位置**：`package.json:11` — `"test": "vitest run"`

**现象**：`devDependencies` 无 `vitest`，`src/` 下无 `*.test.ts(x)`，CI 也不跑前端测试。干净环境执行 `npm test` 会失败。

**建议**：要么安装 vitest 并补最小 smoke test；要么移除 script 避免误导。

---

## P1 — 高价值性能与架构

### 5. 缓存 `context_config.json` 读取

**重复读盘位置**（每次 actor exec 迭代 / compact 回调均可能触发）：

| 文件 | 行号 |
|------|------|
| `actor/mod.rs` | ~351 |
| `agent/runner.rs` | 36 |
| `agent/neige/mod.rs` | ~438 |
| `commands/workflow.rs` | 623, 737 |

**建议**：在 `ActorContext` 或 `AppState` 层加载一次，文件 mtime 变更时刷新；compact handler 闭包引用 `Arc<HashMap>` 而非每次 `read_to_string`。

---

### 6. 事件驱动替代前端轮询集群

**当前 Tauri 事件**（`commands/workflow.rs:302-354`）：仅 3 种 — `chat-message`、`dept-log`、`plan-update`。

**当前轮询**：

| 数据源 | 间隔 | 文件 |
|--------|------|------|
| `getActiveRoles` | 1s ×2 | `useActiveDepts.ts` |
| `getRoundMetrics` | 3s | `DeptStatusBar.tsx:41` |
| `getWorkflowState` | 3s | `WorkflowTimeline.tsx:110` |
| `getPendingApprovals` | 3s | `ProjectDashboard.tsx:79` |
| `getWorkflowGraph` | 5s | `WorkflowGraph.tsx:97` |
| `getContextStats` | 10s | `ContextPanel.tsx:19` |
| `getTokenStats` | 30s | `DeptStatusBar.tsx:32` |

**建议**：milestone / actor 状态变更时 emit 聚合事件（如 `project-snapshot`、`round-metrics-update`）；轮询仅作 fallback。`constants.ts` 已定义 `DEPT_ACTIVE_TIMEOUT_MS`、`TOKEN_REFRESH_INTERVAL_MS` 等常量，但组件使用硬编码值，应统一引用。

---

### 7. Project 状态前端不刷新

**后端**：milestone handler 更新 `AppState.current_project` 并 `save_project`（`workflow.rs:358-377`），但不 emit 任何 project 事件。

**前端**：`project.phases` / `project.overall` 仅在 `loadProject` 时设置；`getProject()` / `getSnapshot()`（`api.ts:25-48`）**零调用**。

**影响**：WorkflowTimeline 进度条依赖陈旧 `project` 对象，只能靠 3s 轮询 `getWorkflowState` 部分补偿。

**建议**：milestone 时 emit `project-update`（完整 `Project` 或 `ProjectSnapshot`），或前端在 `chat-message` / milestone 空闲时调用 `getProject()`。

---

### 8. 统一部门 Agent 执行模板

**现状**：`agent/runner.rs` 已提取 `build_compact_handler`、`build_checkpoint_handler`、`load_and_compact_context`、`save_context`，但 6+ 部门（吏/兵/刑/礼/尚书令等）的 `execute()` 仍复制 ~80 行相同流程（见 `libushangshu/mod.rs:46-128`）。

**建议**：扩展 runner 为 `run_standard_agent(input, tools_fn, prompt, is_cabinet)`；中书令/门下侍中的 skill outer loop 再抽 `run_skill_gated_agent()`。预计减少 ~800 行重复，降低 drift 风险。

**附带问题**：
- `build_compact_handler` 硬编码 interval=40（`runner.rs:59`），与 `config/mod.rs` 中 `default_max_exec_iterations = 20` 不一致，且不可配置
- 各部门 `Arc::new(self.client.clone())`（`libushangshu/mod.rs:55`）多余 — `Session` 已接受 `&Arc<AnthropicClient>`

---

### 9. 共享 HTTP Client

**位置**：`commands/workflow.rs:59-98` — 每个 actor 独立 `AnthropicClient::new()`，各自持有 `reqwest::Client`。

**影响**：9 个 actor = 9 个连接池，无法复用 TCP/TLS 连接。

**建议**：按 `(api_url, api_key)` 去重，共享 `Arc<AnthropicClient>`；`discuss_with_cabinet`、`compact_context` 等同理。

---

### 10. `Session::api_request` 仅 OpenAI 格式

**位置**：`api/session.rs:833-841`

```rust
.header("Authorization", format!("Bearer {}", client.api_key))
```

**对比**：`api/client.rs` 的 `send_message` 有 Anthropic/OpenAI 双格式分支；但 **tool-use 主路径 `Session::step()` 走 `api_request`**，Anthropic URL + 工具调用可能失败（除非代理转写格式）。

**建议**：将 `client.rs` 的双格式逻辑下沉到共享层；或启动时检测 URL 并 assert/warn；补 Anthropic tool-use 集成测试。

---

## P2 — 中期改进

### 11. 前后端类型对齐

| 差异点 | 后端 | 前端 `types.ts` |
|--------|------|-----------------|
| `Project` 字段 | 含 `state`、`talk`、`task`、`resume`、`summary` 等（`models/project.rs:4-41`） | 仅 9 字段（`types.ts:1-11`） |
| `OverallStatus::Rejected` | 普通枚举变体 → `"Rejected"`（`project.rs:140`） | `{ Rejected: number }`（`types.ts:28`） |
| `Escalated` | **不存在** | 存在于 `types.ts:29,44` |
| `ChatMessage.documents` | **不存在**（`models/chat.rs:4-9`） | `documents: Document[]`（`types.ts:83`） |
| `ChatResponse` | N/A | 定义但全项目无引用（`types.ts:93-96`） |

**建议**：引入 `tauri-specta` 或 `typeshare` 从 Rust 生成 TS 类型；短期至少同步 `types.ts` 并清理死字段。

---

### 12. 持久化与 I/O 优化

| 问题 | 位置 | 建议 |
|------|------|------|
| token 每次 API 调用全量 rewrite JSON | `token_tracker.rs:65-69` | 改 append-only JSONL 或批量 flush（如每 10 次或 5s） |
| `state.json` 非原子写 | `storage/shuji_dir.rs:221-225` — 直接 `fs::write` | 对齐 `PersistedContext::save_to` 的 tmp+rename 模式 |
| 每条 chat/dept-log 消息 open+append | `workflow.rs:311-318, 337-344` | 持有 file handle 或 batch writer task |
| checkpoint index 读-改-写 | `storage/checkpoint.rs:255-273` | 考虑 append-only index 或 debounce |
| `forward_route` 每次写 graph | `actor/mod.rs:626-631` | debounce 或批量写 |
| 全局 `COUNTER_LOCK` | `tool/documents.rs:7-27` | 按项目 shuji_dir 分锁 |
| 全局 `LOG_LOCK` | `tool/tool_log.rs:4-36` | 按部门/项目分锁或 async channel 写盘 |

---

### 13. Actor 系统改进

**位置**：`actor/mod.rs`

| 问题 | 说明 |
|------|------|
| `shared_context` 只写不读 | 写入 `402-404`，grep 无读取方 — 死字段，应移除或使用 |
| `talk_history` 无裁剪 | `272-280, 409-411` 持续 append，与 `Project.append_talk` 裁剪逻辑脱节 |
| unbounded mailbox | `workflow.rs:182-183` 注释写 capacity 16，实际 `unbounded_channel` |
| async 中使用 `std::sync::Mutex` | `111-117` — `shared_context`、`talk_history` 等可能阻塞 executor |
| 通道发送静默失败 | 大量 `let _ = tx.send(...)` |
| Cancel 竞态 | 每条消息开头 `cancel.store(false)`（`254`），可能与 Interrupt 并发 |

**建议**：`talk_history` 对齐 `Project.append_talk` 容量；考虑 `tokio::sync::Mutex`；mailbox 改 bounded + backpressure 策略。

---

### 14. 上下文压缩配置与行为

**位置**：`config/mod.rs:199-207`

- `default_compact_token_threshold = 750_000` — 极高阈值
- `default_compact_mid_run_enabled = false` — 运行时 compact 默认关闭
- `default_compact_thresholds_for_role()` — 9 个角色全部返回相同值（`498-504`）

**影响**：自动压缩几乎不触发，依赖手动 `compact_context`；与产品预期（文档称 20 迭代 mid-run compact）不符。

**建议**：确认产品意图后调整默认值；compact interval 接入 `RuntimeConfig`；角色差异化内置推荐值。

---

### 15. 前端聊天性能

| 问题 | 位置 | 建议 |
|------|------|------|
| 全量重渲染 | `ChatPanel.tsx:59-66` — 无 memo/虚拟列表 | `React.memo(ChatBubble)`；长会话用 `@tanstack/react-virtual` |
| 重量级 Markdown | `ChatBubble.tsx` — 每条消息 `ReactMarkdown` + `remarkGfm` + `rehypeHighlight` | 按需 lazy import highlight；无 code block 时跳过 |
| `handleSend` 未稳定化 | `useChat.ts` 每次 render 新建 | `useCallback` 包装，避免 Demo effect 重触发（`ProjectDashboard.tsx:84-91`） |
| 重复 IPC | `getChatHistory` 在 `useProject.ts` + `useChat.ts` 各调一次 | 聊天历史只由 `useChat` 负责 |
| render 中突变 session | `ProjectDashboard.tsx:51` — `session.msgs = initialMsgs` | 改用 `useEffect` 或统一状态源 |

---

### 16. 删除或接入死代码

| 文件 | 状态 |
|------|------|
| `api/persisted_context.rs` | 存在但未加入 `api/mod.rs`（`mod.rs` 仅 5 模块），与 `session.rs` 内 `PersistedContext` 重复 |
| `commands/agent_builder.rs` | 存在但未加入 `commands/mod.rs`（`mod.rs` 仅 7 模块），与 `workflow.rs:47-254` 大量重复，且缺少 `workflow_graph`、`shared_context`、`plan` 等字段 |

**建议**：删除遗留文件，或完成 refactor 并切换引用，避免双份实现 drift。

---

### 17. `tool/mod.rs` 拆分

**现状**：~1900 行，dispatch + 文件操作 + 文档 + 路由 + 特殊工具全在一个文件。

**建议**：按 `file_ops`、`documents`、`routing`、`command_exec` 拆子模块；`read_file` 在大文件场景应支持 offset/limit 而不全量读入（当前 `700-706` 先读全文件再切片）。

---

## P3 — 长期 / 维护性

### 18. 测试覆盖缺口

**已有**（Rust，~124 integration tests）：`tool_test`、`path_security_test`、`document_test`、`session_test`、`session_control_test`、`config_test`、`actor_test`（浅）、`workflow_demo_test`（E2E mock LLM）。

**缺失**：

| 区域 | 说明 |
|------|------|
| `run_actor` 集成 | Interrupt / Replace / fallback / plan loop / pause |
| Compact 全流程 | 无 `tests/*compact*` |
| Checkpoint | list/restore/git 隔离 repo |
| Token tracker | record/persist/aggregate |
| Tauri commands | 50+ `#[tauri::command]` 无直接测试 |
| `audit` / `shuji_docs` / `friendly_error` | 无测试文件 |
| 事件端到端 | chat-message / dept-log emit→listen |
| 前端 | 0 测试文件 |

**CI**（`.github/workflows/check.yml`）：
- Rust test 用 `--test-threads=1` 串行，耗时长
- 无 `npm run format:check`
- frontend job 无 npm cache（`publish.yml` 有）
- 无 `swatinem/rust-cache` / nextest

---

### 19. 构建与发布优化

**前端**（`vite.config.ts`）：
- 无路由级 `React.lazy`（`main.tsx` 静态 import 全部页面）
- 无 `manualChunks`（markdown/highlight 可独立 chunk）
- `react-markdown` + `rehype-highlight` 在 ChatBubble 与 DocPreview 均全量引入

**Rust**（`Cargo.toml:22`）：
- `tokio = { features = ["full"] }` — 可能引入未用特性
- 无 `[profile.release]` 自定义（LTO、`codegen-units=1`）

**Tauri**（`tauri.conf.json`）：`csp: null`

---

### 20. 错误处理与可观测性

| 模式 | 位置 | 建议 |
|------|------|------|
| `let _ =` 吞掉 persist 错误 | `session.rs` PersistedContext::save_to、`workflow.rs` milestone save | 至少 log + 可选前端 toast |
| tool 错误检测不一致 | `control.rs` 用字符串 `contains("失败")` heuristic | 统一 `ToolOutput { ok, error_code }` JSON 解析 |
| API 重试固定 2s | `session.rs:507` | 指数退避 + jitter |
| `round_metrics` / `token_tracker` lock 失败静默 return | 多处 `if let Ok` / `Err(_) => return` | 计数器 + 告警 |

---

### 21. 工部 Plan 状态双轨

**现象**：
- Actor 层 `plan` 字段（`workflow.rs:224`）与 `GongbuShangshuAgent` 内部 `Arc<Mutex<PlanState>>` 两套状态
- `plan_display()` JSON 解析在 actor（`actor/mod.rs:323-334`）做 progress 检测
- 工部用 `std::sync::Mutex` 在 async 中 `.lock().unwrap()`（`gongbushangshu/mod.rs`）

**建议**：Plan 状态单一来源（agent 或 actor 择一）；async 路径用 `tokio::sync::Mutex`。

---

### 22. Config `merge_from` 语义

**位置**：`config/mod.rs:369-444`

**现象**：基于默认值比较 — 若 `config.local.toml` 显式设为与 default 相同值，会被忽略，无法"覆盖回默认"。

**建议**：改用"字段是否存在于 override 文件"而非"值是否等于 default"；补 `merge_from` 单元测试。

---

## 架构示意：当前 IPC 热点

```
┌─────────────────────────────────────────────────────────┐
│                    Rust AppState                         │
├─────────────────────────────────────────────────────────┤
│  Events (3)          │  Polling targets (7+)            │
│  · chat-message      │  · getActiveRoles      1s ×2     │
│  · dept-log          │  · getRoundMetrics     3s        │
│  · plan-update       │  · getWorkflowState    3s        │
│                      │  · getPendingApprovals 3s        │
│                      │  · getWorkflowGraph    5s        │
│                      │  · getContextStats    10s        │
│                      │  · getTokenStats      30s        │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
                   React Dashboard
```

---

## 建议实施路线图

| 阶段 | 内容 | 预期收益 |
|------|------|----------|
| **Sprint 1** | P0 四项 + 缓存 context_config | 正确性修复 + 热路径 I/O 减半 |
| **Sprint 2** | 事件驱动替代轮询 + Project 状态刷新 + 类型对齐 | IPC 负载降 60%+，UI 进度准确 |
| **Sprint 3** | Agent runner 收敛 + 共享 HTTP client + 死代码清理 | 维护成本降，连接复用 |
| **Sprint 4** | 持久化优化 + tool 拆分 + Actor 改进 | 长会话稳定性 |
| **Sprint 5** | 测试/CI 补全 + 构建优化 | 回归保障 + 发布体积 |

---

## 附录：关键文件索引

| 模块 | 路径 |
|------|------|
| Actor 主循环 | `src-tauri/src/actor/mod.rs` |
| 消息入口 | `src-tauri/src/commands/workflow.rs` |
| LLM Session | `src-tauri/src/api/session.rs` |
| 工具循环 | `src-tauri/src/api/control.rs` |
| Agent 共享逻辑 | `src-tauri/src/agent/runner.rs` |
| 部门 Agent 模板 | `src-tauri/src/agent/libushangshu/mod.rs`（代表） |
| 内阁 | `src-tauri/src/agent/neige/mod.rs` |
| 工具分发 | `src-tauri/src/tool/mod.rs` |
| 运行时配置 | `src-tauri/src/config/mod.rs` |
| 项目持久化 | `src-tauri/src/storage/shuji_dir.rs` |
| Token 统计 | `src-tauri/src/token_tracker.rs` |
| 前端 Dashboard | `src/pages/ProjectDashboard.tsx` |
| 前端类型 | `src/types.ts` |
| IPC 封装 | `src/api.ts` |
| CI | `.github/workflows/check.yml` |

---

*生成方式：静态代码审查 + 关键路径 grep 验证。实施前请对具体改动点再跑 `cargo test --tests` 与 `npm run lint` 确认。*
