# 枢机仓库导读

> 新人 / 维护者从这里开始。按下面顺序阅读，避免被旧 `route_to` 叙事带偏。

---

## 这个仓库是什么

| 路径 | 内容 |
|------|------|
| [`shuji-app/`](shuji-app/) | **唯一主应用**：Tauri v2 桌面端 + React 前端 + Rust 后端 |
| 仓库根目录 | 项目元信息、贡献指南、截图资源、脚本与演示资产 |

**不要**在根目录新增第二个应用目录。和主应用无关的脚本放 [`scripts/`](scripts/)，演示 PPT / 图片放 [`assets/`](assets/) 或 [`docs/images/`](docs/images/)。

---

## 阅读路径（推荐顺序）

### ① 5 分钟：知道枢机是什么

1. [README.md](README.md) — 特性、界面一览、Pipeline 流程图  
2. 本地跑起来：`cd shuji-app && npm install && npm run tauri dev`  
3. 点击 **体验枢机**，走一遍 demo（含入门引导）

### ② 30 分钟：理解现行架构

1. **[shuji-app/docs/ARCHITECTURE.md](shuji-app/docs/ARCHITECTURE.md)** — **主流程、Actor/Pipeline、朱批、观测**（对外 + agent 必读）  
2. [CONTRIBUTING.md](CONTRIBUTING.md) — 配置优先级、目录约定、测试命令  

### ③ 深入维护 / AI 辅助开发

1. [CLAUDE.md](CLAUDE.md) — 文件级索引、各模块路径、边缘 case 清单  
2. **[shuji-app/docs/AGENT_TASKS.md](shuji-app/docs/AGENT_TASKS.md)** — 后续 agent 任务边界、清单完成状态、不建议做的事  

### ④ 改代码时

| 你要… | 先读 |
|-------|------|
| 改用户发消息 / Pipeline 恢复 | `commands/workflow/send.rs` + ARCHITECTURE §主流程 |
| 改部门行为 | `agent/{role}/` + `tool/dispatch.rs` |
| 改 UI 主流程 | `pages/ProjectDashboard.tsx`、`components/CommandBar.tsx` |
| 加测试 | [CONTRIBUTING.md#测试](CONTRIBUTING.md#测试) |

---

## 代码从哪里读（后端主链路）

```
shuji-app/src-tauri/src/
├── lib.rs                    # 模块树 + Tauri 命令注册
├── commands/workflow/send.rs # 用户发消息入口（常规 / pipeline 恢复双路径）
├── commands/workflow/bootstrap.rs # ensure_actor_system、事件转发
├── pipeline/engine.rs        # PipelineEngine 调度
├── actor/spawn.rs            # 各部门 Actor 循环
├── api/control/mod.rs        # LLM 工具驱动循环（最核心）
├── api/session/              # 消息历史与 API 调用
├── tool/dispatch.rs          # 工具总调度 + contract gate
└── agent/*/mod.rs            # 各部门 Agent 实现
```

前端边界：[`shuji-app/src/api.ts`](shuji-app/src/api.ts) 通过 `invoke()` 调 Rust，事件名 kebab-case（如 `chat-message`）。

---

## 目录约定（摘要）

完整版见 [CONTRIBUTING.md#仓库目录约定](CONTRIBUTING.md#仓库目录约定)。

| 你要加… | 放哪里 |
|---------|--------|
| Tauri 命令 | `shuji-app/src-tauri/src/commands/` |
| 新 Agent | `shuji-app/src-tauri/src/agent/{角色}/`（`mod.rs` + `prompt.md`） |
| 新工具 | `shuji-app/src-tauri/src/tool/`，在 `registry.rs` 注册 |
| 集成测试 | `shuji-app/src-tauri/tests/` |
| React 组件 | `shuji-app/src/components/`（通用 UI → `components/ui/`） |
| 应用内文档 | `shuji-app/docs/` |
| 界面截图 | `docs/images/` |
| 仓库级脚本 | `scripts/` |

---

## 常见困惑

| 问题 | 答案 |
|------|------|
| 主流程是 route_to 还是 Pipeline？ | **Pipeline-first**。内阁 `submit_pipeline_plan` → `PipelineEngine`。`route_to` 仅步骤内兼容。 |
| README 和 CLAUDE 看哪个？ | 对外 / 架构叙事 → **ARCHITECTURE.md**；改具体文件 → **CLAUDE.md** |
| 为什么有 CLAUDE.md？ | Cursor/Claude 维护索引，含测试命令与文件路径 |
| 改配置不生效？ | `config.local.toml` > `config.toml`；API 用 `api_config.json` > `.env` |
| 测试要 API Key 吗？ | 绝大多数不需要；`expand_requirements_test` 默认跳过 |

---

*维护：做仓库治理时同步更新本文链接与阅读顺序。*
