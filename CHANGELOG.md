# Changelog

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
