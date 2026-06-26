# 枢机仓库导读

> 新人 / 维护者从这里开始。只读代码、跑测试、做仓库治理时，按本文索引即可，不必在目录里乱翻。

---

## 这个仓库是什么

| 路径 | 内容 |
|------|------|
| [`shuji-app/`](shuji-app/) | **唯一主应用**：Tauri v2 桌面端 + React 前端 + Rust 后端 |
| 仓库根目录 | 项目元信息、贡献指南、截图资源、脚本与演示资产 |

**不要**在根目录新增第二个应用目录。和主应用无关的脚本放 [`scripts/`](scripts/)，演示 PPT / 图片放 [`assets/`](assets/)。

---

## 文档地图（按阅读顺序）

### 第一步：跑起来

1. [README.md](README.md) — 项目介绍与快速开始
2. [CONTRIBUTING.md](CONTRIBUTING.md) — 环境、配置优先级、测试命令、**目录约定**

### 第二步：理解架构（现状，非未来设计）

| 文档 | 读者 | 说明 |
|------|------|------|
| [CLAUDE.md](CLAUDE.md) | 维护者 / AI | 深度索引（文件级导航） |

读架构时**以 `CLAUDE.md` 为准**。

---

## 代码从哪里读（后端主链路）

```
shuji-app/src-tauri/src/
├── lib.rs                    # 模块树 + Tauri 命令注册
├── commands/workflow/send.rs # 用户发消息入口（常规 / pipeline 恢复双路径）
├── commands/workflow/bootstrap.rs # ensure_actor_system、事件转发
├── actor/spawn.rs            # 各部门 Actor 循环
├── api/control.rs            # LLM 工具驱动循环（最核心）
├── api/session/              # 消息历史与 API 调用
├── tool/dispatch.rs          # 工具总调度
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
| 应用内文档 | `shuji-app/docs/`（本地） |
| 仓库级脚本 | `scripts/` |
| 答辩 / 演示网页 | `assets/presentations/` |

---

## 常见困惑

| 问题 | 答案 |
|------|------|
| 为什么有 CLAUDE.md？ | 深度维护索引，面向维护者与 AI 辅助开发 |
| 改配置不生效？ | `config.local.toml` > `config.toml`；API 用 `api_config.json` > `.env` |
| 测试要 API Key 吗？ | 绝大多数不需要；`expand_requirements_test` 默认跳过 |

---

*维护：做仓库治理时同步更新本文的链接与目录表。*
