# 架构决策记录（ADR）

本目录存放枢机重构与重大设计变更的决策记录。每份 ADR 对应三阶段重构方案中的一处关键抉择。

## 索引

| ADR | 标题 | 阶段 | 状态 |
|-----|------|------|------|
| [0002](0002-control-module-split.md) | `api/control` 模块拆分 | 阶段一 | 提议 |
| [0001](0001-orchestration-single-entry.md) | 流程编排单一入口（WorkflowFacade） | 阶段二 | 待创建 |
| [0003](0003-workspace-crate-boundaries.md) | Cargo Workspace crate 边界 | 阶段三 | 待创建 |

## 何时写 ADR

- 拆分或合并顶层模块
- 引入新的持久化状态文件或改变写入方
- 调整 `workflow` / `pipeline` / `actor` 协作方式
- 新建 workspace crate

## 模板

见 [../refactoring-three-phases.md §6.2](../refactoring-three-phases.md#6-adr-与-pr-流程)。
