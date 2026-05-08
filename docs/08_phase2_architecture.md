# Phase 2：Rust + Tauri 独立引擎

> 解决三大问题：框架越界、日志缺失、角色上下文混用

**皇帝御批：**
- 前端框架：React + TypeScript
- 工作流模式：全自动（编排器推进到需要皇帝决策才暂停，且阶段间可并行）
- 项目位置：`shuji-app/` 目录，与 docs/ 并列

---

## 背景

当前 MVP 跑在 Claude Code 单会话中，三省六部是"口头角色扮演"。本质问题：

- **一个会话 = 所有角色共享上下文** → 中书省的犹豫、门下省的吐槽、兵部的测试全混在一起
- **没有架构强制力** → Claude 可以跳过步骤、不写日志，全靠自觉
- **Phase 2 目标**：Rust + Tauri 独立桌面应用，每个角色是独立的 API 调用，各自维护上下文窗口

## 工作目录模式（.shuji/）

像 Claude Code 一样工作：用户在任何目录下打开 ShuJi，在该目录下生成 `.shuji/` 文件夹，作为项目的完整上下文环境。

```
D:\Projects\MyERP/           ← 用户的工作目录
├── .shuji/                  ← ShuJi 创建的隐藏文件夹
│   ├── state.json           ← 项目状态
│   ├── contexts/            ← 各角色独立上下文
│   │   ├── zhongshu.json
│   │   ├── mensheng.json
│   │   ├── neige.json
│   │   └── ...
│   ├── designs/             ← 设计文档
│   ├── reviews/             ← 审查报告
│   ├── reports/             ← 奏折
│   ├── logs/                ← 日志
│   └── execution/           ← 任务拆解等
├── (用户的代码文件)
├── main.py
└── ...
```

ShuJi 可同时在多个目录为不同项目工作，各自独立（类似 Claude Code 的并行会话）。

## 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                 Emperor UI (React)                        │
│  工作目录选择 │ 项目详情 │ 文档查看 │ 审批面板 │ 设置     │
└──────────────────────────┬──────────────────────────────┘
                           │ Tauri IPC (invoke)
┌──────────────────────────▼──────────────────────────────┐
│              Rust Backend Engine                          │
│                                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │ Orchestrator  │  │  State       │  │  Context     │   │
│  │  (workflow)   │  │  Machine     │  │  Manager     │   │
│  └──────┬───────┘  └──────┬───────┘  │  (per-role)  │   │
│         │                 │          └──────────────┘   │
│         ▼                 ▼                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │  API         │  │  Storage     │  │  Logger      │   │
│  │  Client      │  │  (.shuji/)  │  │  (auto-JSON) │   │
│  └──────────────┘  └──────────────┘  └──────────────┘   │
│                                                           │
│  ┌──────────────┐                                        │
│  │  Agent Trait │ ← MockAgent (W1) / APIAgent (W2)      │
│  └──────────────┘                                        │
└───────────────────────────────────────────────────────────┘
```

### 核心变更 vs MVP 模式

| MVP (Claude Code) | Phase 2 (Rust + Tauri) |
|---|---|
| 所有角色在同一个对话中切换 | 每个角色独立的 API 调用，独立上下文 |
| 靠 CLAUDE.md 约束行为 | 状态机 + 编排器强制执行流程 |
| 手动写日志 | 每次状态切换自动写日志 |
| 角色"口头声明" | Agent Trait，由 system prompt + context 定义 |
| 用户在聊天框交互 | 用户在 UI 面板审批决策 |
| 项目在 projects/ 下管理 | 在用户指定目录的 .shuji/ 下管理 |

## 组件说明

### Agent Trait（核心抽象）

```rust
#[async_trait]
trait Agent {
    fn role(&self) -> Role;
    async fn execute(&self, input: AgentInput) -> Result<AgentOutput>;
}

struct AgentInput {
    role: Role,
    task_description: String,
    context_messages: Vec<Message>,
    project_dir: PathBuf,
    working_dir: PathBuf,  // 用户工作目录
}

struct AgentOutput {
    content: String,              // 角色产出文本
    documents: Vec<Document>,     // 产出文档列表
    decision: Option<Decision>,   // 涉及决策时填充
}
```

- `MockAgent` — Week 1 实现，返回预设内容，可随机驳回
- `APIAgent` — Week 2 实现，调 Anthropic API

### 状态机

项目全局 + 各阶段并行状态：

```rust
// 项目运行态
struct ProjectRuntime {
    overall: PhaseStatus,             // 整体方案阶段
    phases: Vec<PhaseRuntime>,        // 各阶段，固定 3 个
}

struct PhaseRuntime {
    index: u32,
    design: DesignPhaseStatus,
    execution: ExecutionPhaseStatus,
}

// 整体方案 / 各阶段设计共用
enum DesignPhaseStatus {
    NotStarted,           // 未开始
    Designing,            // 设计中（中书省）
    Reviewing,            // 审查中（门下省）
    PendingApproval,      // 待皇帝批
    Rejected(u32),        // 驳回（计数）
    Escalated,            // 驳回升级（3 次到皇帝）
    Approved,             // 已批准
}

enum ExecutionPhaseStatus {
    NotStarted,           // 未开始
    TaskBreakdown,        // 吏部任务拆解
    Testing,              // 兵部写测试
    Implementing,         // 工部编码
    Checking,             // 刑部检查
    Standards,            // 礼部检查
    Logging,              // 户部记录
    Blocked { reason: String },  // 阻塞（需皇帝决策）
    MiniorIssue,          // 轻微问题（尚书省自修）
    Completed,            // 已完成
}
```

编排器逻辑：

```
loop {
    // 整体方案
    if overall == NotStarted → 调中书省设计
    if overall == PendingApproval → 暂停等皇帝
    
    // 各阶段并行
    for each phase:
        if design == NotStarted and (前一阶段已批准 or 阶段1) → 开始设计
        if design == PendingApproval → 暂停等皇帝
        if design == Rejected(n) → 调中书省修改
        if approved and execution == NotStarted → 开始执行
        if execution == Blocked → 暂停等皇帝决策
        if execution == MinorIssue → 尚书省自行处理，不暂停
    
    // 执行反馈
    if 执行中发现问题 that 需要改设计 → 暂停该阶段执行，退回设计
    
    // 交付判断
    if 所有阶段 completed → 交付
}
```

### 上下文管理器

每个角色独立的对话历史，存 `.shuji/contexts/<role>.json`：

```json
[
  { "role": "system", "content": "你是中书省...", "timestamp": "..." },
  { "role": "user", "content": "皇帝目标是...", "timestamp": "..." },
  { "role": "assistant", "content": "臣建议...", "timestamp": "..." }
]
```

### API 客户端（第 2 周）

封装 Anthropic Messages API：
- 支持 system / user / assistant 角色
- 支持流式和非流式
- 超时重试、错误处理

### 存储层

操作 `.shuji/` 目录结构：
- 项目状态读写
- 文档读写（designs/, reviews/, reports/）
- 日志追加
- 工作目录监听（检测已有 .shuji/）

### 日志系统

每次状态切换自动写入 JSONL：

```json
{"id":"序-001","timestamp":"...","source":"编排器","type":"状态转移","from":"设计中","to":"审查中","summary":"整体方案设计完成","ref":{"projectPath":"..."}}
```

## 技术栈

| 层 | 技术 | 理由 |
|---|---|---|
| 桌面框架 | **Tauri v2** | 跨平台、小体积、Rust 原生 |
| 后端语言 | **Rust** | 性能、安全、Tauri 原生语言 |
| 前端框架 | **React + TypeScript** | 生态最广，市场价值高 |
| 构建工具 | **Vite** | Tauri 推荐，构建速度快 |
| HTTP 客户端 | **reqwest** | Rust 标准 HTTP 库 |
| 序列化 | **serde / serde_json** | Rust 标准 |
| Markdown 渲染 | **react-markdown** | 查看设计文档 / 奏折 |
| 样式 | **Tailwind CSS** | 快速出 UI |

## 项目目录结构

```
shuji-app/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs               # Tauri 入口
│   │   ├── lib.rs                # 库根
│   │   ├── commands/             # IPC 命令
│   │   │   ├── mod.rs
│   │   │   ├── project.rs        # create/load/list/open
│   │   │   ├── workflow.rs       # execute_step, make_decision
│   │   │   └── settings.rs       # api_key, model
│   │   ├── state_machine/        # 状态机引擎
│   │   │   ├── mod.rs
│   │   │   ├── states.rs         # 状态定义
│   │   │   └── transitions.rs    # 转移规则
│   │   ├── orchestrator/         # 编排器
│   │   │   ├── mod.rs
│   │   │   └── engine.rs         # 流程引擎
│   │   ├── agent/                # Agent Trait + 实现
│   │   │   ├── mod.rs
│   │   │   ├── trait.rs          # Agent trait 定义
│   │   │   └── mock.rs           # MockAgent（第1周）
│   │   ├── context/              # 上下文管理
│   │   │   ├── mod.rs
│   │   │   └── manager.rs
│   │   ├── api/                  # API 客户端（第2周）
│   │   │   ├── mod.rs
│   │   │   └── client.rs
│   │   ├── storage/              # .shuji/ 文件系统
│   │   │   ├── mod.rs
│   │   │   └── shuji_dir.rs
│   │   ├── logging/              # 日志
│   │   │   ├── mod.rs
│   │   │   └── logger.rs
│   │   └── models/               # 数据模型
│   │       ├── mod.rs
│   │       ├── project.rs
│   │       ├── role.rs
│   │       ├── document.rs
│   │       └── message.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                          # React 前端
│   ├── App.tsx
│   ├── main.tsx
│   ├── pages/
│   │   ├── WorkspaceSelect.tsx   # 选择/打开工作目录
│   │   ├── ProjectDashboard.tsx  # 项目主面板
│   │   └── Settings.tsx          # 设置（第4周）
│   ├── components/
│   │   ├── WorkflowTimeline.tsx  # 流程进度时间线
│   │   ├── DecisionPanel.tsx     # 皇帝审批（A/B/C/D/E）
│   │   ├── DocumentViewer.tsx    # Markdown 查看
│   │   └── PhasePanel.tsx        # 单阶段面板
│   └── styles/
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## 实施阶段

### 第 1 周：完整流程 + Mock Agent

> 目标：从选择工作目录 → 输入目标 → 全自动跑通全流程（含随机驳回、阶段并行、执行反馈），所有 Agent 为 Mock

| 子任务 | 产出 |
|--------|------|
| 1.1 Tauri 项目初始化 | `cargo tauri dev` 可启动 |
| 1.2 数据模型 | Project, Role(含LiBu_P/LiBu_R), Message, Document, 状态枚举 |
| 1.3 状态机 | 全部状态定义 + 转移表 + 单元测试 |
| 1.4 Agent Trait + MockAgent | Agent trait + 每个角色的 mock 实现（含随机驳回） |
| 1.5 编排器 | 全自动推进，3阶段并行，执行→设计反馈 |
| 1.6 存储层 | .shuji/ 目录创建、状态读写、文档读写、日志追加 |
| 1.7 前端 - 工作目录选择 | 打开/选择目录，最近目录列表 |
| 1.8 前端 - 项目主面板 | 项目信息、流程时间线、阶段面板、决策面板、文档查看 |
| 1.9 IPC 联调 | 全部 IPC 命令对接前端 → 编排器完整走通 |

**第 1 周验收标准：**
1. 打开 ShuJi → 选择一个空目录 → 输入目标（"做一个ERP"）
2. 点击"开始" → 编排器全自动推进
3. 观察流程：整体方案设计 → 门下省审查（可能驳回）→ 皇帝审批
4. 皇帝决策后 → 阶段 1 设计 → 审批 → 执行 → 阶段 2 设计（与阶段 1 执行并行）
5. 执行中遇到问题 → 退回设计修改 → 继续
6. 所有阶段完成 → 显示"已交付"
7. `.shuji/` 目录正确生成，日志完整，文档完整

### 第 2 周：真实 API 集成

- 实现 `api/client.rs`，Anthropic API 调用
- 替换 MockAgent 为 APIAgent
- System Prompt 定义（从现有架构提炼）
- 上下文持久化正确工作
- 验证全流程走通

### 第 3 周：UI 完善

- 文档 Markdown 渲染优化
- 流程可视化增强
- 设置页面（API Key、模型选择）
- 日志查看器

### 第 4 周：打磨 + 收尾

- 错误处理、超时重试
- 边缘情况（暂停/恢复、终止、3次驳回升级）
- 自测完整项目
- 打包配置
