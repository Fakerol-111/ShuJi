# Changelog

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
