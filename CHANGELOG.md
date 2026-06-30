# Changelog

所有重要变更记录于此文件。格式遵循 [Keep a Changelog](https://keepachangelog.com/)，版本号对应 git tag。

## [Unreleased]

v0.8.0 之后已合并到 main 但未发布 tag 的变更。

### Added
- LLM 推理/思考链支持（`feature/llm-reasoning-policy`）— per-vendor reasoning 注入、UI 展示、文档同步
- 角色化学习记忆系统（multi-role soul + global learning）— `feat/soul-learning-system`
- 文档线审计 + 语义检查点体系 — `feat/audit-document-line`
- 外部 IDE 集成 — 架阁一键打开文件/行号/项目
- Linux 平台适配 — 跨平台路径解析、命令安全、Python 检测、编辑器路径
- 文档预览增强 + 7 套代码主题 + TabBar 交互优化
- 流式讨论 + Pipeline 进度推送 + DeliveryReceipt 收据
- 实时运行状态推送 + 部门活动摘要增强
- 仓库优化 P0 — playbook 系统 + pipeline supervisor + 前端审批增强
- English i18n support — react-i18next, bilingual LLM prompts, LangSwitcher
- pipeline 引擎增强 + validate/metrics/playbook/precepts 新模块 + UI 重构
- 独立设置页面 + 离线演示模式 + ESAA 契约 & 意图拦截
- 九部门卡片 + inspector + 实时步骤流式

### Changed
- Priority Six 优化 — 架构重构、命令安全加固、审批门禁硬化（`codex/priority-six-optimizations`）
- Dashboard 布局重构：AgentStreamPanel + ArtifactPanel 左右分栏

### Fixed
- 前端窗口布局拉伸崩溃 — flex 约束 / 动态 clamp / pointer 拖拽 / 滚动隔离
- 审批模式重构 + pipeline artifacts + options render 修复
- resolve_scoped_path Windows CI 路径比较与符号链接逃逸
- 部门状态栏追踪不显示、Token 统计不显示
- MockActorHarness mock 输出补充文档 ID 用于 pipeline artifact 提取
- CommandBar 测试补充 listCheckpoints mock

## [0.8.0] - 2026-06-12

### Added
- ESAA 架构改造 — hash 链审计 + 意图拦截层 + 边界契约
- 多平台构建目标启用（MSI / DMG / DEB / AppImage）
- README 重写 + CONTRIBUTING + docs 目录

## [0.7.0] - 2026-06-12

### Added
- Pipeline 引擎实现（Phase 1 引擎 / Phase 2 cleanup + prompt rewrite / Phase 3 前端 PlanPanel）
- 路由管道经尚书省分配六部 + 双文移图
- 定价系统重构 — pricing 模块 + 双货币 + Web 刷新 + 批量配置入口
- 费用透明 + 文档更新 + 进度增强
- 降低上手门槛 + 前端重构

### Changed
- 拆分 session/tool/actor 单文件为模块化结构
- Prettier 格式化前端文件

### Fixed
- send_message 管道恢复路径（T6）
- 静默 catch + skill fallback + README 下载区（P0-2）
- cargo fmt 格式化问题
- CI 跳过预存的 workflow_demo 测试

## [0.6.0] - 2026-06-08

### Added
- 三提示层架构：base_prompt / soul_prompt / context_messages，消除消息顺序漂移
- Skill 消息作为普通 system 消息存储，最大化 LLM 前缀缓存命中率
- 可选输出块：中书令/门下侍中 skill 输出模板（设计结论/待决问题/引用/路由）
- 可选输出块在 [对话摘要] 压缩后保留，避免结构化数据丢失
- Agent 模块拆分（session/tool/actor 子模块化）

### Changed
- 3 层上下文持久化替代旧 4 层设计
- Context compaction 改用单层压缩，skill 消息在压缩前后剥离重新追加
- 内阁 context 压缩 prompt 与部门压缩 prompt 分开

### Fixed
- Soul 消息漂移问题：PersistedContext 单独存储 soul_prompt

## [0.5.0] - 2026-06-05

### Added
- 尚书令+吏部+兵部+工部+刑部+礼部 全部接入 runner.rs 共享执行框架
- 工部批量计划循环（PlanState + batch/current/complete）
- agent/runner.rs 内置 compact handler + checkpoint handler
- 兵部测试+接口契约工作流
- 刑部测试验证工作流
- 礼部规范检查+审计工作流
- Workflow Profile 系统（profiles/ YAML 定义）

### Changed
- 内阁 routing.rs 升级：显式 skill > 关键词匹配 > 复杂度分析
- 内阁 soul 系统：update_soul 工具，8KB 上限，自动 LLM 压缩
- Agent trait 统一 execute() 返回 anyhow::Result<AgentOutput>

## [0.4.0] - 2026-06-03

### Added
- 代码质量优化七项（本版本）：
  - 部门颜色/角色名统一 — DEPT_META 单一数据源，消除 8 个组件中 12 处重复定义
  - Agent 执行框架提取 — agent/runner.rs 共享模块，消除 8 个 Agent 中 ~750 行重复
  - 核心驱动循环测试 — 新增 14 个 session/control 测试（sanitize_messages、PersistedContext、RunResult）
  - Watchdog 闭环自愈 — 同工具重复/只读不写时向 tool result 注入 [干预] 提示
  - Audit 反向索引 — RefIndex + check_immutability 实现 O(1) 引用查找
  - AuditPanel 组件拆分 — 提取 audit/shared.tsx，主文件精简
  - 配置层扁平化 — config.local.toml merge 加载支持
- 审计三件套（OPT-015/016/017）：
  - 新建 `audit/mod.rs` 模块：事件 JSONL 持久化、文档血缘追溯、时间线聚合、交付报告
  - `create_document` / `set_document_status` / checkpoint / milestone 处自动记录审计日志到 `.shuji/audit.jsonl`
  - 新增后端命令：`get_document_lineage`、`get_audit_timeline`、`generate_delivery_report`
  - DocPreview 增加「血缘」Tab，树形展示文档引用链
  - ActivityBar 增加「朝报」入口，Sidebar 展示审计时间线
  - ProjectOverview 增加「生成交付报告」按钮，Markdown 格式汇总事件统计与文档产出
  - 复用 `documents.rs` 的 `parse_doc` / `parse_refs`，无新增依赖
- 架构文档与实现一致（OPT-011）：
  - mailbox_design.md 添加 FUTURE DESIGN 状态声明
  - 新建 `ARCHITECTURE.md` 描述当前实际 Actor + mpsc Push 架构
  - CLAUDE.md 增加架构文档指引
- 部门活跃状态精确化：
  - 后端暴露 `get_active_roles` 命令
  - 前端轮询替代 5 秒超时推断
- 部门最终存档：agent 执行完成后强制 git commit + checkpoint

## [0.3.0] - 2026-06-01

### Added
- Demo 上手漏斗强化（OPT-001）：
  - 工作区选择页「体验枢机」按钮进入 Dashboard 后自动发送指令
  - 引导浮层 DemoTour（四步引导：部门栏 → 文档树 → 工部修 bug → 测试验证）
  - 完成后展示小结卡片（耗时、Token 消耗、下一步建议）
- 内阁自动 workflow 选择（OPT-002）：
  - 新增 `routing.rs` 轻量规则引擎，纯文本匹配自动建议 workflow skill
  - 按优先级：显式 skill 名 > 关键词匹配（bug/refactor/optimize/audit）> 复杂度分析
  - 高置信度直接注入建议，低置信度提示内阁用 `<options>` 让皇帝选择
  - 16 个单元测试覆盖所有 routing 分支
  - HelpDrawer 添加「自动选流程」提示
- 流程地图置顶与待办可见（OPT-003）：
  - 重写 WorkflowTimeline 为 WorkflowStatus 组件，使用项目设计系统色板
  - 在 Dashboard 主区域顶部固定展示整体进度条与阶段状态
  - 阻塞原因 badge：待朱批文档 ID（⚑）、活跃部门（● 脉冲动画）、工部计划批次
  - 新增 `get_pending_approvals` 后端命令，前端 3 秒轮询
  - 点击待朱批 badge 可直接跳转到对应文档
  - 空阶段时显示「尚未启动流程」
- 模型分级预设三档（OPT-005）：
  - 新增 `preset` 字段到 `api_config.json`：`balanced`（默认）/ `economy` / `quality` / `custom`
  - 后端单一真相源映射表：economy → 审查/检查角色用轻量模型；quality → 设计/编码角色用强模型
  - 自动派生模型名（支持 DeepSeek / Anthropic / OpenAI 模型家族自动匹配 cheap/strong）
  - SetupPage 增加预设选择器；SettingsMenu 增加预设切换（手动改覆盖 → 自动标记 custom）
  - 旧版 `api_config.json` 无 `preset` 字段时默认 `balanced`（`#[serde(default)]`）
- `/project` 路由将工作区选择与 Dashboard 分离
- 任务级短路规则（OPT-007）：
  - workflow_demo/bugfix 模式下禁止 route_to 中书令和门下侍中
  - 在 内阁 tool exec 闭包中注入技能感知拦截，返回结构化错误
  - 支持 `--override-skill-gate` 用户强制绕行
  - 2 个集成测试覆盖拦截与放行两种路径
- 朱批文档 diff 体验（OPT-008）：
  - 新增 `get_document_diff` 后端命令，使用 git HEAD 版本比较生成 unified diff
  - DocPreview 增加「全文/差异」Tab 切换，差异视图着色显示 +/- 行
  - 驳回时提供快捷理由模板下拉（缺少 API 定义/测试策略/范围过大需拆分）
  - 无上一版时自动隐藏差异 Tab

## [0.2.0] - 2026-05-28

### Added
- 内阁自进化系统：reflect 复盘模式、soul 结构化章节、create_skill 工具
- 工部 TDD 循环：execute_command 支持开发中自我验证
- 刑部重新定位：集成测试 + 全量测试 + 质量报告
- 运行时技能动态加载（.shuji/skills/）

### Changed
- 工部-刑部分工：工部负责单元测试，刑部负责集成测试+终验
- 所有模型从 deepseek-chat 迁移到 deepseek-v4-flash
- DeepSeek reasoning_content 正确处理（提取+日志+history 前剥离）
- 系统 prompt 精简 40-60%，单次 API 调用 token 消耗减半
- max_tokens 按 agent 类型合理设置（不再全部 0）
- context compaction 阈值从 160K 降到 80K

### Fixed
- route_to 消息不推送前端（emit filter bug）
- 恢复暂停机制（paused_for_decision 标志）
- 全项目 8 处 .catch(()=>{}) 改为 console.error
- Windows 路径安全检测从硬编码盘符改为通用 Prefix 检测

## [0.1.0] - 2026-05-01

### Added
- 初始原型：三省六部制 AI 开发系统
- 12 个部门 agent + expand_requirements sub-agent
- Skill 系统（内阁 7→11 个、中书令 3 个、门下侍中 2 个）
- Tauri v2 桌面应用 + React 前端
- 文档中心通信（YAML frontmatter + ID counter）
- 4 层上下文持久化 + 双层压缩
- 参与者模式（全自动/关键确认/逐步审核 3 级）
- Token 用量统计
- 双格式 API（OpenAI / Anthropic）
