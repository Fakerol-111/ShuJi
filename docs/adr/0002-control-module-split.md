# ADR-0002: control 模块拆分

## 状态
已接受

## 背景
`api/control.rs` 是一个典型的上帝对象，约 920 行，承担了过多职责：
- 类型定义（RunResult、RouteTo、RouteMsgType、回调类型别名）
- 迭代预算计算（is_read_tool、max_iterations_for_tools）
- 主运行循环（run()，含 tool dispatch、watchdog、routing）
- 生命周期管理（interrupt、restart_with、take_snapshot）
- Step 事件发射（setup_agent_step_emitter）

单个文件修改牵动全局，违背单一职责原则。

## 决策
将 `api/control.rs` 拆分为目录模块 `api/control/`，按职责划分为以下子模块：

```
api/control/
├── mod.rs              # AgentController 门面 + pub use 统一导出
├── types.rs            # RunResult, RouteTo, RouteMsgType, 回调类型别名
├── iterations.rs       # max_iterations_for_tools, is_read_tool
├── run_loop.rs         # run() 主循环（编排层，≤150 行）
├── tool_batch.rs       # 单轮 tool_calls 执行（并行读、串行写）
├── watchdog.rs         # 同工具重复、只读不写、delete-create 循环检测与干预
├── route_detect.rs     # 从 tool 输出 JSON 解析 route_to
├── lifecycle.rs        # interrupt, suspend, take_snapshot, checkpoint 触发
└── step_emit.rs        # setup_agent_step_emitter, DeptStep 发射
```

拆分顺序依据依赖关系，按 PR 分批进行。

## 后果

正面：
- 每个子模块职责单一，可独立理解和测试
- 核心文件单文件 ≤400 行（run_loop.rs 编排层除外）
- 外部 API 通过 `mod.rs` 的 `pub use` 完全保持兼容，调用方无需任何修改

负面：
- 引入目录模块后开发者需知晓新文件位置
- 微小理解成本：原来一个文件要浏览的文件变多

迁移成本：
- PR-1.1（本 PR）：纯移动 types.rs + iterations.rs，零行为变化
- 后续 PR：逐步抽出 watchdog、route_detect、tool_batch 等，每 PR 验证一次 cargo test

## 不做什么
- 不修改 AgentController 的行为或公开 API
- 不引入 trait 或设计模式变更
- 不改动 run() 的逻辑结构
