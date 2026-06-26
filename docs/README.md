# 仓库级 `docs/` 目录

本目录与 [`shuji-app/docs/`](../shuji-app/docs/) 分工如下：

| 目录 / 文件 | 是否提交 Git | 用途 |
|-------------|--------------|------|
| **[refactoring-three-phases.md](refactoring-three-phases.md)** | ✅ | **三阶段重构总方案**（渐进式，非大重写） |
| **[adr/](adr/)** | ✅ | 架构决策记录（ADR） |
| **`docs/images/`** | ✅ | README / 官网用产品截图（见 [images/README.md](images/README.md)） |
| **`docs/README.md`** | ✅ | 本说明 |
| **`docs/` 下其他文件** | ❌ 忽略 | 本地设计草稿、个人笔记（见根目录 `.gitignore`） |

## 重构与架构

| 文档 | 说明 |
|------|------|
| [refactoring-three-phases.md](refactoring-three-phases.md) | 阶段一拆上帝对象 → 阶段二收敛流程 → 阶段三 Cargo Workspace |
| [adr/README.md](adr/README.md) | ADR 索引与模板 |

现行运行时代码架构见 [`shuji-app/docs/ARCHITECTURE.md`](../shuji-app/docs/ARCHITECTURE.md)。

## 团队文档放哪里

- **应用架构、测试流程、后端学习计划** → [`shuji-app/docs/`](../shuji-app/docs/)
- **新人入口、贡献规范** → 根目录 [`ONBOARDING.md`](../ONBOARDING.md)、[`CONTRIBUTING.md`](../CONTRIBUTING.md)

## 本地草稿示例

可在本目录自由创建（不会进 Git，除非加入 `.gitignore` 白名单）：

```
docs/
├── 大模型驾驶舱-典籍仪制设计.md   # 本地 UI 设计（若需共享请移到 shuji-app/docs/）
└── notes/                        # 个人研读笔记
```
