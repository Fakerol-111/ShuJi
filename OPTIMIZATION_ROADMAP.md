# 枢机（ShuJi）产品优化路线图

> **用途**：供 Agent 逐项阅读、实现、验收。每完成一项，将对应任务的 `状态` 改为 `done`，并在 `CHANGELOG.md` 简要记录。
>
> **执行原则**：
> 1. 严格按 **优先级 P0 → P1 → P2 → P3** 顺序推进；同优先级内按任务编号顺序。
> 2. 每项任务独立 PR/commit，避免大范围混合改动。
> 3. 改动前先读「相关文件」列出的代码；改动后跑 CI 等价命令（见文末「验证清单」）。
> 4. 不要过度设计：每项任务只解决文档描述的问题，不顺带重构无关模块。
> 5. 用户未明确要求时，**不要**修改 Agent prompt 的中英混用策略，除非任务明确要求。

---

## 目录

| 编号 | 优先级 | 任务 | 状态 |
|------|--------|------|------|
| [OPT-001](#opt-001-强化-demo-上手漏斗) | P0 | 强化 Demo 上手漏斗 | done |
| [OPT-002](#opt-002-内阁自动选择-workflow) | P0 | 内阁自动选择 workflow | done |
| [OPT-003](#opt-003-流程地图置顶与待办可见) | P0 | 流程地图置顶与待办可见 | done |
| ~~OPT-004~~ | ~~P0~~ | ~~启动前成本与时间预估~~ | ~~removed~~ |
| [OPT-005](#opt-005-模型分级预设三档) | P1 | 模型分级预设（三档） | done |
| [OPT-006](#opt-006-刑部与礼部并行执行) | P1 | 刑部与礼部并行执行 | pending |
| [OPT-007](#opt-007-任务级短路规则) | P1 | 任务级短路规则 | done |
| [OPT-008](#opt-008-朱批文档-diff-体验) | P1 | 朱批文档 diff 体验 | done |
| [OPT-009](#opt-009-失败恢复引导-ui) | P2 | 失败恢复引导 UI | pending |
| [OPT-010](#opt-010-前端测试与版本对齐) | P2 | 前端测试与版本对齐 | pending |
| [OPT-011](#opt-011-架构文档与实现一致) | P2 | 架构文档与实现一致 | done |
| [OPT-012](#opt-012-shuji-配置导出导入) | P3 | `.shuji` 配置导出/导入 | pending |
| [OPT-013](#opt-013-外部编辑器桥接) | P3 | 外部编辑器桥接 | pending |
| [OPT-014](#opt-014-可选现代命名模式) | P3 | 可选「现代命名」模式 | pending |
| [OPT-015](#opt-015-审计日志血源追溯) | P2 | 审计日志 + 血缘追溯 | done |
| [OPT-016](#opt-016-朝报时间线) | P2 | 朝报时间线聚合 | done |
| [OPT-017](#opt-017-交付报告生成) | P2 | 交付报告自动生成 | done |

---

## OPT-001: 强化 Demo 上手漏斗

**优先级**：P0  
**状态**：done  
**预估工作量**：中（前端 + 后端小改）

### 背景

已有 `create_demo_project` 与 WorkspaceSelect 上的「体验枢机 — 5 分钟上手」按钮，但 Demo 进入 ProjectDashboard 后 **不会自动发送指令**；用户仍需自己输入才能看到部门运转。ProjectDashboard 内有 `handleDemoProject` 会自动发指令，但 WorkspaceSelect 路径未复用该逻辑。

### 当前实现

- Demo 创建：`shuji-app/src-tauri/src/commands/demo.rs`
- 工作区入口：`shuji-app/src/pages/WorkspaceSelect.tsx`（仅 navigate，无 auto-send）
- Dashboard 内 Demo：`shuji-app/src/pages/ProjectDashboard.tsx` 的 `handleDemoProject`（有 auto-send）

### 目标行为

1. 用户点击「体验枢机」后，进入 Dashboard **自动发送** 预设指令（与 `handleDemoProject` 一致）。
2. 首次 Demo 期间显示 **分步引导 overlay**（3–4 步）：① 看部门状态栏 ② 看左侧文档树 ③ 等工部修 bug ④ 看测试结果。
3. Demo 结束后弹出 **小结卡片**：耗时、Token 消耗（从 `get_token_stats` / `get_round_metrics` 读取）、建议下一步（打开真实项目 / 调整参与度 `/level-2`）。

### 相关文件

```
shuji-app/src/pages/WorkspaceSelect.tsx
shuji-app/src/pages/ProjectDashboard.tsx
shuji-app/src/hooks/useChat.ts
shuji-app/src/api.ts
shuji-app/src-tauri/src/commands/demo.rs
```

### 实现要点

- 用 `sessionStorage` 或 navigate state 传递 `{ demo: true }` 标记，Dashboard mount 时检测并触发 `handleSend(...)`。
- 引导 overlay 用独立组件 `DemoTour.tsx`，`localStorage` 键 `shuji_demo_tour_done` 控制只显示一次。
- 小结卡片在检测到 Demo 工作目录（`temp/shuji-demo/calc_demo`）且 workflow 空闲时展示。

### 验收标准

- [ ] 新用户从 WorkspaceSelect 点 Demo，30 秒内能在 UI 看到至少一个部门活跃。
- [ ] 引导 overlay 可跳过，且不会重复骚扰（除非清 localStorage）。
- [ ] Demo 小结展示 Token 数字（可为 0，但接口必须调用）。
- [ ] 不破坏「打开已有项目」路径。

### 测试

- 手动走通 Demo 全流程（见 `shuji-app/TEST_FLOW.md` Step 0 简化版）。
- `npm run lint` + `cargo test --tests` 通过。

### 依赖

无。

---

## OPT-002: 内阁自动选择 workflow

**优先级**：P0  
**状态**：done  
**预估工作量**：中（内阁 prompt + 规则代码）

### 背景

用户常需手动写「请先载入 workflow_complex」才能走对流程。内阁本应据需求复杂度自动选 skill，降低认知负担。

### 当前实现

- Skill 检测循环：`shuji-app/src-tauri/src/agent/neige/mod.rs`（`extract_skill` + `load_skill`）
- Skill 定义：`shuji-app/src-tauri/src/agent/neige/skills/workflow_*.md`
- 内阁 base prompt：`shuji-app/src-tauri/src/agent/neige/prompt.md`

### 目标行为

内阁收到用户消息后，**无需用户指定 skill 名**，按规则自动加载：

| 条件（启发式，可组合） | 自动 skill |
|------------------------|------------|
| 单文件 / 明确 bugfix / 测试已存在且失败 | `workflow_demo` 或 `workflow_bugfix` |
| 2–5 文件、逻辑 straightforward | `workflow_simple` |
| 新功能、多模块、需设计文档 | `workflow_standard` |
| 多阶段、ERP 级、用户描述含「系统」「平台」 | `workflow_complex` |
| 用户明确说 refactor / optimize / audit | 对应 skill |

仅在 **置信度低**（如两种 workflow 都合理）时输出 `<options>` 让用户选，而非默认 complex。

### 相关文件

```
shuji-app/src-tauri/src/agent/neige/mod.rs
shuji-app/src-tauri/src/agent/neige/prompt.md
shuji-app/src-tauri/src/agent/neige/skills/*.md
shuji-app/src-tauri/src/agent/util.rs
shuji-app/src-tauri/tests/workflow_demo_test.rs  # 扩展或新增测试
```

### 实现要点

- **优先**在 Rust 层做轻量规则预判断（关键词、文件数、`list_dir` 结果），再注入 hint 给 LLM；避免纯靠 prompt。
- 规则函数建议放 `agent/util.rs` 或 `agent/neige/routing.rs`，便于单测。
- 用户显式 `<skill>xxx</skill>` 或消息中含 skill 名时，**尊重用户覆盖**自动判断。
- 更新 `HelpDrawer.tsx`：说明「系统会自动选流程，也可手动指定 skill」。

### 验收标准

- [ ] 「修复 calc.py 的 bug」类输入自动走 demo/bugfix，不经过中书令设计（与 OPT-007 协同但不重复实现）。
- [ ] 「做一个 ERP 管理系统」自动走 complex 或弹出选项，**不会**默认 simple。
- [ ] 新增至少 3 个单元/集成测试覆盖路由启发式（mock 输入 → 预期 skill）。
- [ ] 现有 `workflow_demo_test` 仍通过。

### 依赖

- OPT-007 与之有重叠；建议 **先完成 OPT-002 的自动判断**，OPT-007 做硬短路 gate。

---

## OPT-003: 流程地图置顶与待办可见

**优先级**：P0  
**状态**：done  
**预估工作量**：中

### 背景

`WorkflowTimeline` 组件已存在但 **未接入** ProjectDashboard。用户难以回答「系统在等什么？」——尤其是朱批阻塞时。

### 当前实现

- 组件（未使用）：`shuji-app/src/components/WorkflowTimeline.tsx`
- 阶段数据模型：`shuji-app/src-tauri/src/models/project.rs`（`PhaseSnapshot`）
- 项目状态：`.shuji/state.json`，前端 `Project` type：`shuji-app/src/types.ts`
- 待审批：`shuji-app/src-tauri/src/tool/documents.rs` → `pending_approvals.json`
- 概览面板：`shuji-app/src/components/ProjectOverview.tsx`

### 目标行为

1. 在 Dashboard **主区域顶部**（或 Sidebar 顶部固定区）展示：
   - 整体进度条 + 各阶段设计/执行状态（复用 `WorkflowTimeline` 或重写更贴合设计系统的样式）。
   - **当前阻塞原因** badge：如「等待朱批：plan_003」「工部执行中（第 2/4 批）」。
2. 点击阻塞项 → 跳转到对应文档（`DocPreview`）或部门日志。
3. 数据来源：
   - `project.phases` / `state.json`
   - 新增 Tauri command `get_workflow_status`（聚合 pending approval + active depts + planInfo）**或**纯前端组合现有 API。

### 相关文件

```
shuji-app/src/pages/ProjectDashboard.tsx
shuji-app/src/components/WorkflowTimeline.tsx
shuji-app/src/components/ProjectOverview.tsx
shuji-app/src/hooks/useActiveDepts.ts
shuji-app/src/hooks/useChat.ts          # planInfo
shuji-app/src-tauri/src/commands/workflow.rs
shuji-app/src-tauri/src/models/project.rs
shuji-app/src-tauri/src/tool/documents.rs
shuji-app/src/api.ts
```

### 实现要点

- 若 `project.phases` 为空（新项目），显示「尚未启动流程」而非空白。
- 样式与现有 Tailwind 设计 token 一致（`surface-paper`、`vermillion` 等），**不要**沿用 WorkflowTimeline 里硬编码的 `bg-white` / `text-gray-700`。
- 轮询或监听现有事件（`dept-log`、`chat-message`、`plan-update`）刷新状态，间隔 ≥ 2s。

### 验收标准

- [ ] WorkflowTimeline（或等价组件）在 Dashboard 可见。
- [ ] 存在 `in_review` 文档时，顶部显示「待朱批」及文档 ID/标题。
- [ ] 工部 plan 执行时显示批次进度（复用 `planInfo`）。
- [ ] 点击待办可定位到文档。

### 依赖

无。

---

## OPT-005: 模型分级预设（三档）

**优先级**：P1  
**状态**：done  
**预估工作量**：大

### 背景

当前支持按部门独立配置 API（`SetupPage` / `api_config.json`），但对普通用户过于复杂。应提供「经济 / 均衡 / 质量」三档一键映射。

### 当前实现

- 设置 UI：`shuji-app/src/pages/SetupPage.tsx`
- 配置读写：`shuji-app/src-tauri/src/commands/settings.rs`
- 各部门取配置：`shuji-app/src-tauri/src/lib.rs` / actor 启动处

### 目标行为

1. Setup 页增加 **预设档位** 选择器：
   - **经济**：审查/礼部/摘要 → 便宜模型；设计/工部 → 中等模型
   - **均衡**：全部用用户选的 DEFAULT 模型
   - **质量**：设计/工部/中书令 → 强模型；其余 → DEFAULT
2. 预设只改 **模型名**（及可选 max_tokens），不改 API URL/Key。
3. 预设可保存到 `api_config.json` 的 `preset: "economy" | "balanced" | "quality"` 字段。
4. 用户手动改某一部门配置后，预设标记为 `custom`。

### 相关文件

```
shuji-app/src/pages/SetupPage.tsx
shuji-app/src-tauri/src/commands/settings.rs
shuji-app/src/components/SettingsMenu.tsx
shuji-app/config.toml.template  # 可选：文档化默认映射
```

### 实现要点

- 映射表定义在后端（单一真相源），前端只展示档位名与说明。
- 切换预设时 **二次确认**（会覆盖部门 model 字段）。
- 保留现有 per-role 高级编辑能力。

### 验收标准

- [ ] 三档切换后，至少 2 个部门 model 字段按映射变化（可写 integration test 读 config）。
- [ ] 手动改一个部门后 preset 变为 custom。
- [ ] 旧版 `api_config.json` 无 preset 字段时默认 balanced。

### 依赖

无。

---

## OPT-007: 任务级短路规则

**优先级**：P1  
**状态**：done  
**预估工作量**：中

### 背景

即使用 `workflow_demo`，内阁仍可能误路由到中书令。需要在 **代码层** 硬门禁，不仅靠 prompt。

### 当前实现

- 路由工具：`route_to` in `shuji-app/src-tauri/src/tool/mod.rs` 或 `registry.rs`
- 审批门禁：`documents.rs` 中已有 pending approval 硬拦截范例
- Demo workflow skill：`shuji-app/src-tauri/src/agent/neige/skills/workflow_demo.md`

### 目标行为

当内阁当前 skill 为 `workflow_demo` / `workflow_bugfix` 且满足条件时：

1. **禁止** route_to 中书令、门下侍中（除非用户显式 override）。
2. **允许** 直路由工部 → 刑部。
3. 条件示例：工作区文件数 ≤ N、用户 intent 含 fix/bug/测试失败、无 `plan`/`dsgn` 待审批。

### 相关文件

```
shuji-app/src-tauri/src/tool/mod.rs
shuji-app/src-tauri/src/tool/registry.rs
shuji-app/src-tauri/src/agent/neige/mod.rs
shuji-app/src-tauri/tests/workflow_demo_test.rs
```

### 实现要点

- 在 `execute_named_tool("route_to", ...)` 内检查当前内阁 session skill 状态；skill 名可从最近 `[skill: xxx]` system 消息解析（`session.rs` 已有 `is_skill_message`）。
- 拦截时返回结构化错误 JSON，内阁 LLM 可读并改路由。
- 与 OPT-002 配合：自动选 demo + 硬短路。

### 验收标准

- [ ] Demo 项目自动指令不会触发中书令 actor 启动（看 dept log 或测试）。
- [ ] `workflow_standard` 下中书令路由 **不受影响**。
- [ ] 集成测试断言 route_to 拦截行为。

### 依赖

- OPT-002 建议先完成或同步。

---

## OPT-008: 朱批文档 diff 体验

**优先级**：P1  
**状态**：done  
**预估工作量**：中

### 背景

皇帝批文档时需要理解「改了什么」。当前 `DocPreview` 仅展示全文 + 朱批 banner，无版本对比。

### 当前实现

- 文档预览：`shuji-app/src/components/DocPreview.tsx`
- Checkpoint 含 git commit：`shuji-app/src-tauri/src/storage/checkpoint.rs`（或 commands/checkpoint.rs）
- 文档修改：`modify_document` / `append_document` in `documents.rs`

### 目标行为

1. 对 `in_review` 的 plan/revw 文档，DocPreview 增加 **「与上一版 diff」** 切换。
2. 上一版来源优先级：git 上一 commit 中同路径文件 → checkpoint 快照 → 无则隐藏 diff Tab。
3. 驳回时提供 **快捷理由模板**（下拉）：「缺少 API 定义」「缺少测试策略」「范围过大需拆分」→ 填入 comment textarea。
4. Diff 渲染：行级即可（可用 `diff` crate 后端算或前端 diff 库；后端已有 `diffy` 依赖）。

### 相关文件

```
shuji-app/src/components/DocPreview.tsx
shuji-app/src-tauri/src/commands/shuji_docs.rs
shuji-app/src-tauri/src/tool/documents.rs
shuji-app/src-tauri/Cargo.toml  # diffy 已存在
```

### 实现要点

- 新增 command `get_document_diff(project_dir, doc_path)` 返回 unified diff 或 `{ added, removed }` 结构。
- 注意 `.shuji/` 路径与 git track 状态；若文档未 git track，用 checkpoint json 内嵌内容。
- 批准流程不变。

### 验收标准

- [ ] 二次提交的 review 文档能看到与首版的 diff。
- [ ] 模板理由一键填入 comment。
- [ ] 无上一版时不显示 diff Tab（不报错）。

### 依赖

无。

---

## OPT-009: 失败恢复引导 UI

**优先级**：P2  
**状态**：pending  
**预估工作量**：中

### 背景

Checkpoint 系统已有（`CheckpointPanel`、`restore_checkpoint`），但用户遇到工部失败/Watchdog 停服时不知下一步。

### 当前实现

- Checkpoint UI：`shuji-app/src/components/CheckpointPanel.tsx`
- 恢复命令：`shuji-app/src-tauri/src/commands/checkpoint.rs`
- Watchdog 停止：`shuji-app/src-tauri/src/api/control.rs`

### 目标行为

1. 当 `chat-message` 或 dept log 含 `[系统]` 停止/失败/Watchdog 关键字时，显示 **恢复引导条**：
   - **从最近 checkpoint 恢复**（调 `restoreCheckpoint`）
   - **仅重跑当前部门**（发预设消息给内阁：`请仅重试{部门}，自 doc_id xxx 继续`）
   - **Git 回滚到 checkpoint commit**（可选，需确认对话框）
2. 引导条带最近 3 条 checkpoint 摘要（role + 描述 + 时间）。

### 相关文件

```
shuji-app/src/components/CheckpointPanel.tsx
shuji-app/src/pages/ProjectDashboard.tsx
shuji-app/src/hooks/useChat.ts
shuji-app/src-tauri/src/commands/checkpoint.rs
shuji-app/src-tauri/src/api/control.rs
```

### 实现要点

- 复用 CheckpointPanel 的 `listCheckpoints` 逻辑，抽 hook `useCheckpoints`。
- 「仅重跑当前部门」不实现新后端能力，用 **结构化 sendMessage** 即可（内阁 parse）。
- Git 回滚需调 `tauri-plugin-shell` 或已有 command 安全封装；**必须**二次确认。

### 验收标准

- [ ] 模拟 consecutive errors 停止后，引导条出现。
- [ ] 点恢复可触发已有 restore 流程且 UI 有 loading/结果反馈。
- [ ] 不会自动 git hard reset（必须用户确认）。

### 依赖

无。

---

## OPT-010: 前端测试与版本对齐

**优先级**：P2  
**状态**：pending  
**预估工作量**：小

### 背景

- `package.json` 声明 `vitest` 脚本但 **无测试文件**。
- `CHANGELOG.md` 为 0.2.0，`package.json` / `Cargo.toml` 仍为 0.1.0。

### 目标行为

1. 版本号统一为 **0.2.0**（或当前 CHANGELOG 最新版）。
2. 添加 vitest + `@testing-library/react`（若尚未安装）。
3. 至少覆盖：
   - `useChat` hook：send 后 messages 更新
   - `ChatInput`：`/level-1` slash command 调用 API
   - `DocPreview`：in_review banner 渲染（mock API）
4. CI `frontend` job 增加 `npm run test`。

### 相关文件

```
shuji-app/package.json
shuji-app/src-tauri/Cargo.toml
CHANGELOG.md
.github/workflows/check.yml
shuji-app/src/hooks/useChat.ts
shuji-app/src/components/ChatInput.tsx
shuji-app/src/components/DocPreview.tsx
```

### 验收标准

- [ ] 三处版本号一致。
- [ ] `npm run test` 本地与 CI 通过。
- [ ] 至少 3 个 frontend test cases。

### 依赖

无。

---

## OPT-011: 架构文档与实现一致

**优先级**：P2  
**状态**：pending  
**预估工作量**：小

### 背景

`shuji-app/mailbox_design.md` 描述 V2 信箱机制（Pull 式、快慢信箱），与当前 **Actor + route_to Push** 模型不一致，易误导 Agent 和贡献者。

### 目标行为

1. 在 `mailbox_design.md` 顶部增加 **状态声明** banner：`Status: FUTURE / NOT IMPLEMENTED`。
2. 新建 `shuji-app/ARCHITECTURE.md`（或更新 `README.md` 架构节）描述 **当前实际** 消息流：
   - ActorSystem + mpsc
   - route_to + FastMessage interrupt
   - 文档 ID 通信
3. `CLAUDE.md` 加一行指向 `ARCHITECTURE.md`，注明 mailbox 为未来设计。
4. 若 README 与 CLAUDE 冲突，以 **代码为准** 更新文档。

### 相关文件

```
shuji-app/mailbox_design.md
README.md
CLAUDE.md
shuji-app/ARCHITECTURE.md  # 新建
```

### 验收标准

- [ ] 新 Agent 读 ARCHITECTURE.md 不会误以为 mailbox 已实现。
- [ ] 文档中的组件名、文件路径与代码一致（抽查 5 处）。

### 依赖

无。

---

## OPT-012: `.shuji` 配置导出/导入

**优先级**：P3  
**状态**：pending  
**预估工作量**：中

### 背景

skills、soul、context_config、祖训（zuxun）分散在 `.shuji/`，换机器或换模型后行为不一致。

### 目标行为

1. Settings 或 ProjectOverview 提供 **导出配置包**（zip）：`.shuji/skills/`、`soul.md`、`context_config.json`、`zuxun.md`（若存在）。
2. **导入配置包**到当前项目（冲突时询问覆盖/合并）。
3. 不导出 API Key、token 统计、checkpoint 快照。

### 相关文件

```
shuji-app/src-tauri/src/storage/shuji_dir.rs
shuji-app/src-tauri/src/commands/project.rs  # 或新 commands/config_pack.rs
shuji-app/src/components/SettingsMenu.tsx
shuji-app/assets/defaults/zuxun.md
```

### 验收标准

- [ ] 导出 zip 可在另一项目导入且内阁 skill 列表一致。
- [ ] 导入不含任何 api key 字段。
- [ ] 集成测试：export → import roundtrip（temp dir）。

### 依赖

无。

---

## OPT-013: 外部编辑器桥接

**优先级**：P3  
**状态**：pending  
**预估工作量**：小

### 背景

用户习惯在 VS Code / Cursor 中看 diff 和改代码，枢机仅桌面 UI 不够顺手。

### 目标行为

1. ProjectDashboard 增加 **「在外部编辑器中打开」** 按钮。
2. 调用系统默认或配置的编辑器（环境变量 `SHUJI_EDITOR` 或设置项）打开 `project.working_dir`。
3. Windows：`code` / `cursor` 检测；macOS：`open -a`；Linux：`xdg-open` 或 `code`。
4. 可选：监听 git status 变化刷新 DocTree（debounce 5s），不实现 LSP。

### 相关文件

```
shuji-app/src/pages/ProjectDashboard.tsx
shuji-app/src-tauri/src/commands/project.rs
shuji-app/src/pages/SetupPage.tsx  # 可选编辑器路径配置
shuji-app/src-tauri/tauri.conf.json  # shell 权限
```

### 验收标准

- [ ] Windows 上点击可在 VS Code 或 Cursor 打开项目根目录。
- [ ] 未安装编辑器时有友好错误提示。
- [ ] 不引入完整 IDE 插件。

### 依赖

无。

---

## OPT-014: 可选「现代命名」模式

**优先级**：P3  
**状态**：pending  
**预估工作量**：中

### 背景

「三省六部」对国内开发者有辨识度，但对新用户和国际化有认知成本。

### 目标行为

1. 设置项 `naming_mode: classic | modern`。
2. modern 模式下 UI **显示名** 映射：
   - 内阁→PM/编排器，中书令→架构师，工部→工程师，刑部→QA，等（完整表见 HelpDrawer）。
3. **不修改** 后端 Role enum、日志文件名、`.shuji` 内部 author 字段（避免破坏兼容）。
4. 仅前端展示层 + ChatBubble 部门标签切换。

### 相关文件

```
shuji-app/src/models/roleLabels.ts  # 新建
shuji-app/src/components/HelpDrawer.tsx
shuji-app/src/components/DeptStatusBar.tsx
shuji-app/src/components/DeptStatusPanel.tsx
shuji-app/src/pages/SetupPage.tsx
```

### 验收标准

- [ ] 切换 naming_mode 后 UI 部门名变化，后端日志仍为中文部门名。
- [ ] 默认 classic，旧用户无感知。
- [ ] HelpDrawer 根据模式展示两套说明。

### 依赖

无。

---

## OPT-015: 审计日志 + 血缘追溯

**优先级**：P2  
**状态**：done  
**预估工作量**：中

### 背景

项目的文档驱动通信模式天然产生丰富的可审计数据（谁创建了什么文档、谁引用了谁、谁修改了什么状态），但缺乏系统化的记录和可视化。

### 目标行为

1. 新建 `src-tauri/src/audit/mod.rs`，提供 `AuditEntry` 结构体和 JSONL 持久化
2. 在 `create_document` / `set_document_status` / checkpoint / milestone 四个关键节点自动 append `.shuji/audit.jsonl`
3. 实现 `build_lineage()`：给定文档 ID，递归 follow refs 构建血缘树
4. DocPreview 增加「血缘」Tab，树形展示引用链

### 验收标准

- [ ] 创建文档后 `.shuji/audit.jsonl` 自动新增一行
- [ ] DocPreview 血缘 Tab 展示树形结构（无环）
- [ ] 复用 `documents.rs` 的 `parse_doc` / `parse_refs`

### 依赖

无。

---

## OPT-016: 朝报时间线聚合

**优先级**：P2  
**状态**：done  
**预估工作量**：小

### 背景

审计日志是原始事件流，需要聚合视图以便快速了解项目进展。

### 目标行为

1. `AuditPanel.tsx` sidebar 组件：按时间倒序展示所有审计条目
2. 顶部聚合卡片：事件总数 + 按事件类型/部门的计数
3. ActivityBar 新增「朝报」入口，Sidebar 切换到审计面板

### 验收标准

- [ ] 朝报面板显示事件列表（时间倒序）
- [ ] 顶部统计摘要正确
- [ ] 空状态显示「尚无朝报记录」

### 依赖

- OPT-015

---

## OPT-017: 交付报告自动生成

**优先级**：P2  
**状态**：done  
**预估工作量**：小

### 背景

项目结束后或阶段性交付时，需要一份汇总文档说明产出。

### 目标行为

1. `generate_delivery_report()` 读取审计日志，生成 Markdown 报告
2. 包含：工起/工讫时间、事件统计、部门活跃度、文档产出清单
3. ProjectOverview 增加「生成交付报告」按钮，点击后就地预览

### 验收标准

- [ ] 有审计记录时生成包含统计数据和文档清单的报告
- [ ] 无审计记录时显示「尚无审计记录」
- [ ] 按钮有 loading 状态

### 依赖

- OPT-015

---

## 验证清单（每项任务完成后执行）

```bash
# 后端
cd shuji-app/src-tauri
cargo fmt --check
cargo clippy --all-targets -- -W clippy::all
cargo test --lib
cargo test --tests -- --skip expand_requirements

# 前端
cd shuji-app
npm run lint
npm run test      # OPT-010 之后
npm run build
```

手动冒烟（涉及 UI 的任务）：

1. `npm run tauri dev`
2. 走 Demo 或发送一条简单指令
3. 确认 Dashboard、部门栏、文档预览无回归

---

## 产品定位备忘（Agent 请勿偏离）

枢机差异化在 **「可审计、可审批、文档驱动的自主开发流水线」**，而非与 Cursor 拼单文件改代码速度。实现时：

- 优先 **降低首次成功成本**（P0）和 **透明化等待/成本**（P0）
- 再追求 **吞吐与成本**（P1 并行、模型分级、短路）
- 最后做 **生态与可选体验**（P3）

---

## 修订记录

| 日期 | 说明 |
|------|------|
| 2026-06-01 | 初版：14 项优化任务，源自产品评审 |
