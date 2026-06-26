# 枢机后端代码研读计划书

> **目标**：在添加任何新功能之前，系统性地读懂 Rust 后端。  
> **原则**：只读、只跑测试、只记笔记；不改业务代码、不加 feature。  
> **预计周期**：4～6 周（每天 1～2 小时）或 2～3 周（每天 3～4 小时）

---

## 一、研读范围

### 必读（核心链路）

```
lib.rs → commands/workflow/send.rs → actor/spawn.rs
      → agent/*/mod.rs → api/control.rs → api/session/
      → tool/dispatch.rs → tool/documents/ → models/
```

### 暂缓（第二阶段以后再读）

| 模块 | 原因 |
|------|------|
| `pipeline/` | 在核心 Actor 链路之上，依赖前面的理解 |
| `validate/` | 交付验证，独立子系统 |
| `workflow/graph.rs` | 可视化辅助，非执行主路径 |
| `pricing.rs`、`metrics/` | 统计展示 |
| `playbook/`、`precepts/` | 辅助 playbook |

### 不必深读

- `src/` 前端 TypeScript（已通过 `api.ts` 的 `invoke` 知道边界即可）
- `agent/*/prompt.md`、`skills/*.md`（行为说明，第二阶段配合 Agent 读）

---

## 二、学前准备（第 0 周，约 3～5 天）

### 2.1 Rust 最小语法包（有 C++ 基础）

按顺序阅读 [Rust 程序设计语言（中文）](https://kaisery.github.io/tran/rust-book-cn/) 以下章节即可，**不必全书通读**：

| 章节 | 对应本项目的用法 |
|------|------------------|
| 第 3 章 通用编程概念 | 函数、控制流 |
| 第 4 章 所有权 | `String` vs `&str`，为什么到处有 `.clone()` |
| 第 6 章 enum 和 match | `Option`、`Result`、`LoopDecision` |
| 第 9 章 trait | `Agent` trait、`async_trait` |
| 第 10 章 生命周期（浏览） | 看懂 `&self`、`&AgentInput` 即可 |
| 第 16 章 fearlessness concurrency（浏览） | `Arc`、`Mutex`、`AtomicBool` |

### 2.2 环境验证

```bash
cd shuji-app
npm install
cd src-tauri
cargo test --lib                          # 单元测试应全部通过
cargo test --test actor_test -- --nocapture  # 第一个「说明书」测试
```

### 2.3 工具配置

- IDE：安装 `rust-analyzer` 扩展
- 善用：`Go to Definition`（F12）、`Find References`（Shift+F12）
- 打开 `shuji-app/docs/ARCHITECTURE.md` 和仓库根目录 `CLAUDE.md`（或 `ONBOARDING.md`）作为长期参考

### 2.4 研读笔记模板（建议建 `notes/backend/` 目录）

每读完一个模块，填一张卡片：

```markdown
## 模块：api/control.rs
- **职责**：（一句话）
- **入口函数**：
- **依赖谁**：
- **被谁调用**：
- **关键数据结构**：
- **我还没懂的**：
- **验证方式**：（跑了哪个 test）
```

---

## 三、分阶段计划

### 阶段 1：找到入口，跟完一条消息（第 1 周）

**目标**：能口头描述「用户按发送后，代码怎么跑到内阁」。

#### 阅读顺序

| 顺序 | 文件 | 关注点 |
|------|------|--------|
| 1 | `src-tauri/src/main.rs` | 程序入口，调用 `lib::run()` |
| 2 | `src-tauri/src/lib.rs` | 模块树、`AppState`、`invoke_handler` 注册了哪些命令 |
| 3 | `commands/project.rs` | `AppState` 字段含义：`cancel_flag`、`actor_system`、`chat_history` |
| 4 | `commands/workflow/send.rs` | **`send_message` 完整流程**（最重要） |
| 5 | `commands/workflow/bootstrap.rs` | Actor 系统如何懒启动 |
| 6 | `models/chat.rs`、`models/role.rs` | 聊天消息、九个角色枚举 |

#### 动手验证

```bash
# 读 actor_test 里「消息怎么进信箱」的用例
cargo test --test actor_test test_actor_receives_task -- --nocapture
```

#### 阶段 1 过关标准（自检）

- [ ] 能画出：`前端 invoke('send_message')` → `AppState` → `ActorMessage` → 内阁 mailbox
- [ ] 知道 `chat-message`、`dept-log` 事件在哪里 `emit`
- [ ] 知道 `cancel_processing` 和 `cancel_flag` 的关系

#### 推荐阅读顺序图

```
main.rs
  └─ lib.rs::run()
       └─ AppState (project.rs)
            └─ send_message (workflow/send.rs)
                 ├─ bootstrap::start_actor_system
                 └─ 内阁 mailbox.send(ActorMessage)
```

---

### 阶段 2：Actor 系统 — 部门间怎么传话（第 2 周）

**目标**：理解 Push 式 mpsc 模型，以及 `route_to` 如何触发下一个部门。

#### 阅读顺序

| 顺序 | 文件 | 关注点 |
|------|------|--------|
| 1 | `actor/mod.rs` | `ActorMessage`、`FastMessage`、`DeptLogEntry` |
| 2 | `actor/spawn/mod.rs` | **`run_actor` 主循环**（子模块：`mailbox` / `exec_loop` / `output` / `neige`） |
| 3 | `actor/routing.rs` | 消息路由辅助 |
| 4 | `agent/trait.rs` | `AgentInput` / `AgentOutput` / `LoopDecision` |
| 5 | `api/control.rs`（前半） | `RouteTo`、`RouteMsgType`；先不读 `AgentController::run` 全文 |

#### 动手验证

```bash
cargo test --test actor_test -- --nocapture
cargo test --test session_control_test -- --nocapture
```

重点看 `tests/actor_test.rs` 里：

- mpsc 消息顺序
- `Interrupt` 后信箱是否清空
- `cancel_agent` 是否只影响目标部门

#### 阶段 2 过关标准

- [ ] 能解释：`route_to(to="工部尚书", document_id=...)` 之后发生了什么
- [ ] 能区分：`cancel_flag`（全局）vs `FastMessage::Interrupt`（单部门）
- [ ] 能解释：`paused_for_decision` 和 `<options>` 朱批等待的关系

---

### 阶段 3：LLM 会话层 — Session 与 AgentController（第 3 周）

**目标**：理解「一次 tool call 循环」怎么转起来。这是后端**最核心**的一层。

#### 阅读顺序

| 顺序 | 文件 | 关注点 |
|------|------|--------|
| 1 | `api/client.rs` | HTTP 调用，Anthropic vs OpenAI 自动检测 |
| 2 | `api/session/mod.rs` | `Session::step()`、消息历史、`PersistedContext` |
| 3 | `api/session/persisted_context.rs` | 三层 prompt 如何保存/恢复 |
| 4 | `api/control.rs`（全文） | **`AgentController::run` 驱动循环** |
| 5 | `api/compact/mod.rs`（浏览） | 何时触发 `[对话摘要]` 压缩 |
| 6 | `config/mod.rs`（浏览） | `max_tool_iterations`、`timeout_secs` 等从哪来 |

#### 建议阅读法

不要从头到尾线性读 `control.rs`（约 800 行）。按这个顺序跳读：

1. `AgentController::run` 的 `while` 循环结构
2. 每次迭代：`session.step()` → 处理 `ToolCalls` → `execute_named_tool`
3. `watchdog` 相关：同一工具重复、连续错误 5 次停止
4. `CompactFn` / `CheckpointFn` 回调触发点

#### 动手验证

```bash
cargo test --test session_test -- --nocapture
cargo test --test session_control_test -- --nocapture
cargo test --test watchdog_behavior_test -- --nocapture
```

#### 阶段 3 过关标准

- [ ] 能画出：`step()` → `tool_call` → `dispatch` → `tool_result` → 再 `step()` 的循环
- [ ] 知道 `finish_reason=length` 时为什么会减半 `max_tokens` 重试
- [ ] 知道 watchdog 注入的干预提示出现在哪里

---

### 阶段 4：工具与文档 — 部门的手脚（第 4 周）

**目标**：理解工具注册、调度、安全门禁，以及 `.shuji/` 文档系统。

#### 阅读顺序

| 顺序 | 文件 | 关注点 |
|------|------|--------|
| 1 | `tool/registry.rs` | 工具分组：`doc_inspect_tools`、`file_write_tools` 等 |
| 2 | `tool/dispatch.rs` | **`execute_named_tool` 总调度** |
| 3 | `tool/path.rs` | 路径安全：`resolve_scoped_path` |
| 4 | `tool/documents/crud.rs` | 文档 CRUD、YAML frontmatter |
| 5 | `tool/documents/approval.rs` | 朱批门禁：`route_to` / `append_document` 前检查 |
| 6 | `tool/file_ops/` | 读/写/补丁文件 |
| 7 | `tool/command_ops.rs` | 命令执行安全：`check_safe_command` |
| 8 | `storage/shuji_dir.rs` | `.shuji/` 目录布局 |

#### 动手验证

```bash
cargo test --test tool_test -- --nocapture
cargo test --test path_security_test -- --nocapture
cargo test --test document_test -- --nocapture
```

#### 阶段 4 过关标准

- [ ] 能列出：内阁、工部、礼部分别有哪些工具（查 `registry.rs`）
- [ ] 能解释：未朱批的 `plan` 文档为什么阻塞下游
- [ ] 能解释：`../` 路径遍历为什么会被拒绝

---

### 阶段 5：Agent 实现 — 从一个部门推广到全部（第 5 周）

**目标**：搞懂一个完整 Agent 后，其余部门是「同一模板 + 不同 prompt/工具」。

#### 推荐阅读顺序（由易到难）

| 顺序 | Agent | 文件 | 为什么先读它 |
|------|-------|------|--------------|
| 1 | 工部尚书 | `agent/gongbushangshu/mod.rs` | 有代表性的 coding agent + batch plan |
| 2 | 内阁 | `agent/neige/mod.rs` | 最复杂：skill 切换、routing、pause |
| 3 | 中书令 | `agent/zhongshuling/mod.rs` | skill 自管理范例 |
| 4 | 尚书令 | `agent/shangshuling/mod.rs` | 执行调度、chain 注入 |
| 5 | 其余四部 | `libushangshu`、`bingbushangshu` 等 | 结构类似，快速浏览 |
| 6 | 共享框架 | `agent/runner.rs` | 非内阁部门的 compact/checkpoint 复用 |

配合阅读（不算代码，但决定行为）：

- `agent/neige/prompt.md`
- `agent/gongbushangshu/prompt.md`
- `agent/neige/skills/workflow_demo.md`（最短工作流）

#### 动手验证

```bash
cargo test --test workflow_demo_test -- --nocapture
cargo test --test workflow_mock_test -- --nocapture
```

#### 阶段 5 过关标准

- [ ] 能解释：工部 `submit_plan` → `after_execute(Continue)` → 下一 batch 的循环
- [ ] 能解释：内阁 `<skill>workflow_demo</skill>` 如何注入 session
- [ ] 能对比：吏部（只写文档）和工部（写代码）的工具差异

---

### 阶段 6：横切子系统（第 6 周，选读深化）

**目标**：理解审计、Checkpoint、Pipeline、Workflow Profile——它们在主链路的什么位置插入。

#### 6A 审计与 Checkpoint（建议读）

| 文件 | 职责 |
|------|------|
| `audit/mod.rs` | 审计日志、RefIndex、血缘、diff |
| `storage/checkpoint.rs` | `.shuji/.git/` 隔离仓库、快照恢复 |
| `commands/checkpoint.rs` | Tauri 命令入口 |

```bash
cargo test --test audit_test -- --nocapture
cargo test --test checkpoint_test -- --nocapture
```

#### 6B Pipeline 引擎（第二优先级）

| 文件 | 职责 |
|------|------|
| `pipeline/mod.rs` | PlanRuntime 磁盘状态 |
| `pipeline/engine.rs` | 步骤执行、approval_gate、resume |
| `pipeline/schema.rs` | plan JSON 校验 |

```bash
cargo test --test pipeline_test -- --nocapture
```

#### 6C Workflow Profile（第三优先级）

| 文件 | 职责 |
|------|------|
| `workflow/mod.rs`、`workflow/state.rs` | profile 状态持久化 |
| `workflow/profiles/*.yaml` | demo / bugfix / greenfield 等配置 |
| 内阁里的 `GateEngine` 调用点 | `neige/mod.rs` 内搜索 `gate` |

```bash
cargo test --test config_test -- --nocapture
```

#### 阶段 6 过关标准

- [ ] 能解释：Checkpoint 恢复时 git stash + checkout 的顺序
- [ ] 能解释：`send_message` 开头检查 `PlanRuntime::load_from` 的原因
- [ ] 知道 `audit.jsonl` 里一条记录长什么样（跑过一次 demo 后打开看）

---

## 四、端到端实操（贯穿第 3～5 周）

在读代码的同时，用**真实运行 + 日志对照**加深理解。这不算是「加新功能」，是验证理解。

### 4.1 跑 Demo 并对照代码

```bash
cd shuji-app
npm run tauri dev
```

1. 创建 demo 项目或空目录项目
2. 发送：`请先载入 workflow_demo，写一个 hello.py`
3. 同时观察终端 `[actor]`、`[api] tool_call` 日志
4. 对照 `send.rs` → `spawn.rs` → `gongbushangshu/mod.rs` 走读

### 4.2 测试流程文档

按 `shuji-app/docs/TEST_FLOW.md` 走一遍完整流程（可选，时间较长）。

### 4.3 磁盘产物对照

任务跑完后打开项目目录：

```
.shuji/
├── designs/      ← 中书令产出
├── reviews/      ← 门下侍中产出
├── tasks/        ← 尚书令/工部任务
├── audit.jsonl   ← 每次文档操作
├── chat.jsonl    ← 聊天持久化
├── context/      ← 各部门压缩上下文
└── state.json    ← 项目里程碑
```

每看到一个文件，回头找「是哪个 tool 写的」。

---

## 五、每周节奏建议

| 天 | 活动 | 时间 |
|----|------|------|
| 周一～周三 | 按阶段读代码 + 做笔记 | 1～2h/天 |
| 周四 | 跑对应阶段的 `cargo test` | 1h |
| 周五 | 画一张流程图（纸笔或 Excalidraw） | 30min |
| 周末 | `tauri dev` 实操 + 日志对照（可选） | 1～2h |

---

## 六、研读期间的纪律

1. **禁止**：加 feature、改 prompt、改 workflow 行为（除非 fix 阻塞你读代码的 typo）
2. **允许**：记笔记、画流程图、在测试里加 `println!` 或 `--nocapture` 观察（读完删掉）
3. **卡住时**：先跑 test，再 `grep` 函数名，不要硬啃 800 行文件
4. **提问清单**：每周整理「我还没懂的」≤ 5 条，集中解决

---

## 七、总过关标准（全部阶段完成后）

完成以下任意 **3 项** 即可认为「后端已入门，可以开始小改动」：

- [ ] **白板测试**：不看代码，15 分钟内画出从 `send_message` 到工部 `write_file` 的完整序列图
- [ ] **定位测试**：给定一个 bug 描述（如「朱批后 route_to 失败」），能在 10 分钟内定位到 `tool/documents/approval.rs` 或 `dispatch.rs`
- [ ] **测试阅读**：能向他人讲解 `actor_test` 中任意一个测试在验证什么
- [ ] **配置调试**：能说明 `config.toml` 里改哪个字段可以延长 tool 迭代上限
- [ ] **Mock 流程**：能解释 `workflow_demo_test` 不用真 API 也能跑通的原因（`MockActorHarness`）

---

## 八、常见问题速查

| 困惑 | 去看 |
|------|------|
| 消息发给谁了？ | `actor/spawn.rs` 里 `route_to` 分支 |
| LLM 为什么又调了同一个工具？ | `api/control.rs` watchdog |
| 文档 ID 怎么来的？ | `tool/documents/crud.rs` |
| 配置为什么不生效？ | `config/mod.rs` 优先级：`config.local.toml` > `config.toml` |
| 前端怎么收到回复？ | `send.rs` 里 `emperor_tx` → `emit("chat-message")` |
| 和 C++ 最不像的地方？ | 所有权 + `async/await` + 没有 null（用 `Option`） |

---

## 九、下一步（读完以后）

按风险从低到高开始动手：

1. 改 `config.toml` 默认值 → 跑 test 验证
2. 改某个部门的 `prompt.md` 措辞 → demo 流程验证
3. 给现有模块补一个 integration test
4. 修一个你已定位的小 bug
5. 再考虑新 feature

---

## 附录 A：文件优先级一览

```
P0 必须读懂
├── lib.rs
├── commands/workflow/send.rs
├── actor/spawn.rs
├── agent/trait.rs
├── api/control.rs
├── api/session/mod.rs
└── tool/dispatch.rs

P1 重要
├── agent/neige/mod.rs
├── agent/gongbushangshu/mod.rs
├── tool/documents/
├── tool/registry.rs
├── config/mod.rs
└── commands/project.rs (AppState)

P2 扩展
├── audit/mod.rs
├── storage/checkpoint.rs
├── pipeline/engine.rs
└── workflow/

P3 以后再说
├── validate/
├── pricing.rs
└── metrics/
```

## 附录 B：推荐 grep 命令

```bash
cd shuji-app/src-tauri

# 谁调用了 execute_named_tool？
rg "execute_named_tool" src/

# route_to 工具在哪定义？
rg "route_to" src/tool/

# 某个角色在哪 spawn？
rg "Role::Gongbushangshu" src/

# 内阁 skill 检测逻辑
rg "skill" src/agent/neige/
```

---

*文档版本：2026-06-17 · 适用 commit 附近的枢机后端结构*
