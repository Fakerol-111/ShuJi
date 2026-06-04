# 枢机（ShuJi）架构文档

> 本文档描述当前实际实现的架构。与 `mailbox_design.md`（未来设计）不同，本文档反映**运行时代码的真实行为**。

## 消息流模型：Actor + mpsc Push

### 核心架构

```
皇帝 ──send_message──→ ActorSystem ──→ 内阁(actor)
                                          │
                                     route_to (文档ID)
                                          │
                    ┌─────────────────────┼────────────────────┐
                    │                     │                    │
               中书令(actor)       门下侍中(actor)       尚书令(actor)
                    │                                      │
               design skills                         dispatch to 六部
                    │                                      │
               ┌────┴────┐                    ┌───────┬────┼────┬───────┐
               │         │                    │       │    │    │       │
           门下侍中    尚书令              吏部   兵部  工部  刑部  礼部
           (审查)     (调度)
```

### 部门列表（10 个角色）

| 部门 | 职责 |
|------|------|
| 内阁 | 皇帝入口，skill 检测，工作流分发 |
| 中书令 | 方案设计（3 skills：overall_design / phase_plan / phase_design） |
| 门下侍中 | 审查（2 skills：review_overall / review_phase） |
| 尚书令 | 执行调度 |
| 吏部尚书 | 详细设计 |
| 兵部尚书 | 测试编写 + 接口契约 |
| 工部尚书 | TDD 编码 + 分批计划循环 |
| 刑部尚书 | 测试验证 |
| 礼部尚书 | 规范检查 |
| 户部 | token 统计（仅数据收集，无 actor 循环） |

### 消息传递

- **ActorSystem**（`actor/mod.rs`）：管理所有 actor 的生命周期和消息路由
- **mpsc channel**（`UnboundedSender<ActorMessage>`）：actor 之间通过 `route_to` 工具发送消息，每个 actor 有一个 `mpsc::UnboundedReceiver` 信箱
- **FastMessage**（`actor/mod.rs`）：高优先级中断通道，通过独立的 `mpsc::UnboundedReceiver<FastMessage>` 发送中断信号，绕过正常消息队列
- **文档 ID 通信**：部门间不传递完整上下文，只传递 `.shuji/` 下的文档 ID，由接收方读取文档获取详情

### 消息流路径

1. 用户发送消息 → `send_message` Tauri command → ActorSystem → 内阁 actor
2. 内阁 actor 运行 skill 检测循环，加载对应 skill（`[skill: name]` 注入 session）
3. 内阁调用 `route_to` 发送文档 ID 给目标部门
4. 目标部门 actor 从自己的 mpsc channel 接收消息，开始执行
5. 执行结果通过 `emperor_tx`（→ `chat-message` 事件）回传给前端

### 中断机制

- **取消**：`AtomicBool` 检查 + `FastMessage::Interrupt` 通道
- **FastMessage**：内阁可通过 `cancel_agent` 工具精确中断指定部门，无需全局取消

### Session / AgentController 分离

```
Session (api/session.rs)              AgentController (api/control.rs)
┌─────────────────────────┐          ┌──────────────────────────────┐
│ 纯 LLM 层                │          │ 驱动循环层                    │
│ - 消息历史管理            │ ←step()→ │ - 调用 session.step()        │
│ - API 调用              │          │ - 执行工具                    │
│ - 自动重试 (length)      │          │ - 检查 cancel/interrupt      │
│ - PersistedContext 持久化│          │ - Watchdog 诊断              │
│ - 消息净化 (sanitize)    │          │ - CompactFn / CheckpointFn   │
└─────────────────────────┘          └──────────────────────────────┘
```

### 上下文压缩

所有部门的上下文采用**单层压缩**：当 `context_messages` token 数超过阈值时，较早的非 skill 消息被压缩为 `[对话摘要]`。skill 消息（`[skill: ...]`）在压缩前后剥离并重新追加，保持缓存命中率。

### 文档系统

- 文档存储于 `.shuji/` 目录下，按类型分目录（`designs/`、`reviews/`、`tasks/`、`contracts/` 等）
- 使用 YAML frontmatter 元数据，自动 ID 生成
- `plan`/`revw` 类型文档需皇帝朱批（`pending_approvals.json`），下游 `route_to` 和 `append_document` 硬门禁

## 关键文件

| 文件 | 用途 |
|------|------|
| `actor/mod.rs` | Actor 系统：run_actor、消息路由、FastMessage 中断 |
| `api/session.rs` | LLM 会话管理、PersistedContext |
| `api/control.rs` | AgentController 驱动循环、watchdog、checkpoint |
| `api/client.rs` | HTTP 客户端（兼容 Anthropic / OpenAI 格式） |
| `api/compact/mod.rs` | 上下文压缩 |
| `tool/mod.rs` | 工具调度、resolve_scoped_path、命令安全 |
| `tool/documents.rs` | 文档 CRUD、朱批审批 |
| `agent/neige/` | 内阁 agent：skill 检测循环、prompt |
| `agent/*/` | 其他 9 个部门 agent |
| `config/mod.rs` | RuntimeConfig 加载 |
| `config.toml` | 运行时配置（限流、超时、阈值） |
| `context_config.json` | 每角色上下文压缩配置 |

| `workflow/` | Workflow Profile 系统：config/profile/resolver/gate/chain/state |

## Workflow Profile 系统

> Phase A 最小可用切片。使用 `workflow_config.json` + Rust struct 内置 profile，向后兼容。

### 核心概念

**Intent（任务意图）** × **Governance（治理强度）** 两轴决定工作流行为：

| Intent | 说明 |
|--------|------|
| `auto` | 默认。使用 routing.rs 关键词启发式推断，Low 置信度时强制 `<options>` |
| `greenfield_standard` | 绿场新功能：完整设计→审查→执行链 |
| `brownfield_optimize` | 存量优化：跳过需求展开和门下审查 |
| `bugfix` | 缺陷修复：直路由到工部/尚书令 |
| `demo` | 快速原型：最轻量流程 |

| Governance | overlay 行为 |
|------------|-------------|
| `full` | 无额外门禁 |
| `standard` | 无额外门禁（默认） |
| `fast` | 追加禁 expand_requirements + 禁门下侍中 |
| `audit` | （Phase B 完善） |

### 架构

```
workflow_config.json ─→ WorkflowResolver ─→ ActiveProfile
                        (auto/routing 推断)     │
                          ↓                     ↓
                    GateEngine            ChainEngine
                    (工具/路由拦截)         (尚书令执行链注入)
                          ↓                     ↓
                    内阁 exec              尚书令 session
```

### 组件职责

| 组件 | 职责 |
|------|------|
| `config.rs` | 读写 `.shuji/workflow_config.json`，默认 auto+standard |
| `profile.rs` | 4 个内置 WorkflowProfile + Governance overlay |
| `resolver.rs` | WorkflowResolver：合并 intent + governance + routing 启发式 |
| `gate.rs` | GateEngine：工具名 + route_to 目标拦截（替代旧硬编码 demo/bugfix gate） |
| `chain.rs` | ChainEngine：greenfield_full / brownfield_patch 执行链 |
| `state.rs` | 读写 `.shuji/workflow_state.json`，记录当前 profile/stage |

### GateEngine 门禁规则

| Profile | 禁 expand_requirements | 禁 route_to |
|---------|----------------------|-------------|
| greenfield_standard | — | — |
| brownfield_optimize | ✓ | 门下侍中 |
| bugfix | ✓ | 中书令、门下侍中 |
| demo | ✓ | 中书令、门下侍中 |

Governance Fast overlay 追加禁 expand_requirements + 门下侍中（greenfield 上生效）。

### 集成点

- **内阁（`neige/mod.rs`）**：execute 开头调用 WorkflowResolver；exec 闭包前调用 GateEngine
- **尚书令（`shangshuling/mod.rs`）**：session 创建后注入 ChainEngine 执行链

### 保留旧组件

- `skills/workflow_*.md` 内容不变；profile.cabinet_skill 引用它们
- `workflow_preset.json` 双写兼容；governance 值与之对齐
- 无 `workflow_config.json` 时行为与改前一致（auto + routing + 无 GateEngine）

| 特性 | mailbox_design.md（未来） | 当前实现 |
|------|--------------------------|---------|
| 消息模型 | Pull 式调度器轮询 | Push 式 mpsc channel |
| 信箱 | 快信箱 + 慢信箱 | 单一 mpsc + FastMessage 通道 |
| 消息路由 | 调度器统一分发 | Actor 间直接 route_to |
| 消息格式 | JSON 结构（含 hop_count） | ActorMessage 枚举 + 文档 ID |
| 阶段并行 | 内阁判断阶段条件 | 尚书令 dispatch |
| 户部 | token 报告信箱 | 仅数据收集（无 actor） |
