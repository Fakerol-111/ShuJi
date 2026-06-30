# 维护者索引（MAINTAINER_INDEX）

> 文件级索引与维护者速查。架构叙事见 [ARCHITECTURE.md](ARCHITECTURE.md)；开发指南见 [../../CONTRIBUTING.md](../../CONTRIBUTING.md)。
>
> **核对日期**：2026-06-30（基于 `shuji-app/src-tauri/src/` 与 `shuji-app/src/` 实际目录结构）。

---

## 关键文件位置

```
shuji-app/
├── src/                              # 前端 (React + Vite + Tailwind CSS 4 + Vitest)
│   ├── pages/                        # WorkspaceSelect, ProjectDashboard, LogsPage, SettingsPage, SetupPage
│   ├── components/
│   │   ├── ui/                       # 原始 UI kit (Button, Card, Tabs, 等)
│   │   ├── ChatBubble.tsx            # <options> 可点按钮
│   │   ├── ChatInput.tsx / ChatPanel.tsx
│   │   ├── CommandBar.tsx            # Pipeline 阶段命令栏
│   │   ├── DeptStatusPanel.tsx       # 实时部门状态
│   │   ├── DeptCard.tsx / DeptCardRail.tsx / DeptInspector.tsx  # 部门详情视图
│   │   ├── ReasoningPopover.tsx      # LLM reasoning/thinking 内容展示
│   │   ├── DocPreview.tsx / DocTree.tsx  # 文档浏览器
│   │   ├── DecisionPanel.tsx / AuditPanel.tsx  # 决策/审计 tabs
│   │   ├── CheckpointPanel.tsx       # Checkpoint 快照列表/恢复
│   │   ├── TokenPanel.tsx / ContextPanel.tsx  # 侧栏面板
│   │   ├── ProjectOverview.tsx / WorkflowTimeline.tsx
│   │   ├── HelpDrawer.tsx / DemoTour.tsx
│   │   ├── SettingsMenu.tsx / SealLogo.tsx
│   │   └── settings/                 # 设置 tabs: ApiSettingsTab, ReasoningSettingsTab 等
│   ├── hooks/                        # React hooks: useChat, useClickOutside, usePendingApprovals 等
│   ├── utils/                        # chat.ts, error.ts, approvalGate.ts 等
│   ├── constants/                    # constants.ts, reasoning.ts, presets.ts
│   ├── api.ts                        # Tauri invoke 封装
│   ├── types.ts                      # TypeScript 类型定义 (RoleName union 等)
│   └── test/setup.ts                 # Vitest setup (jsdom, testing-library)
└── src-tauri/src/
    ├── commands/                     # Tauri 命令处理器
    │   ├── project.rs                # 项目 CRUD + demo 生成器
    │   ├── settings.rs               # .env + api_config.json 加载, 模型预设
    │   ├── checkpoint.rs             # 列出/恢复 checkpoints
    │   ├── shuji_docs.rs             # .shuji/ 文件树 + 文档查看器
    │   ├── pricing.rs                # 定价系统
    │   ├── metrics.rs                # 运行指标
    │   ├── validate.rs               # 交付验证命令
    │   └── workflow/                 # send_message, compact, context_stats, audit, bootstrap
    ├── actor/mod.rs                  # Actor 系统: run_actor, ActorContext, FastMessage/FastChannel
    ├── agent/
    │   ├── trait.rs                  # Agent trait + AgentInput/Output, LoopDecision
    │   ├── runner.rs                 # 共享执行框架 (compact/checkpoint/context 辅助)
    │   ├── util.rs                   # 标签提取辅助
    │   ├── expand_requirements.rs    # 需求展开 sub-agent
    │   ├── survey_codebase.rs        # 代码库调查 sub-agent
    │   ├── neige/                    # 内阁: mod.rs, prompt.md, routing.rs, skills/ (12 .md)
    │   ├── zhongshuling/             # 中书令: mod.rs, prompt.md, skills/ (7 skills)
    │   ├── menxiashizhong/           # 门下侍中: mod.rs, prompt.md, skills/ (2 skills)
    │   ├── shangshuling/             # 尚书令: mod.rs, prompt.md
    │   ├── libushangshu/             # 吏部: mod.rs, prompt.md
    │   ├── bingbushangshu/           # 兵部: mod.rs, prompt.md
    │   ├── gongbushangshu/           # 工部: mod.rs, prompt.md (批次计划循环)
    │   ├── xingbushangshu/           # 刑部: mod.rs, prompt.md
    │   └── liburshangshu/            # 礼部: mod.rs, prompt.md
    ├── api/
    │   ├── client.rs                 # AnthropicClient (双格式 HTTP)
    │   ├── session/                  # mod.rs (Session, PersistedContext, step), persisted_context.rs
    │   ├── control/                  # mod.rs (AgentController), iterations.rs, types.rs
    │   ├── compact/                  # 上下文压缩 (2 提示词变体)
    │   ├── reasoning.rs              # 每厂商 reasoning/thinking token 注入
    │   ├── intent.rs                 # 用户意图分类
    │   ├── stream.rs                 # 流式响应处理
    │   └── token_count.rs            # Token 计数工具
    ├── tool/
    │   ├── registry.rs               # 工具组工厂函数
    │   ├── dispatch.rs               # 中央工具调度 + 门禁逻辑
    │   ├── file_ops/ / documents/    # 文件与文档操作
    │   ├── command_ops.rs / editor.rs / lint_ops.rs / python_cmd.rs / test_env.rs
    │   ├── audit_tools.rs / neige_special.rs / shangshuling_special.rs
    │   ├── cache.rs / path.rs / output.rs / tool_log.rs
    ├── pipeline/                     # engine.rs, schema.rs, artifacts.rs, handlers.rs, supervisor.rs, templates.rs
    ├── workflow/                     # graph.rs, stage.rs, state.rs, profiles/
    ├── validate/                     # 交付验证: contract, lint, diff, tests_runner
    ├── learning/                     # 角色学习: store, extract, inject, config
    ├── metrics/                      # 指标聚合
    ├── scenario/                     # 场景重放框架
    ├── precepts/                     # 规范/策略管理
    ├── audit/                        # mod.rs, document_line.rs
    ├── config/                       # mod.rs (RuntimeConfig), esaa_contract.rs
    ├── models/                       # role.rs, chat.rs, message.rs, project.rs, dept_step.rs
    ├── storage/                      # shuji_dir.rs, checkpoint.rs
    ├── logging/logger.rs             # 部门级 JSONL 日志
    ├── playbook/                     # Watchdog playbook 模式
    ├── templates/                    # 文档模板
    ├── round_metrics.rs / token_tracker.rs
    └── lib.rs                        # Tauri builder, 插件注册
```

---

## Session Limits（config.toml 可配置）

| 设置 | 默认 | Agent |
|---|---|---|
| write_file max_tokens | 0（无限） | 兵部、工部 |
| append_document max_tokens | 4096 | 中书令、吏部、刑部 |
| 只读 max_tokens | 2048 | 礼部 |
| 纯文本 max_tokens | 1024 | 内阁、尚书令、门下侍中 |
| 只读工具迭代 | 80 | 礼部 |
| 写密集工具迭代 | 60 | 兵部、工部 |
| 文档密集工具迭代 | 100 | 中书令、吏部、刑部 |
| finish_reason=length 重试 | 5（每次减半） | 全部 |
| 连续工具错误 | 5 → 自动停止 | 全部 |
| 最大计划循环迭代 | 6 | 工部 |
| 单 actor 最大执行迭代 | 20 | 全部 |
| Checkpoint 间隔 | 300s | 全部 |
| 同工具重复警告 | 3 | 全部 |
| 只读不写警告 | 5 | 全部 |
| 审批自动放行重试 | 3 | 内阁 |

---

## Edge Cases Handled

- **截断的工具调用**：过滤 assistant 消息仅留有效 `tool_call_id`（防 400 错误）
- **所有工具调用损坏**：返回 `StepResult::Text` 而非空 `ToolCalls`（防死循环）
- **孤立 tool 消息**：两遍 sanitize——先收集所有 ID，再过滤（消除顺序相关竞争）
- **Soul 消息漂移**：`PersistedContext` 单独存储 `soul_prompt`，保存/加载时保留其在 base 与 skill prompt 间的位置
- **Windows CRLF**：`log_console!` 用 `write!` + 显式 `\n` 而非 `eprintln!`
- **技能循环去重**：内阁连续输出同一 `<skill>` 标签两次则中断循环
- **自路由预防**：base prompt 禁止 `route_to(to="内阁")`
- **必须批准重提示守卫**：连续 3 次无 `<options>` → 自动放行继续
- **压缩并发安全**：AppState 中活跃角色追踪 + `compacting_roles` 防双击；原子 tmp+rename 写入
- **路径安全**（`resolve_scoped_path`）：拒绝绝对路径与 `..` 穿越。回退到祖先遍历 + canonicalize。捕获符号链接逃逸攻击。
- **命令安全**（`check_safe_command`）：token 匹配。阻断 `sudo rm`、`format X:`、`shutdown`、`mkfs`、`dd`、`wget`/`curl` 到外部 URL。

---

## Token Tracking

两套并行系统：

- **`token_tracker.rs`**：持久化 JSON，每次调用记录（prompt/cached/uncached/completion），按时间窗口聚合（今日/近3天/近7天/汇总）。通过 `get_token_stats` 命令暴露。
- **`round_metrics.rs`**：内存实时，追踪当前角色、技能、累计 token 含缓存拆分、部门迭代。通过 `get_round_metrics` 命令暴露。

缓存字段从 API 响应解析：OpenAI `usage.prompt_tokens_details.cached_tokens` 或 Anthropic `usage.cache_read_input_tokens`。

---

## Project State Persistence

- `Project.talk`：追加式，自动修剪到 ~12 条（最旧 → 摘要）
- `Project.task`：里程碑（追加式，不修剪）
- `Project.summary`：单行状态，自动更新
- 每个里程碑事件持久化到 `.shuji/state.json`

---

## API Dual-Format

单一 `AnthropicClient` 结构体按请求自动检测格式：

- URL 含 `anthropic.com` → Anthropic Messages API（`x-api-key` header）
- 否则 → OpenAI Chat Completions API（`Bearer` auth）
- 非 Anthropic API 自动启用 reasoning/thinking tokens

---

## Code Style

- **Rust**：`cargo fmt`（4 空格），提交前清理 `clippy` 警告，优先 `Result<_, String>` / `anyhow::Result<_>`，避免 `unwrap()`
- **TypeScript/React**：Prettier（`npm run format`），`ChatMessage.role` 为 `RoleName` 联合类型，新通用组件放 `components/ui/`，新 hook 以 `use` 开头放 `hooks/`
- **事件命名**：Tauri 事件使用 kebab-case：`chat-message`、`dept-log`、`plan-update`、`project-update`

---

## 导航

- 架构叙事 → [ARCHITECTURE.md](ARCHITECTURE.md)
- 开发指南 / 测试命令 → [../../CONTRIBUTING.md](../../CONTRIBUTING.md)
- 新人导读 → [../../ONBOARDING.md](../../ONBOARDING.md)
- 项目介绍 → [../../README.md](../../README.md)
