# 枢机（ShuJi）项目记忆

> LLM 多智能体协作自动化软件开发系统。PoC / prototype 阶段。
> 作者：Fakerol。单开发者项目，无测试。

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面壳 | Tauri v2 (Rust 后端) |
| 前端 | React 19 + TypeScript + Vite 6 + TailwindCSS 4 |
| LLM API | DeepSeek (默认) / Anthropic (自动检测 URL 格式) |
| 异步 | tokio (full features) |
| 序列化 | serde + serde_json |
| 路由 | react-router-dom v7 |

## 核心架构

### 11 Agent 三省六部制

**决策层**
- `皇帝` (用户) → 输入需求，做最终决策
- `内阁` (neige) → 与皇帝对话，skill 系统动态切换工作模式，路由任务

**设计层**
- `中书令` (zhongshuling) → 方案设计（3 skills: overall_design, phase_plan, phase_design）
- `门下侍中` (menxiashizhong) → 设计审查（与给事中合并）

**执行层**
- `尚书令` (shangshuling) → 任务调度
- `吏部尚书` (libushangshu) → 详细设计
- `兵部尚书` (bingbushangshu) → 测试 + 契约
- `工部尚书` (gongbushangshu) → 编码实现（写代码）
- `刑部尚书` (xingbushangshu) → 测试验证
- `礼部尚书` (liburshangshu) → 规范检查
- `户部` → 记录归档

**监督**
- `制司` (zhisi) → 独立审计

### 消息流

```
用户 → send_message (Tauri command) → ActorSystem → 内阁(LLM)
  → <skill> 切换模式 → <route> 路由 → 各部门(独立 tokio::spawn actor)
  → 结果通过 emperor_tx → 前端 chat-message 事件
```

- 部门间通过 **文档** 通信（`.shuji/` 目录），不直接调用
- 每个 Actor 是 `tokio::spawn` + `mpsc::UnboundedReceiver` mailbox
- `dept_log_tx` → 前端 DeptStatusPanel (dept-log 事件)
- `milestone_tx` → 持久化到 `.shuji/state.json`

### Session / AgentController 分离

- `Session` (api/session.rs): 纯 LLM 层，管理消息历史、API 调用、finish_reason=length 自动重试（减半 max_tokens）
- `AgentController` (api/control.rs): 驱动循环，工具执行、结果反馈、取消/中断/重启、看门狗诊断

### 文档中心架构

文档类型：dsgn（设计）、plan（计划）、pdsg（阶段设计）、ddtl（详细设计）、revw（审查）、task（任务）、ctrt（契约）、rprt（报告）

存储在 `.shuji/designs/`、`.shuji/reviews/` 等目录，YAML frontmatter + 自动编号（`_counter`）。

### 内阁 Skill 系统

7 个 skills：clarify、workflow_demo、workflow_simple、workflow_standard、workflow_complex、discuss、summary

通过 LLM 输出 `<skill>name</skill>` 标签动态切换。

## Session 限制关键参数

| 参数 | 值 | 适用角色 |
|---|---|---|
| write_file max_tokens | 2048 | 兵部、工部 |
| append_document max_tokens | 1536 | 中书令、吏部、刑部 |
| read-only max_tokens | 1024 | 礼部 |
| write_file 工具迭代上限 | 120 | 兵部、工部 |
| append_document 工具迭代上限 | 100 | 中书令、吏部、刑部 |
| read-only 工具迭代上限 | 80 | 礼部 |
| finish_reason=length 重试 | 5 次（减半 max_tokens） | 所有 |
| 连续工具错误上限 | 5 → auto-stop | 所有 |
| create/append/modify 单次内容 | ≤500 chars | 工部、兵部 |
| 上下文压缩阈值 | ~160k chars | config.toml |

## 前端结构

```
shuji-app/src/
├── pages/
│   ├── WorkspaceSelect.tsx    # 项目选择/创建
│   ├── ProjectDashboard.tsx   # 主聊天 UI + 仪表盘（17 个 useState，过重）
│   └── LogsPage.tsx           # 部门日志查看
├── components/
│   ├── ChatBubble.tsx         # 消息气泡 + <options> 渲染
│   ├── ChatInput.tsx          # 输入框 + 斜杠命令
│   ├── DeptStatusPanel.tsx    # 部门实时状态
│   └── WorkflowTimeline.tsx   # 进度可视化
├── api.ts                     # Tauri invoke 封装
└── types.ts                   # TypeScript 类型定义
```

## 后端结构（Rust）

```
shuji-app/src-tauri/src/
├── main.rs / lib.rs           # 入口 + Tauri 插件注册
├── commands/                  # Tauri IPC 命令
│   ├── project.rs             # 项目 CRUD
│   ├── workflow.rs            # send_message / discuss
│   └── settings.rs            # .env 配置加载
├── actor/mod.rs               # Actor 系统
├── agent/                     # 各 Agent 实现
│   ├── trait.rs / util.rs     # Agent trait + 工具函数
│   ├── neige/                 # 内阁（prompt.md + 7 skills）
│   ├── zhongshuling/          # 中书令（3 skills）
│   ├── menxiashizhong/        # 门下侍中
│   └── ... (各执行部门)
├── api/                       # LLM API 层
│   ├── client.rs              # HTTP 客户端（Anthropic/OpenAI 双格式）
│   ├── session.rs             # 会话管理、重试
│   └── control.rs             # 控制流、看门狗
├── tool/                      # 工具系统
│   ├── mod.rs                 # 中央调度 + 路径安全防护
│   └── documents.rs           # 文档工具
├── models/                    # 数据模型
├── storage/shuji_dir.rs       # .shuji/ 文件系统
├── logging/logger.rs          # 部门 JSONL 日志
└── token_tracker.rs           # Token 用量追踪
```

## 项目中发现的代码质量问题

### Rust 后端

| 严重度 | 位置 | 问题 |
|--------|------|------|
| 中 | `actor/mod.rs:286` | 异步上下文中间使用同步 `std::fs::write` |
| 中 | `session.rs:614` | `write_debug_truncated` 用同步 `std::fs` |
| 中 | `neige/mod.rs:227` | `block_in_place` + `block_on` 可能死锁 |
| 低 | `tool/mod.rs:33` | `rel.contains("..")` 会误伤合法文件名 |
| 低 | `tool/mod.rs:414` | 写死 `bash`，不支持 Windows |
| 低 | `control.rs:205` | `tc.name.contains("write")` 字符串脆皮匹配 |

### 前端

| 严重度 | 位置 | 问题 |
|--------|------|------|
| 中 | ProjectDashboard.tsx | 6+ 处空 `.catch(() => {})`，错误静默吞没 |
| 中 | 同上 | 17 个 useState，组件过重 |
| 低 | ChatBubble.tsx:55 | non-null assertion 可能炸 |
| 低 | ChatBubble.tsx:91 | 硬编码中文 label 检查 |
| 低 | 多处 | index as React key |
| 低 | package.json | 无测试框架 |

### 总体评价

- **方向**：有趣且有创意。LLM Agent 编排做软件自动化的探索。
- **架构**：Actor 模式、Session/Controller 分离、路径安全防护设计得当。
- **状态**：PoC / prototype。**无测试，核心能力未被验证能生成可工作的真实代码。**
- **成本**：完整流程需调动 11 个 Agent × 多轮工具调用，token 消耗极大。
- **可靠性**：高度依赖 LLM 输出格式一致性（route/skill 标签、工具调用格式），链条脆弱。
- **一句话**：架构有想法，离实用还有很大距离。
