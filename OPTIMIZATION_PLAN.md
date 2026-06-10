# ShuJi 优化路线图

> 基于 2026-06-09 对仓库的代码阅读、后端测试运行结果、前端结构评估、后端架构评估、测试与文档评估整理。目标是把当前原型逐步收敛为更稳定、可维护、可验证的产品。

## 当前判断

ShuJi 的后端底座已经超过普通概念验证：Actor 工作流、文档中心通信、工具安全、Session/AgentController 分离、Workflow Profile、审计与上下文持久化都有成体系设计。实际运行过的后端测试结果也较好：

```bash
cd shuji-app/src-tauri
cargo test --lib
cargo test --tests -- --skip expand_requirements
```

上述两组测试均已通过。

当前最值得优先处理的不是继续增加部门或新能力，而是降低复杂度、补齐前端和关键业务路径测试、统一流程真相源、强化错误可观测性，并修正文档与配置漂移。

## P0：先保正确性和回归能力

### 1. 补前端最小测试基建

- [ ] 引入 Vitest 与 React Testing Library。
- [ ] 将 `package.json` 中占位的 `test` 脚本替换为真实测试命令。
- [ ] 覆盖 `utils/error.ts` 的错误格式化和分类逻辑。
- [ ] 覆盖 `constants.ts` 的部门元数据查询逻辑。
- [ ] 覆盖 `ChatBubble` 的 `<options>` 渲染、点击、补充输入。
- [ ] 覆盖 `DocPreview` 的朱批按钮、驳回理由、diff tab、lineage tab 的基础渲染。
- [ ] 覆盖 `useChat` 的乐观发送、失败标记、重试、事件追加。

建议验收：

```bash
cd shuji-app
npm test
npm run lint
```

### 2. 补朱批与门禁测试

朱批是项目架构中的核心契约，目前测试覆盖不足。

- [ ] 新增或扩展 `src-tauri/tests/document_test.rs`，覆盖 `plan` / `revw` 创建后进入 `in_review` 状态。
- [ ] 覆盖 pending approval 写入与移除。
- [ ] 覆盖未批准文档阻止下游 `route_to`。
- [ ] 覆盖未批准文档阻止继续 `append_document`。
- [ ] 覆盖 `set_document_status(approved)` 后放行。
- [ ] 覆盖 `set_document_status(rejected)` 后能保存皇帝意见。

建议验收：

```bash
cd shuji-app/src-tauri
cargo test --test document_test
cargo test --test workflow_demo_test
```

### 3. 修复 Prettier / CI 配置断裂

当前 `format` / `format:check` 脚本存在，但 Prettier 未明确出现在依赖与 CI 中。

- [ ] 将 Prettier 加入前端 devDependencies。
- [ ] 确认 `package-lock.json` 同步更新。
- [ ] 在 CI 中加入 `npm run format:check`。
- [ ] 本地跑一次格式检查。

建议验收：

```bash
cd shuji-app
npm run format:check
npm run lint
npm run build
```

### 4. 修复前端聊天状态多源问题

当前聊天状态来源包括后端 `getChatHistory`、Tauri `chat-message` 事件、`sessionStorage`，去重使用 `timestamp|role|content前40字`，容易误合并或漏合并。

- [ ] 明确单一真相源：推荐以后端历史 + 事件增量为主，`sessionStorage` 仅用于刷新恢复。
- [ ] 为后端 `ChatMessage` 增加稳定 id，或在前端建立更可靠的去重 key。
- [ ] 移除 `ProjectDashboard` render 阶段对 session 对象的 mutation。
- [ ] 将 `mergeMessages` 移到 `utils/chat.ts` 并补测试。
- [ ] 统一 `initialCabinetMessage`，避免 `useChat.ts` 与 `useProject.ts` 重复。

### 5. 提高关键错误可观测性

后端存在较多 `let _ =` 静默吞错，尤其是事件发送、持久化、checkpoint、审计日志写入。

- [ ] 对 `emperor_tx.send` / `dept_log_tx.send` 失败记录日志。
- [ ] 对 `.shuji/` 状态写入失败记录日志。
- [ ] 对 checkpoint 创建失败记录日志并回传可诊断信息。
- [ ] 对 audit 写入失败记录日志。
- [ ] 为用户可见错误保留中文友好提示，为开发者日志保留原始错误。

## P1：降低维护成本

### 6. 拆分前端上帝组件

`ProjectDashboard.tsx` 已承担项目加载、Demo、pending approval、文档 tabs、布局 resize、错误处理、聊天装配等职责。

- [ ] 抽出 `useDocumentTabs`，管理打开、关闭、切换、初始视图。
- [ ] 抽出 `useDemoFlow`，管理 demo 自动发送、tour、完成摘要。
- [ ] 抽出 `usePendingApprovals`，管理 pending approvals 刷新与点击跳转。
- [ ] 抽出 `DashboardLayout`，只负责整体布局。
- [ ] 目标：`ProjectDashboard.tsx` 收敛到 300 行以内。

### 7. 拆分 `SettingsMenu.tsx`

`SettingsMenu.tsx` 体量过大，且 API preset、模型 preset 等配置与 `SetupPage.tsx` 有重复。

- [ ] 抽出 `ApiSettingsTab`。
- [ ] 抽出 `ContextSettingsTab`。
- [ ] 抽出 `WorkflowSettingsTab`。
- [ ] 抽出 `SoulSettingsTab` 或相关设置区域。
- [ ] 将 API URL 与模型 preset 移到共享常量文件。
- [ ] 保持设置保存逻辑集中，避免多个 tab 各自直接写后端。

### 8. 拆分后端 `tool/mod.rs`

`tool/mod.rs` 体量过大，混合了路径安全、文件操作、命令操作、工具分发、缓存、工具 schema。

建议拆分：

- [ ] `tool/path_security.rs`：`resolve_scoped_path` 与路径测试关联逻辑。
- [ ] `tool/file_ops.rs`：read/create/patch/delete/rename。
- [ ] `tool/command_ops.rs`：命令安全检查与执行。
- [ ] `tool/cache.rs`：读缓存。
- [ ] `tool/dispatch.rs`：`execute_named_tool` 分发。
- [ ] `tool/output.rs`：`ToolOutput`。
- [ ] 保留 `tool/mod.rs` 只做模块导出与少量 facade。

每一步拆分后都跑：

```bash
cd shuji-app/src-tauri
cargo test --test tool_test
cargo test --test path_security_test
cargo test --lib
```

### 9. 拆分 `commands/workflow.rs`

`commands/workflow.rs` 同时承担 Actor 启动、事件转发、IPC 命令、状态查询、审计查询。

建议拆分：

- [ ] `commands/workflow/send.rs`：`send_message`、`discuss_with_cabinet`、取消。
- [ ] `commands/workflow/bootstrap.rs`：`start_actor_system`、agent 构建、channel 创建。
- [ ] `commands/workflow/events.rs`：chat/dept-log/plan/milestone 事件转发。
- [ ] `commands/workflow/context.rs`：上下文统计与压缩命令。
- [ ] `commands/workflow/audit.rs`：审计、lineage、diff、trace 查询。
- [ ] `commands/workflow/state.rs`：workflow state/graph/archive 查询。

### 10. 收敛 Workflow 决策入口

当前 `workflow/resolver.rs`、`agent/neige/routing.rs`、skill prompt 中都含流程决策信息，长期容易漂移。

- [ ] 明确唯一入口：推荐 `WorkflowResolver` 作为流程选择唯一入口。
- [ ] `neige/routing.rs` 只保留为 resolver 的启发式子模块。
- [ ] prompt 中只描述原则，不重复具体 gate 表。
- [ ] `GateEngine` 作为工具执行前唯一硬门禁。
- [ ] 为每个 profile 增加一条 mock E2E 或集成测试。

## P2：补齐关键运行时测试

### 11. AgentController 与 watchdog 测试

watchdog 是 LLM 工具系统的重要保护层，但当前直接测试不足。

- [ ] 用 mock session 模拟同工具同参数重复调用，验证干预提示。
- [ ] 模拟连续 read without write，验证提示。
- [ ] 模拟连续 tool error，验证达到上限后停止。
- [ ] 模拟 delete-create 循环，验证提示。
- [ ] 覆盖只读工具并行执行的结果顺序与错误处理。

### 12. 上下文压缩测试

压缩直接影响长会话稳定性。

- [ ] 测 skill 消息压缩前剥离、压缩后重挂。
- [ ] 测 `[对话摘要]` 被保留。
- [ ] 测 JSON 状态行不会丢失。
- [ ] 测 `keep_recent_count` 生效。
- [ ] 测工具结果持久化前截断。

### 13. ActorSystem 轻量集成测试

当前 actor 测试更多覆盖消息结构，而不是真实 actor 生命周期。

- [ ] 两个 mock actor 之间通过 `route_to` 转发。
- [ ] `FastMessage::Interrupt` 能中断目标 actor。
- [ ] 项目切换时旧 actor 被 teardown。
- [ ] 并发 `send_message` 行为明确：拒绝、排队或替换，不要隐式竞态。
- [ ] 前端 cancel 能正确设置全局 cancel flag 并清理残留消息。

### 14. 审计与 checkpoint 测试

- [ ] 覆盖 audit event append。
- [ ] 覆盖文档 lineage 构建。
- [ ] 覆盖 reverse ref index。
- [ ] 覆盖交付报告生成。
- [ ] 覆盖 checkpoint 无变更时不提交。
- [ ] 覆盖 checkpoint restore 的文件与 session 状态恢复。

## P3：性能、稳定性与产品化

### 15. 缓存与全局状态隔离

- [ ] 将 `READ_CACHE` 按项目或 session 隔离。
- [ ] 在 `load_project` / actor teardown 时清空当前项目缓存。
- [ ] 评估 `token_tracker` 是否需要按项目隔离。
- [ ] 评估 `COUNTER_LOCK` 是否需要项目级锁或文件锁。

### 16. async 锁与 channel 背压

- [ ] 审计 async 热路径中的 `std::sync::Mutex`。
- [ ] 对可能跨 await 或高频访问的锁改为 `tokio::sync::Mutex` / `RwLock`。
- [ ] 将关键 `unbounded_channel` 替换为 bounded channel，或至少记录队列深度。
- [ ] 对 dept-log/chat 事件慢消费时的行为制定策略。

### 17. 配置读取与压缩阈值优化

- [ ] 避免 actor 每轮从磁盘读取 `context_config.json`。
- [ ] 任务开始时读取一次并放入 `ActorContext` 或配置缓存。
- [ ] 根据模型窗口动态计算压缩阈值，而不是固定 `750000`。
- [ ] 对工部、中书令等长循环角色默认开启或强化 mid-run compaction。

### 18. 前端轮询与事件订阅收敛

- [ ] 将 pending approvals 改为后端事件推送，或合并到统一 status poll。
- [ ] 将 active roles 与 dept logs 提供为统一 Context。
- [ ] 避免多个组件重复 `listen("dept-log")`。
- [ ] 统一 token/context 刷新策略，减少分散定时器。

### 19. 前端类型收紧与死代码清理

- [ ] 将 `ChatMessage.role` 从 `string` 收紧为部门/用户联合类型。
- [ ] 消除 `any`，尤其是 token 聚合与图数据处理。
- [ ] 删除或迁移未使用的 `DocumentViewer.tsx`。
- [ ] 统一所有 catch 使用 `formatError`。
- [ ] 将常用 Modal、Select、Toast、Tabs 扩展到 `components/ui/`。

### 20. 真正取消讨论模式

当前廷议取消主要是前端忽略返回结果，后端请求仍可能继续消耗 API。

- [ ] 为 `discuss_with_cabinet` 增加取消句柄。
- [ ] 前端 `cancelDiscuss` 调用后端取消。
- [ ] 与决策模式的 `cancelProcessing` 行为对齐。
- [ ] 增加取消状态测试。

## P4：文档与项目治理

### 21. 同步过时文档

- [ ] 更新 `README.md` 中测试数量与测试文件描述。
- [ ] 更新 `CLAUDE.md` 中测试数量、命令、当前架构说明。
- [ ] 更新 `shuji-app/TEST_FLOW.md` 中旧部门名、旧工具名、旧流程描述。
- [ ] 确认 `ARCHITECTURE.md` 与当前实现一致。
- [ ] 标注 `mailbox_design.md` 为未来设计，不参与当前实现验收。

### 22. 增加贡献与本地开发说明

- [ ] 新增 `CONTRIBUTING.md`。
- [ ] 写明本地推荐检查命令。
- [ ] 写明真实 API 测试如何运行、何时跳过。
- [ ] 写明 `.env` / `api_config.json` / `config.local.toml` 的关系。
- [ ] 写明 checkpoint 自动 commit 策略与注意事项。

建议本地提交前检查：

```bash
cd shuji-app/src-tauri
cargo fmt --check
cargo clippy --all-targets -- -W clippy::all
cargo test --lib
cargo test --tests -- --skip expand_requirements --test-threads=1

cd ../
npm run format:check
npm run lint
npm run build
npm test
```

### 23. CI 扩展

- [ ] CI 加入 `npm run format:check`。
- [ ] CI 加入前端测试。
- [ ] 增加 `windows-latest` 轻量 job：`cargo check` + `cargo test --lib`。
- [ ] 可选：nightly job 运行真实 API 的 `expand_requirements_test`，失败不阻塞主分支。

## 推荐执行顺序

如果只能按最小风险路线推进，建议按下面顺序：

1. P0-3：修 Prettier / CI 断裂。
2. P0-1：建立前端测试基建。
3. P0-2：补朱批门禁测试。
4. P0-4：理清聊天状态单一真相源。
5. P1-6：拆 `ProjectDashboard.tsx`。
6. P1-7：拆 `SettingsMenu.tsx`。
7. P1-8：拆 `tool/mod.rs`。
8. P1-10：收敛 Workflow 决策入口。
9. P2-11 / P2-12：补 AgentController/watchdog 与 compact 测试。
10. P4-21：同步所有文档，避免新贡献者被旧说明误导。

## 阶段性里程碑

### 里程碑 A：回归安全网

完成条件：

- [ ] 前端测试能跑。
- [ ] 朱批门禁有集成测试。
- [ ] CI 跑前端测试和格式检查。
- [ ] README/CLAUDE 测试数量更新。

收益：后续重构不再完全依赖手工验收。

### 里程碑 B：前端可维护性

完成条件：

- [ ] `ProjectDashboard.tsx` 小于 300 行。
- [ ] `SettingsMenu.tsx` 按 tab 拆分。
- [ ] 聊天状态单一真相源明确。
- [ ] dept-log / active roles 订阅收敛。

收益：UI 功能迭代和 bug 修复成本明显下降。

### 里程碑 C：后端模块化

完成条件：

- [ ] `tool/mod.rs` 拆分。
- [ ] `commands/workflow.rs` 拆分。
- [ ] workflow 决策入口统一。
- [ ] 关键 `let _ =` 改为可观测日志。

收益：后端核心模块更容易审查、测试和扩展。

### 里程碑 D：长会话稳定性

完成条件：

- [ ] compact 模块有测试。
- [ ] AgentController watchdog 有测试。
- [ ] actor 中断与项目切换有集成测试。
- [ ] 缓存和 token 统计按项目隔离或明确生命周期。

收益：真实长任务、跨项目使用、异常恢复更可靠。

## 不建议优先做的事

- [ ] 暂时不要继续增加新部门或新治理流程。
- [ ] 暂时不要大改 UI 视觉风格。
- [ ] 暂时不要引入大型状态管理库来一次性重写前端。
- [ ] 暂时不要把 mailbox future design 直接落地，除非当前 Actor 模型的瓶颈已被测试证明。
- [ ] 暂时不要扩大 prompt 复杂度；优先把流程规则移到可测试代码和配置中。

## 最终目标

优化完成后的理想状态：

- 后端核心流程有稳定自动化测试。
- 前端关键交互有基础测试。
- 用户能清楚看到系统卡在哪个部门、哪个文档、哪个工具。
- 文档、代码、CI 口径一致。
- Workflow Profile 是流程行为的单一真相源。
- 大文件被拆分为可审查、可局部测试的模块。
- 新功能开发优先修改结构化配置和小模块，而不是继续堆 prompt 与巨型文件。

---

## 执行进度

| 优先级 | 任务 | 状态 | 完成日期 | 说明 |
|--------|------|------|----------|------|
| P0-3 | 修 Prettier / CI 断裂 | ✅ | 2026-06-09 | Prettier 加入依赖，CI 格式检查 |
| P0-1 | 建立前端测试基建（79 tests） | ✅ | 2026-06-09 | Vitest + RTL |
| P0-2 | 补朱批与门禁测试（+16 tests） | ✅ | 2026-06-09 | document_test 24 tests |
| P0-4 | 修复聊天状态多源问题 | ✅ | 2026-06-09 | 单一真相源 + mergeMessages |
| P1-6 | 拆分 ProjectDashboard.tsx（650→380行） | ✅ | 2026-06-09 | 抽出 useDocumentTabs/useDemoFlow/usePendingApprovals |
| P1-7 | 拆分 SettingsMenu.tsx（580→240行） | ✅ | 2026-06-09 | 4 tab 组件：Api/Context/Workflow/Soul |
| P1-8 | 拆分 tool/mod.rs（2475→45行，12模块） | ✅ | 2026-06-09 | cache/path/file_ops/command_ops/dispatch/neige_special/audit_tools |
| P1-10 | 收敛 Workflow 决策入口 | ✅ | 2026-06-09 | GateEngine/ChainEngine + refactor/audit profile |
| P2-11/12 | AgentController/watchdog/compact 测试（+11 tests） | ✅ | 2026-06-09 | session_control_test + config_test 扩展 |
| P4-21 | 同步过时文档 | ✅ | 2026-06-09 | ARCHITECTURE.md + CLAUDE.md 更新 |
| **P0-5** | **提高关键错误可观测性** | **✅** | **2026-06-10** | **emperor_tx/dept_log_tx/milestone_tx send 错误日志 + audit::append 日志化** |
| **P1-9** | **拆分 commands/workflow.rs** | **✅** | **2026-06-10** | **5 子模块：bootstrap/send/context/query/audit，1168→35 行** |
| **P2-13** | **ActorSystem 轻量集成测试** | **✅** | **2026-06-10** | **+4 tests：FastMessage 中断、cancel flag、teardown、route_to 跨 actor** |
| P2-14 | 审计与 checkpoint 测试 | ⬜ | — | audit append、lineage、reverse ref |
| P3-15 | 缓存与全局状态隔离 | ⬜ | — | READ_CACHE 按项目/session 隔离 |
| P3-16 | async 锁与 channel 背压 | ⬜ | — | std::sync::Mutex 审计、bounded channel |
| P3-17 | 配置读取与压缩阈值优化 | ⬜ | — | 避免每轮读 context_config.json |
| P3-18 | 前端轮询与事件订阅收敛 | ⬜ | — | 统一 status poll、避免重复 listen |
| P3-19 | 前端类型收紧与死代码清理 | ⬜ | — | ChatMessage.role 联合类型、消除 any |
| P3-20 | 真正取消讨论模式 | ⬜ | — | discuss_with_cabinet 取消句柄 |
| P4-22 | 增加贡献与本地开发说明 | ⬜ | — | CONTRIBUTING.md |
| P4-23 | CI 扩展 | ⬜ | — | windows-latest job、nightly real API test |
