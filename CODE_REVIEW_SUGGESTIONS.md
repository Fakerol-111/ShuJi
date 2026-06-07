# 枢机（ShuJi）代码审查优化建议

> **来源**：2026-06-05 基于实际代码（`shuji-app/src-tauri` + `shuji-app/src`）的审查结论，**不以文档为准**。
>
> **用途**：供逐项实现、验收。每完成一项，将 `状态` 改为 `done`，并在 `CHANGELOG.md` 简要记录。
>
> **与 `OPTIMIZATION_ROADMAP.md` 的关系**：
> - 本文件聚焦 **代码审查新发现** 与 **体验/架构债务**。
> - 路线图中已有且未完成的项（如 OPT-006、OPT-009）在本文件 **交叉引用**，不重复展开；优先按路线图编号执行即可。
>
> **执行原则**：
> 1. 严格按 **P0 → P1 → P2 → P3** 推进；同优先级内按编号顺序。
> 2. 每项独立 PR/commit，避免大范围混合改动。
> 3. 改动前先读「相关文件」；改动后跑文末「验证清单」。
> 4. 不要过度设计：只解决该项描述的问题，不顺带重构无关模块。

---

## 目录

| 编号 | 优先级 | 任务 | 状态 |
|------|--------|------|------|
| [SUG-001](#sug-001-决策-tab-发送后-processing-态) | P0 | 决策 Tab 发送后 processing 态 | pending |
| [SUG-002](#sug-002-修复-useproject-项目加载竞态) | P0 | 修复 useProject 项目加载竞态 | pending |
| [SUG-003](#sug-003-setup-跳过与-workspaceselect-逻辑一致) | P0 | Setup「跳过」与 WorkspaceSelect 逻辑一致 | pending |
| [SUG-004](#sug-004-聊天区展示-gate-拦截与阻塞原因) | P0 | 聊天区展示 Gate 拦截与阻塞原因 | pending |
| [SUG-005](#sug-005-接通或删除-stagetracker) | P1 | 接通或删除 StageTracker | pending |
| [SUG-006](#sug-006-文档朱批审计推到-ui-中心) | P1 | 文档/朱批/审计推到 UI 中心 | pending |
| [SUG-007](#sug-007-明确兵部职责与工具集) | P1 | 明确兵部职责与工具集 | pending |
| [SUG-008](#sug-008-刑部与礼部并行执行) | P1 | 刑部与礼部并行执行 | → [OPT-006](OPTIMIZATION_ROADMAP.md#opt-006-刑部与礼部并行执行) |
| [SUG-009](#sug-009-提取-ministryagent-消除部门复制粘贴) | P2 | 提取 MinistryAgent 消除部门复制粘贴 | pending |
| [SUG-010](#sug-010-内阁-compact-handler-复用-runnerrs) | P2 | 内阁 compact handler 复用 runner.rs | pending |
| [SUG-011](#sug-011-run_actor-集成测试) | P2 | run_actor 集成测试 | pending |
| [SUG-012](#sug-012-朱批门禁与-set_document_status-测试) | P2 | 朱批门禁与 set_document_status 测试 | pending |
| [SUG-013](#sug-013-统一-context_configjson-读写路径) | P2 | 统一 context_config.json 读写路径 | pending |
| [SUG-014](#sug-014-清理前端死代码与未用-api) | P2 | 清理前端死代码与未用 API | pending |
| [SUG-015](#sug-015-失败恢复引导-ui) | P2 | 失败恢复引导 UI | → [OPT-009](OPTIMIZATION_ROADMAP.md#opt-009-失败恢复引导-ui) |
| [SUG-016](#sug-016-前端测试与版本对齐) | P2 | 前端测试与版本对齐 | → [OPT-010](OPTIMIZATION_ROADMAP.md#opt-010-前端测试与版本对齐) |
| [SUG-017](#sug-017-修复-expand_requirements-编码损坏) | P2 | 修复 expand_requirements 编码损坏 | pending |
| [SUG-018](#sug-018-删除或接线-dead-字段) | P2 | 删除或接线 dead 字段 | pending |
| [SUG-019](#sug-019-基础无障碍与键盘支持) | P3 | 基础无障碍与键盘支持 | pending |
| [SUG-020](#sug-020-减少轮询改为事件或合并) | P3 | 减少轮询，改为事件或合并 | pending |
| [SUG-021](#sug-021-统一-modal-与-projectpicker-视觉) | P3 | 统一 Modal 与 ProjectPicker 视觉 | pending |
| [SUG-022](#sug-022-logs-页面与-logbar-关系澄清) | P3 | Logs 页面与 LogBar 关系澄清 | pending |
| [SUG-023](#sug-023-静默错误处理审计) | P3 | 静默错误处理审计 | pending |
| [SUG-024](#sug-024-多平台发布与-auto-updater) | P3 | 多平台发布与 auto-updater | pending |
| [SUG-025](#sug-025-shuji-配置导出导入) | P3 | `.shuji` 配置导出/导入 | → [OPT-012](OPTIMIZATION_ROADMAP.md#opt-012-shuji-配置导出导入) |
| [SUG-026](#sug-026-外部编辑器桥接) | P3 | 外部编辑器桥接 | → [OPT-013](OPTIMIZATION_ROADMAP.md#opt-013-外部编辑器桥接) |
| [SUG-027](#sug-027-可选现代命名模式) | P3 | 可选「现代命名」模式 | → [OPT-014](OPTIMIZATION_ROADMAP.md#opt-014-可选现代命名模式) |

---

## 产品定位备忘（实现时勿偏离）

枢机差异化在 **「可审计、可审批、文档驱动的多 Agent 开发流水线」**，而非与 Cursor 拼单文件改代码速度。优化时应优先：

1. **让用户始终知道**：当前阶段、卡在哪个文档、哪个部门在做什么。
2. **强化文档 + 朱批 + 审计** 作为主叙事，聊天是「下诏入口」而非唯一界面。
3. **代码硬约束**（Gate、朱批门禁）比 prompt 约束更可靠，应在 UI 可见。

---

## SUG-001: 决策 Tab 发送后 processing 态

**优先级**：P0  
**状态**：pending  
**预估工作量**：小（前端）

### 背景

「廷议」Tab 有 `discussing` 锁与 disabled input；「决策」Tab 的 `ChatInput` 始终 `disabled={false}`。`sendMessage` 立即返回 ack，结果靠 `chat-message` 事件推送。用户可能重复发送，进度只能靠底部 `DeptStatusBar` 间接感知。

### 当前实现

- `shuji-app/src/components/ChatPanel.tsx`：`disabled={false}`
- `shuji-app/src/hooks/useChat.ts`：`handleSend` 无 processing 状态
- `shuji-app/src/hooks/useActiveDepts.ts`：1s 轮询 `getActiveRoles()`

### 目标行为

1. 发送敕命后，输入框 disabled，显示「诸司奉旨…」或类似文案。
2. 当 `getActiveRoles()` 为空且距发送超过短 debounce，或收到内阁/系统首条 `chat-message`，恢复可输入。
3. 「叫停诸司」在 processing 期间仍可用。

### 相关文件

```
shuji-app/src/hooks/useChat.ts
shuji-app/src/components/ChatPanel.tsx
shuji-app/src/components/ChatInput.tsx
shuji-app/src/hooks/useActiveDepts.ts
```

### 验收标准

- [ ] 决策 Tab 发送后无法连点重复发送。
- [ ] 工作流结束后输入框自动恢复。
- [ ] 廷议 Tab 行为不变。
- [ ] `npm run lint` 通过。

---

## SUG-002: 修复 useProject 项目加载竞态

**优先级**：P0  
**状态**：pending  
**预估工作量**：小（前端）

### 背景

`useProject` mount 时自动 `loadProjectIntoState(recentDirs[0])`，与 `WorkspaceSelect` 用户刚选择的路径可能冲突，Dashboard 可能展示错误的项目。

### 当前实现

```typescript
// shuji-app/src/hooks/useProject.ts
getRecentDirs().then((dirs) => {
  setRecentDirs(dirs);
  if (dirs.length > 0) void loadProjectIntoState(dirs[0]);
});
```

`WorkspaceSelect` 调用 `loadProject(path)` 后 `navigate("/project")`，但 Dashboard 的 `useProject` 会再次 load recent[0]。

### 目标行为（择一或组合）

1. **方案 A**：`navigate("/project", { state: { projectPath } })`，Dashboard 优先 load state 中的路径。
2. **方案 B**：mount 时不自动 load recent[0]，仅在没有已 load 项目时加载。
3. **方案 C**：后端 `load_project` 返回当前 session 项目，前端以 session 为准。

### 相关文件

```
shuji-app/src/hooks/useProject.ts
shuji-app/src/pages/WorkspaceSelect.tsx
shuji-app/src/pages/ProjectDashboard.tsx
shuji-app/src-tauri/src/commands/project.rs
```

### 验收标准

- [ ] 从 WorkspaceSelect 打开目录 B，Dashboard 显示 B（即使 recent[0] 是 A）。
- [ ] 直接刷新 `/project` 时行为合理（最近项目或空态提示）。
- [ ] Demo 路径不受影响。

---

## SUG-003: Setup「跳过」与 WorkspaceSelect 逻辑一致

**优先级**：P0  
**状态**：pending  
**预估工作量**：小（前端）

### 背景

`SetupPage` 允许「跳过」进入主界面；`WorkspaceSelect` mount 时若无 API key 会 redirect 到 `/setup`。逻辑分裂，用户可能困惑。

### 目标行为

- 若允许跳过：WorkspaceSelect 不再强制 redirect，或在跳过时显示明确 banner「未配置 API，部分功能不可用」。
- 若不允许跳过：删除 Setup 跳过按钮，与 WorkspaceSelect 一致。

### 相关文件

```
shuji-app/src/pages/SetupPage.tsx
shuji-app/src/pages/WorkspaceSelect.tsx
```

### 验收标准

- [ ] 跳过/强制配置两种策略择一，全链路一致。
- [ ] 无 API key 时发送消息有友好错误，而非 silent fail。

---

## SUG-004: 聊天区展示 Gate 拦截与阻塞原因

**优先级**：P0  
**状态**：pending  
**预估工作量**：中（前端 + 可选后端事件）

### 背景

`GateEngine` 拦截返回 `operation: "gate_blocked"`，信息主要在 dept-log。新手不易理解「为何不能路由到中书令」。

### 当前实现

- 后端：`shuji-app/src-tauri/src/workflow/gate.rs`
- 前端：`LogBar` / `DeptStatusPanel` 有路由/错误分类；聊天区无专门展示

### 目标行为

1. Gate 拦截或朱批 pending 导致的路由失败，在聊天区或 `WorkflowTimeline` 顶部显示 **可行动提示**（如「当前 Profile 禁止路由到门下侍中，请先完成 plan_12 朱批」）。
2. 点击可跳转待批文档 Tab。

### 相关文件

```
shuji-app/src-tauri/src/workflow/gate.rs
shuji-app/src-tauri/src/tool/documents.rs
shuji-app/src/components/WorkflowTimeline.tsx
shuji-app/src/components/ChatBubble.tsx
shuji-app/src/pages/ProjectDashboard.tsx
```

### 验收标准

- [ ] workflow_demo 下尝试 route 到中书令，用户能在 UI 看到拦截原因（不必读 JSONL 日志）。
- [ ] 与 OPT-003 流程地图不重复堆砌，信息分层清晰。

---

## SUG-005: 接通或删除 StageTracker

**优先级**：P1  
**状态**：pending  
**预估工作量**：中～大（后端 + 前端）

### 背景

`StageTracker` 从 Workflow Profile YAML 解析进 `ActiveProfile`，但运行时 **从未** `advance()`。阶段推进靠 LLM + prompt。`WorkflowState.current_stage` 是另一套极简字符串（仅 init/execution）。UI `WorkflowTimeline` 展示的 stage 可能与真实流程脱节。

### 当前实现

- `shuji-app/src-tauri/src/workflow/stage.rs` — 完整阶段机 + 单测
- `shuji-app/src-tauri/src/workflow/profile.rs` — `stage_tracker` 字段
- `shuji-app/src-tauri/src/workflow/state.rs` — 独立 `WorkflowState`
- 全库无 `stage_tracker.advance()` 调用

### 目标行为（二选一）

**方案 A — 接通**：

1. 在 `route_to` 成功、`set_document_status(approved)`、工部 `complete_task` 等节点调用 `stage_tracker.advance()`。
2. 持久化到 `.shuji/workflow_state.json`，与 `WorkflowState` 合并或替代。
3. `WorkflowTimeline` 读取真实 stage 列表与当前索引。

**方案 B — 删除**：

1. 从 `ActiveProfile` 移除 `stage_tracker`，YAML stages 仅作文档。
2. 统一只用 `WorkflowState` + 文档状态推断 UI。
3. 更新 `workflow_profile_test.rs`。

### 相关文件

```
shuji-app/src-tauri/src/workflow/stage.rs
shuji-app/src-tauri/src/workflow/profile.rs
shuji-app/src-tauri/src/workflow/state.rs
shuji-app/src-tauri/src/actor/mod.rs
shuji-app/src/components/WorkflowTimeline.tsx
```

### 验收标准

- [ ] 不存在「Profile 有阶段定义但运行时零消费」的死代码路径。
- [ ] greenfield_standard 走完后，UI 阶段与代码状态一致。
- [ ] `cargo test --test workflow_profile_test` 通过。

---

## SUG-006: 文档/朱批/审计推到 UI 中心

**优先级**：P1  
**状态**：pending  
**预估工作量**：中（前端）

### 背景

产品最强差异化是文档驱动 + 朱批 + 审计，但默认主视图是 `ProjectOverview`，待办分散在 Sidebar、Timeline badge、DocPreview。

### 目标行为

1. 主内容区默认或 prominently 展示：**待朱批文档**、**当前 task/ctrt**、**最新 revw 结论**（只读摘要）。
2. 无待办时展示项目概览；有待办时待办卡片置顶。
3. 与 `AuditPanel`「朝报」形成「决策 → 文档 → 审计」三角，HelpDrawer 更新说明。

### 相关文件

```
shuji-app/src/components/ProjectOverview.tsx
shuji-app/src/components/WorkflowTimeline.tsx
shuji-app/src/components/DocPreview.tsx
shuji-app/src/pages/ProjectDashboard.tsx
shuji-app/src/api.ts  # getPendingApprovals 等
```

### 验收标准

- [ ] 有 pending approval 时，用户进入 Dashboard **无需点 Sidebar** 即可看到待办。
- [ ] 点击待办直达 DocPreview 朱批 UI。
- [ ] 不影响已有 Tab 文档浏览。

---

## SUG-007: 明确兵部职责与工具集

**优先级**：P1  
**状态**：pending  
**预估工作量**：中（后端 prompt + 可选工具 + 文档/UI）

### 背景

「兵部尚书」在组织叙事中是「测试+契约」，但代码仅 `inspect_tools()` + `document_tools()`，无 `run_tests_tool` / `file_write_tools`。测试由工部/刑部承担，角色边界模糊，易误导 LLM 和用户。

### 当前实现

```rust
// shuji-app/src-tauri/src/agent/bingbushangshu/mod.rs
fn tools() -> Vec<ToolDefinition> {
    let mut tools = crate::tool::registry::inspect_tools();
    tools.extend(crate::tool::registry::document_tools());
    tools
}
```

### 目标行为（择一）

| 方案 | 内容 |
|------|------|
| A. 补工具 | 兵部增加 `run_tests_tool`（只跑不写）+ 契约文档为主 |
| B. 收窄职责 | 保持只写 ctrt 文档，更新 prompt/HelpDrawer/部门表为「契约官」，刑部负责验证 |
| C. 合并角色 | 长期考虑与刑部合并（工作量大，非本项范围） |

### 相关文件

```
shuji-app/src-tauri/src/agent/bingbushangshu/mod.rs
shuji-app/src-tauri/src/agent/bingbushangshu/prompt.md
shuji-app/src/constants.ts          # DEPT_META 描述
shuji-app/src/components/HelpDrawer.tsx
```

### 验收标准

- [ ] 兵部 prompt、工具列表、UI 部门说明三者一致。
- [ ] standard workflow 中兵部行为可预期（写 ctrt 或跑测试，不 ambiguity）。

---

## SUG-008: 刑部与礼部并行执行

**优先级**：P1  
**状态**：→ 见 [OPT-006](OPTIMIZATION_ROADMAP.md#opt-006-刑部与礼部并行执行)（pending）

---

## SUG-009: 提取 MinistryAgent 消除部门复制粘贴

**优先级**：P2  
**状态**：pending  
**预估工作量**：大（后端重构）

### 背景

`libushangshu`、`bingbushangshu`、`liburshangshu`、`xingbushangshu`、`shangshuling` 结构 ~80% 相同，仅 `tools()` 和 prompt 不同。维护成本高，改 compact/checkpoint 需改多处。

### 目标行为

1. 新增 `MinistryAgent` 或配置 struct：`role`, `tools_fn`, `prompt_path`, 可选 `after_execute` hook。
2. 五部门改为薄包装或 registry 注册。
3. **保留** 内阁、中书令、门下、工部的特殊逻辑不动。

### 相关文件

```
shuji-app/src-tauri/src/agent/runner.rs
shuji-app/src-tauri/src/agent/libushangshu/mod.rs
shuji-app/src-tauri/src/agent/bingbushangshu/mod.rs
shuji-app/src-tauri/src/agent/xingbushangshu/mod.rs
shuji-app/src-tauri/src/agent/liburshangshu/mod.rs
shuji-app/src-tauri/src/agent/shangshuling/mod.rs
```

### 验收标准

- [ ] 五部门行为与重构前一致（跑 workflow_demo_test + 手动 smoke）。
- [ ] 新增部门只需配置 + prompt 文件。
- [ ] `cargo test --tests` 通过。

---

## SUG-010: 内阁 compact handler 复用 runner.rs

**优先级**：P2  
**状态**：pending  
**预估工作量**：小（后端）

### 背景

`agent/runner.rs` 注释写明为「非内阁 Agent 提取重复逻辑」，但内阁在 `neige/mod.rs` 手写了一份相同的 compact handler。

### 目标行为

内阁改用 `runner.rs` 的 `make_compact_handler` / `load_and_compact_context`（或提取 shared fn），删除重复代码。

### 相关文件

```
shuji-app/src-tauri/src/agent/runner.rs
shuji-app/src-tauri/src/agent/neige/mod.rs
```

### 验收标准

- [ ] 内阁 mid-run compact 行为不变。
- [ ] 无 duplicate compact 逻辑块。

---

## SUG-011: run_actor 集成测试

**优先级**：P2  
**状态**：pending  
**预估工作量**：中（测试）

### 背景

`actor/mod.rs` 的 `run_actor` 是最复杂模块之一（Replace/Interrupt、failure fallback、pause/resume、工部 plan loop），但 `actor_test.rs` 主要测消息枚举和 cancel flag，无运行时集成测。

### 目标行为

至少覆盖：

1. Replace 消息 fall-through 立即执行
2. 部门失败 → 尚书令 fallback（≤3 次）
3. route_to 转发到目标 mailbox
4. （可选）pause/resume 写读 `paused_session.json`

### 相关文件

```
shuji-app/src-tauri/tests/actor_test.rs
shuji-app/src-tauri/tests/common/mod.rs
shuji-app/src-tauri/src/actor/mod.rs
```

### 验收标准

- [ ] 新增 ≥3 个集成测试，不依赖真实 API（mock Agent 或 wiremock）。
- [ ] CI `--test-threads=1` 稳定通过。

---

## SUG-012: 朱批门禁与 set_document_status 测试

**优先级**：P2  
**状态**：pending  
**预估工作量**：中（测试）

### 背景

`document_test.rs` 覆盖 CRUD，但 **无** `pending_approvals`、`route_to` 硬门禁、`append_document` in_review 门禁的测试。这是核心差异化逻辑，应有回归保护。

### 目标行为

新增测试用例：

1. plan in_review 时 `route_to` 工部被拦
2. `set_document_status(approved)` 清除 pending
3. `append_document` 对 must-approve 类型的 gate

### 相关文件

```
shuji-app/src-tauri/tests/document_test.rs
shuji-app/src-tauri/src/tool/documents.rs
shuji-app/src-tauri/src/tool/mod.rs
```

### 验收标准

- [ ] ≥5 个门禁相关测试
- [ ] `cargo test --test document_test` 通过

---

## SUG-013: 统一 context_config.json 读写路径

**优先级**：P2  
**状态**：pending  
**预估工作量**：小～中

### 背景

- UI / settings：`CWD` 或父目录的 `context_config.json`
- 运行时 actor：`working_dir/context_config.json`

两处可能读写不同文件，导致 Settings 改了不生效。

### 目标行为

单一真相源：**项目 `working_dir/context_config.json`**（与 `.shuji/` 同级或项目根，择一定义并全库统一）。

### 相关文件

```
shuji-app/src-tauri/src/commands/settings.rs
shuji-app/src-tauri/src/actor/mod.rs
shuji-app/src-tauri/src/commands/workflow.rs
shuji-app/src-tauri/src/config/mod.rs
```

### 验收标准

- [ ] Settings 保存后，actor compact 阈值立即使用新值（可写 config_test 或 integration test）。
- [ ] 文档与 example 路径更新。

---

## SUG-014: 清理前端死代码与未用 API

**优先级**：P2  
**状态**：pending  
**预估工作量**：小

### 背景

- `DecisionPanel.tsx`、`DocumentViewer.tsx` 无任何 import
- `api.ts` 中 `createProject`、`getProject`、`listProjects`、`getSnapshot` 未调用
- `constants.ts` 中 `DEPT_ACTIVE_TIMEOUT_MS`、`CHAT_PANEL_*` 等未接线
- `ui/Input.tsx`、`Textarea.tsx`、`Badge.tsx` 未 adoption

### 目标行为

1. 删除或接入死组件（若 DecisionPanel 功能已被 DocPreview 替代则删除）。
2. 删除或实现未用 API 的 UI 入口（如「创建项目」）。
3. 未用常量：删除或替换 magic number。

### 相关文件

```
shuji-app/src/components/DecisionPanel.tsx
shuji-app/src/components/DocumentViewer.tsx
shuji-app/src/api.ts
shuji-app/src/constants.ts
shuji-app/src/components/ui/
```

### 验收标准

- [ ] `npm run lint` 通过
- [ ] 无新增 dead code 警告（可选：eslint unused）

---

## SUG-015: 失败恢复引导 UI

**优先级**：P2  
**状态**：→ 见 [OPT-009](OPTIMIZATION_ROADMAP.md#opt-009-失败恢复引导-ui)（pending）

---

## SUG-016: 前端测试与版本对齐

**优先级**：P2  
**状态**：→ 见 [OPT-010](OPTIMIZATION_ROADMAP.md#opt-010-前端测试与版本对齐)（pending）

---

## SUG-017: 修复 expand_requirements 编码损坏

**优先级**：P2  
**状态**：pending  
**预估工作量**：小

### 背景

`expand_requirements.rs` 文件头注释与用户可见 prompt 字符串为乱码（`????`），功能逻辑完整但可读性差，可能影响 sub-agent 输出质量。

### 相关文件

```
shuji-app/src-tauri/src/agent/expand_requirements.rs
shuji-app/src-tauri/src/agent/expand_requirements_prompt.md
```

### 验收标准

- [ ] 源文件 UTF-8 正常，中文注释与 prompt 可读
- [ ] `cargo test --test expand_requirements_test`（本地有 API 时）行为不变

---

## SUG-018: 删除或接线 dead 字段

**优先级**：P2  
**状态**：pending  
**预估工作量**：小～中

### 背景

| 字段/代码 | 现状 |
|-----------|------|
| `ActorContext.shared_context` | 只写不读 |
| `ActorContext.plan` | 从未使用；工部 plan 在 Agent 内部 |
| `Role::system_prompt()` | 无引用 |
| Fast mailbox 注释「bounded 16」 | 实际 `unbounded_channel` |

### 目标行为

- 有产品计划则接线（如 shared_context 供尚书令读各部门摘要）
- 否则删除字段与误导注释，降低认知负担

### 相关文件

```
shuji-app/src-tauri/src/actor/mod.rs
shuji-app/src-tauri/src/models/role.rs
shuji-app/src-tauri/src/commands/workflow.rs
```

### 验收标准

- [ ] `cargo check` + 全测试通过
- [ ] 无「预留但未用」的大 struct 字段（或注释标明 TODO 与 issue）

---

## SUG-019: 基础无障碍与键盘支持

**优先级**：P3  
**状态**：pending  
**预估工作量**：中

### 背景

全库几乎无 `aria-label`；图标按钮依赖 hover tooltip（键盘/读屏不可达）；Modal 无 focus trap。

### 目标行为（最小集）

1. ActivityBar、Settings、Help、叫停 等图标按钮加 `aria-label`
2. `ProjectPicker`、`DemoTour`、`HelpDrawer`、Checkpoint 确认框：`role="dialog"` + Esc 关闭 + 初始 focus
3. 聊天新消息：`aria-live="polite"` 区域（可选）

### 相关文件

```
shuji-app/src/components/ActivityBar.tsx
shuji-app/src/components/SettingsMenu.tsx
shuji-app/src/components/HelpDrawer.tsx
shuji-app/src/components/ProjectPicker.tsx
shuji-app/src/components/DemoTour.tsx
shuji-app/src/components/ChatPanel.tsx
```

### 验收标准

- [ ] Tab 键可到达主要操作并完成发送/关闭 dialog
- [ ] axe 或 Lighthouse a11y 无 critical（手动抽查即可）

---

## SUG-020: 减少轮询，改为事件或合并

**优先级**：P3  
**状态**：pending  
**预估工作量**：中

### 背景

多处 interval：`getActiveRoles` 1s、`getPendingApprovals` 3s、`getWorkflowState` 3s、`getRoundMetrics` 3s、`getTokenStats` 30s。Tab 切换不暂停，空转浪费。

### 目标行为

1. 合并相关 polling 为单 hook（如 `useDashboardPoll`）
2. 后端对 workflow_state / pending_approvals 变更 emit 事件（与 chat-message 模式一致）
3. Sidebar 非 visible 时降频或暂停

### 相关文件

```
shuji-app/src/pages/ProjectDashboard.tsx
shuji-app/src/hooks/useActiveDepts.ts
shuji-app/src/components/WorkflowTimeline.tsx
shuji-app/src/components/DeptStatusBar.tsx
shuji-app/src-tauri/src/commands/workflow.rs
```

### 验收标准

- [ ] 活跃工作流时 UI 仍 ≤3s 内更新
- [ ] idle 时 CPU/invoke 次数明显下降（DevTools 可观测）

---

## SUG-021: 统一 Modal 与 ProjectPicker 视觉

**优先级**：P3  
**状态**：pending  
**预估工作量**：小

### 背景

`ProjectPicker`、部分确认框使用白底 `bg-white`，与全局 paper/ink 主题不一致。

### 目标行为

Modal/Picker 使用 `surface-paper`、`border-fold`、`font-display` 等 design token，与 Dashboard 一致。

### 相关文件

```
shuji-app/src/components/ProjectPicker.tsx
shuji-app/src/components/CheckpointPanel.tsx
shuji-app/src/styles/globals.css
```

### 验收标准

- [ ] 打开项目、恢复 checkpoint 确认框视觉与主界面协调

---

## SUG-022: Logs 页面与 LogBar 关系澄清

**优先级**：P3  
**状态**：pending  
**预估工作量**：小

### 背景

- `/logs` 路由读 JSONL 文件（`LogsPage.tsx`）
- 底部 `LogBar` 订阅 `dept-log` 事件
- ActivityBar 只展开 LogBar，不导航 `/logs`；用户难以理解两者差异

### 目标行为（择一）

| 方案 | 内容 |
|------|------|
| A | 删除 `/logs` 路由，LogBar 加「导出/查看完整日志」 |
| B | ActivityBar 增加入口跳转 `/logs`，页面说明与 LogBar 区别 |
| C | 合并为同一组件，LogsPage 为全屏版 LogBar |

### 相关文件

```
shuji-app/src/pages/LogsPage.tsx
shuji-app/src/components/LogBar.tsx
shuji-app/src/components/ActivityBar.tsx
shuji-app/src/main.tsx
```

### 验收标准

- [ ] 用户只有一条清晰路径查看部门日志
- [ ] 无 orphan 路由

---

## SUG-023: 静默错误处理审计

**优先级**：P3  
**状态**：pending  
**预估工作量**：中～大

### 背景

全库 100+ 处 `let _ = ...`，磁盘写入、审计、channel send 失败被吞掉，生产排查困难。

### 目标行为

1. 分级：用户可见错误 → `log_dept` / emit event；内部错误 → `tracing::warn!` 或 `log_console!`
2. 优先改 **持久化路径**（session save、checkpoint、audit append、milestone）
3. 不追求一次清零，按文件分批

### 相关文件

```
shuji-app/src-tauri/src/api/session.rs
shuji-app/src-tauri/src/storage/checkpoint.rs
shuji-app/src-tauri/src/audit/mod.rs
shuji-app/src-tauri/src/actor/mod.rs
```

### 验收标准

- [ ] 持久化失败至少有一条 stderr / JSONL 记录
- [ ] 第一轮覆盖 session + checkpoint（清单在 PR 描述中列出）

---

## SUG-024: 多平台发布与 auto-updater

**优先级**：P3  
**状态**：pending  
**预估工作量**：大

### 背景

`.github/workflows/publish.yml` 仅 `windows-latest` NSIS draft release；`tauri.conf.json` 无 updater；`csp: null`。

### 目标行为

1. CI matrix：Windows + macOS（+ Linux 可选）
2. 评估 Tauri updater 插件与签名流程
3. 版本号与 CHANGELOG 对齐（见 OPT-010）

### 相关文件

```
.github/workflows/publish.yml
shuji-app/src-tauri/tauri.conf.json
shuji-app/package.json
CHANGELOG.md
```

### 验收标准

- [ ] 至少 macOS artifact 可构建（manual workflow 可接受）
- [ ] README 安装说明更新

---

## SUG-025 ~ SUG-027

见 `OPTIMIZATION_ROADMAP.md`：

- **SUG-025** → [OPT-012](OPTIMIZATION_ROADMAP.md#opt-012-shuji-配置导出导入)
- **SUG-026** → [OPT-013](OPTIMIZATION_ROADMAP.md#opt-013-外部编辑器桥接)
- **SUG-027** → [OPT-014](OPTIMIZATION_ROADMAP.md#opt-014-可选现代命名模式)

---

## 验证清单（每项完成后执行）

```bash
# 后端
cd shuji-app/src-tauri
cargo fmt --check
cargo clippy --all-targets -- -W clippy::all
cargo test --lib
cargo test --tests -- --skip expand_requirements --test-threads=1

# 前端
cd shuji-app
npm run lint
npm run build
# OPT-010 / SUG-016 完成后追加：
# npm run test
```

手动冒烟（涉及 UI 的任务）：

1. `npm run tauri dev`
2. Demo 或发送一条 simple 指令
3. 确认 Dashboard、部门栏、文档预览、决策 Tab 无回归

---

## 建议执行顺序（快速参考）

```
P0 体验止血
  SUG-001 → SUG-002 → SUG-003 → SUG-004

P1 强化差异化
  SUG-006 → SUG-007 → SUG-005 → OPT-006(SUG-008)

P2 工程质量
  SUG-013 → SUG-014 → SUG-017 → SUG-010 → SUG-012 → SUG-011 → SUG-009
  并行：OPT-009(SUG-015)、OPT-010(SUG-016)

P3  polish & 发布
  SUG-019 → SUG-020 → SUG-021 → SUG-022 → SUG-018 → SUG-023 → SUG-024
  并行：OPT-012 ~ OPT-014
```

---

## 修订记录

| 日期 | 说明 |
|------|------|
| 2026-06-05 | 初版：27 项建议，源自代码审查（非文档） |
