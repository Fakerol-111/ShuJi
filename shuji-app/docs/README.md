# 枢机应用文档（`shuji-app/docs/`）

与主应用相关的**可提交**文档集中在此。仓库级导读见根目录 [`ONBOARDING.md`](../../ONBOARDING.md)。

## 架构与研读

| 文档 | 说明 |
|------|------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | **现行实现**架构（Actor + mpsc） |
| [../../docs/refactoring-three-phases.md](../../docs/refactoring-three-phases.md) | **三阶段重构方案**（拆模块 → 收敛流程 → Workspace） |
| [../../docs/adr/README.md](../../docs/adr/README.md) | 架构决策记录（ADR） |
| [BACKEND_LEARNING_PLAN.md](BACKEND_LEARNING_PLAN.md) | 后端 Rust 分阶段研读计划 |
| [design/future-mailbox.md](design/future-mailbox.md) | ⚠️ **未来设计**，非现行代码 |

## 开发与测试

| 文档 | 说明 |
|------|------|
| [TEST_FLOW.md](TEST_FLOW.md) | 手动 E2E 验收流程 |
| [TOOL_OPTIMIZATION.md](TOOL_OPTIMIZATION.md) | 工具层优化 backlog |

## 前端设计

| 文档 | 说明 |
|------|------|
| [古风政务主题改造方案.md](古风政务主题改造方案.md) | 全局主题 token 与 UI Phase 1–2 |

本地 UI 草稿可放在仓库根 [`docs/`](../../docs/)（默认不提交 Git）；若需团队共享，请移入本目录。
