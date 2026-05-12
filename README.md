# 枢机 (ShuJi)

**枢机** — 基于中国古代三省六部制的 AI 驱动自动化软件开发系统。

每个部门是一个 LLM agent，通过角色分工和文档化通信，模拟从需求分析到编码测试的完整软件工程流程。

## 概念

传统 AI 编程助手只解决"怎么写代码"的问题。枢机解决的是 **"怎么组织软件开发过程"** 的问题：

- 用户只提需求（"我要做一个记账软件"）
- 内阁（Cabinet）根据需求复杂度选择合适的协作流程
- 中书令负责系统设计
- 门下省负责设计审查
- 尚书令协调执行链（吏部详细设计 → 兵部测试/契约 → 工部编码 → 刑部验证 → 礼部规范检查）
- 制司拥有独立调查权限

所有部门通过 `.shuji/` 目录下的文档通信，而不是靠 LLM 对话传递上下文。

## 架构

```
皇帝 → send_message → 内阁(actor) → route_to → 各部门(actor)
                                                     ├─ 中书令 → 系统设计
                                                     ├─ 门下侍中 → 审查（整体设计 + 阶段设计）
                                                     ├─ 尚书令 → 执行调度
                                                     │   ├─ 吏部尚书 → 详细设计
                                                     │   ├─ 兵部尚书 → 测试+契约
                                                     │   ├─ 工部尚书 → 编码
                                                     │   ├─ 刑部尚书 → 测试验证
                                                     │   └─ 礼部尚书 → 规范检查
                                                     └─ 制司 → 独立调查（预留，后期安全阶段实现）
```

### 核心技术

- **Actor 模型** — 每个部门是一个 tokio actor，通过 mpsc 通道异步通信
- **Skill 系统** — 内阁和中书令通过 `<skill>` 标签动态切换工作模式
- **文档中心通信** — 部门间通过 `.shuji/` 下的文档传递信息，`route_to` 只传文档 ID
- **AgentController** — 统一的 tool 循环驱动，处理 cancel/interrupt/restart 生命周期
- **Session** — LLM 会话层，支持 OpenAI 和 Anthropic 两种 API 格式

### 项目结构

```
shuji-app/
├── src/                          # 前端 (React + Tauri v2 + Tailwind)
│   ├── pages/
│   │   ├── WorkspaceSelect.tsx   # 项目选择/创建
│   │   ├── ProjectDashboard.tsx  # 主聊天界面 + 仪表盘
│   │   └── LogsPage.tsx          # 部门日志查看
│   └── components/
│       ├── ChatBubble.tsx        # 消息气泡（支持 <options> 按钮）
│       ├── ChatInput.tsx         # 输入框
│       ├── DeptStatusPanel.tsx   # 实时部门状态面板
│       └── WorkflowTimeline.tsx  # 项目进度可视化
└── src-tauri/src/                # 后端 (Rust)
    ├── agent/                    # LLM Agent 实现
    │   ├── neige/                # 内阁 — 流程选择 + 路由分派
    │   │   ├── prompt.md         # 基础 prompt
    │   │   └── skills/           # 工作模式定义
    │   ├── zhongshuling/         # 中书令 — 设计中心
    │   │   ├── prompt.md
    │   │   └── skills/
    │   ├── menxiashizhong/       # 门下侍中 — 审查（预留 skill 化，合并给事中）
    │   ├── shangshuling/         # 尚书令 — 执行调度
    │   ├── libushangshu/         # 吏部尚书 — 详细设计
    │   ├── bingbushangshu/       # 兵部尚书 — 测试 + 契约
    │   ├── gongbushangshu/       # 工部尚书 — 编码
    │   ├── xingbushangshu/       # 刑部尚书 — 测试验证
    │   ├── liburshangshu/        # 礼部尚书 — 规范检查
    │   ├── zhisi/                # 制司 — 独立调查（预留，后期安全阶段实现）
    │   └── hubu/                 # 户部 — 记录归档（已废弃）
    ├── api/                      # LLM API 层
    │   ├── client.rs             # 双格式 API 客户端
    │   ├── session.rs            # 会话管理 + 自动重试
    │   └── control.rs            # tool 循环驱动 + watchdog
    ├── tool/                     # 工具系统
    │   ├── mod.rs                # 文件读写/编辑/执行命令等
    │   └── documents.rs          # create_document / update_document
    ├── actor/                    # Actor 系统
    ├── models/                   # 数据模型 (Role/Project/Chat/Message)
    ├── storage/                  # .shuji/ 文件系统
    └── logging/                  # 活动日志
```

## 快速开始

### 环境要求

- Node.js >= 18
- Rust >= 1.70
- Tauri v2 依赖（根据操作系统安装）

### 配置

在 `shuji-app/` 下创建 `.env` 文件：

```
DEFAULT_API_KEY=sk-your-key
DEFAULT_API_URL=https://api.deepseek.com/chat/completions
DEFAULT_MODEL=deepseek-chat
```

支持为每个角色单独配置：

```
NEIGE_API_KEY=sk-xxx
ZHONGSHULING_API_KEY=sk-xxx
MENXIASHIZHONG_API_KEY=sk-xxx
SHANGSHULING_API_KEY=sk-xxx
LIBUSHANGSHU_API_KEY=sk-xxx
BINGBUSHANGSHU_API_KEY=sk-xxx
GONGBUSHANGSHU_API_KEY=sk-xxx
XINGBUSHANGSHU_API_KEY=sk-xxx
LIBURSHANGSHU_API_KEY=sk-xxx
```

URL 包含 `anthropic.com` → Anthropic Messages API，否则 → OpenAI Chat Completions。

### 运行

```bash
cd shuji-app
npm install
npm run tauri dev
```

### 构建

```bash
npm run tauri build
```

## 项目状态

PoC / 原型阶段。核心 actor 系统和协作流程已通，前端具备基本交互能力。

### 已实现

- [x] 9 个部门 agent + 门下侍中（审查 skill）+ 制司（预留，后期安全阶段实现）
- [x] 内阁 skill 系统（clarify / workflow 选择 / discuss / summary）
- [x] 中书令 skill 系统（overall design / phase plan / phase design）
- [x] Actor 模型异步通信
- [x] 文档中心通信（YAML frontmatter + 自动 ID）
- [x] 活动日志（单文件，按时间有序）
- [x] Token 用量统计
- [x] 双格式 API（OpenAI / Anthropic）
- [x] 取消/中断机制

### 待办

- [ ] 上下文压缩（skill 累积后的 token 管理）
- [ ] 前端仪表盘图完善
- [ ] IDE 布局界面（文件树 + 代码 + 对话）

## 设计理念

- **文档是契约，不是副产品** — 部门间通过文档通信，而不是靠 LLM 对话传递上下文
- **流程适配任务** — 内阁根据任务复杂度选择最轻量的流程，而不是固定走全套
- **职责隔离** — 每个部门只做自己职责内的事（设计的不写代码，编码的不做审查）
