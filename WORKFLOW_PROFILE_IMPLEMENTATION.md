# 枢机 Workflow 声明化改造 — Agent 任务包

> **用途**：交给 Agent 实施「Workflow Profile 声明化 + GateEngine/ChainEngine + Intent×Governance + auto fallback」。  
> **原则**：Phase A 最小可用；不一次性重构全部；向后兼容；最小 diff。

---

## 目录

| 章节 | 内容 |
|------|------|
| [一、主 Prompt](#一主-prompt整段复制给-agent) | 整段复制给 Agent |
| [二、分阶段修改清单](#二分阶段修改清单) | Phase A/B/C 勾选表 |
| [三、Profile Schema](#三profile-schema-参考) | Rust / YAML 结构 |
| [四、Intent × Governance 矩阵](#四intent--governance-矩阵) | 产品参考 |
| [五、不要做的事](#五给-agent-的不要做的事) | 约束 |
| [六、简短版 Prompt](#六简短版-prompt上下文有限时用) | 压缩版 |
| [七、验收与测试命令](#七验收与测试命令) | Done 定义 |

---

## 一、主 Prompt（整段复制给 Agent）

```markdown
# 任务：枢机 Workflow 声明化架构（Phase A 优先）

## 背景

当前枢机的工作流选择依赖内阁 LLM + routing.rs 软 hint + workflow_preset 文本注入，
对「读现有 repo 再优化」等 brownfield 场景容易误走 greenfield（workflow_standard + 全套吏兵工刑礼链）。

目标：实现「Workflow Profile 声明化 + 通用 GateEngine/ChainEngine + Intent×Governance 两轴 + auto fallback」，
但 **Phase A 只做最小可用切片**，不一次性重构全部。

## 架构目标（全貌，Phase A 只做子集）

```
用户选择 Intent × Governance
        ↓
WorkflowResolver → ActiveProfile（profile + overlay）
        ↓
┌─────────────────┬─────────────────┐
│   GateEngine    │   ChainEngine   │
│ tool/route 拦截 │ 尚书令执行链注入 │
└────────┬────────┴────────┬────────┘
         ↓                   ↓
      内阁 Actor         尚书令 Actor
         ↓
    StageTracker → workflow_state.json
```

- **Intent（任务意图）**：greenfield_standard / brownfield_optimize / bugfix / demo / auto ...
- **Governance（治理强度）**：full / standard / fast / audit（迁移自 workflow_preset）
- **Profile**：声明 stages、gates、execution_chain、cabinet_skill
- **auto**：routing.rs 推断；Low 置信度必须 `<options>`，禁止 silent fallback 到 standard

## 硬性约束

1. **最小 diff**：Phase A 只动必要模块；不顺带重构无关代码。
2. **不破坏现有测试**：`cargo test --tests` 全部通过；新增 profile/gate 相关单测。
3. **向后兼容**：无 `workflow_config.json` 时行为与 today 一致（intent=auto, governance=standard）。
4. **不改 Agent prompt 中英混用策略**（除非本任务明确要求）。
5. **不提交 .env、密钥**；不主动 git commit（除非用户要求）。
6. 现有 `neige/skills/workflow_*.md`、`zhongshuling/skills/*.md` **保留**，profile 引用它们而非重写。

## Phase A 交付范围（必须完成）

### A1. 配置与解析

新增 `.shuji/workflow_config.json`（项目级）：

```json
{
  "intent": "auto",
  "governance": "standard",
  "intent_override": null
}
```

- `intent` 枚举（Phase A）：`auto | greenfield_standard | brownfield_optimize | bugfix | demo`
- `governance` 枚举：`full | standard | fast | audit`（迁移自现有 workflow_preset.json，双写兼容）
- `intent_override`：单次任务临时覆盖（用户消息 `<skill>workflow_xxx</skill>` 或 UI「本次用 bugfix」）

新增 Rust 模块 `shuji-app/src-tauri/src/workflow/`：

| 文件 | 职责 |
|------|------|
| `mod.rs` | 模块导出 |
| `config.rs` | 读写 workflow_config.json |
| `profile.rs` | 内置 profile 定义（Phase A 用 Rust struct，Phase B 可迁 YAML） |
| `resolver.rs` | WorkflowResolver：合并 intent + governance + routing auto |
| `gate.rs` | GateEngine |
| `chain.rs` | ChainEngine |
| `state.rs` | workflow_state.json 读写 |

在 `lib.rs` 注册 `mod workflow`。

新增 Tauri commands（或扩展现有 settings）：

- `get_workflow_config` / `set_workflow_config`
- 前端 SettingsMenu 增加「任务意图 Intent」下拉（Phase A 可先做后端 + 最小 UI）

### A2. 内置 Profile（Phase A）

| profile id | cabinet_skill | execution_chain | 说明 |
|------------|---------------|-----------------|------|
| greenfield_standard | workflow_standard | greenfield_full | 现有默认路径 |
| brownfield_optimize | workflow_optimize | brownfield_patch | 存量优化 |
| bugfix | workflow_bugfix | brownfield_patch 或 bugfix 专用 | 已有硬 gate |
| demo | workflow_demo | brownfield_patch | 已有硬 gate |

每个 profile 至少包含：

```rust
pub struct WorkflowProfile {
    pub id: &'static str,
    pub cabinet_skill: &'static str,
    pub execution_chain_id: &'static str,
    pub gates: GateRules,
}

pub struct GateRules {
    pub forbid_tools: &'static [&'static str],
    pub forbid_route_to: &'static [&'static str],
}
```

Governance overlay（Phase A 简化）：

- `fast` → 追加 forbid expand_requirements + forbid 门下侍中
- `full` → 不追加 forbid（保持现有 full preset 语义）
- `audit` → 推荐 audit 相关 gate（Phase B 完善）

### A3. GateEngine

将 `neige/mod.rs` 中 demo/bugfix 的 route_to 硬编码 **迁移** 到 GateEngine，行为不变。

现有逻辑位置：`agent/neige/mod.rs` exec 闭包，`skill == workflow_demo || workflow_bugfix` 禁止 route_to 中书令/门下侍中。

```rust
pub struct ActiveProfile { /* merged profile + governance */ }

pub struct GateEngine;

impl GateEngine {
    pub fn check_tool(
        profile: &ActiveProfile,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> Result<(), GateViolation>;
}
```

- 内阁 `exec` 闭包：所有 tool 调用前先 `GateEngine::check_tool`
- 违规返回现有格式 `ToolOutput::error(..., "skill_short_circuit", ...)`
- 保留 `--override-skill-gate` 逃生阀

brownfield_optimize profile gates（新增）：

- `forbid_tools`: `expand_requirements`
- `forbid_route_to`: `门下侍中`

### A4. ChainEngine

Chain registry（Phase A 两个）：

| chain id | steps |
|----------|-------|
| greenfield_full | 吏部 → 兵部 → 工部 → 刑部 → 礼部 |
| brownfield_patch | 工部 → 刑部 |

在 `ShangshulingAgent::execute` session 创建后 inject：

```text
[Execution Chain: brownfield_patch]
步骤: 工部 → 刑部
（shangshuling base prompt 保留通用调度原则，具体步骤由注入覆盖）
```

Resolver 根据 ActiveProfile 选 chain_id；内阁 route 到尚书令时写入 `workflow_state.json`。

### A5. WorkflowResolver 与内阁集成

修改 `agent/neige/mod.rs`：

1. 读 `workflow_config.json`
2. `WorkflowResolver::resolve(config, &input.task_description)` → `ActiveProfile`
3. **硬模式**（intent != auto）：
   - 强制 `session.inject_skill(profile.cabinet_skill, load_skill(...))`
   - **跳过** `routing.rs` hint 注入
4. **auto 模式**：
   - 保留 `routing.rs`
   - High → 映射到 profile 并锁定
   - Medium → 映射 + inject 确认 hint
   - Low → inject「必须 `<options>` 让用户选 intent」；**禁止** silent fallback 到 greenfield_standard
5. `ActiveProfile` 传入 exec 闭包供 GateEngine 使用

Intent 映射（routing skill → profile id）：

| routing skill | profile id |
|---------------|------------|
| workflow_standard | greenfield_standard |
| workflow_optimize | brownfield_optimize |
| workflow_bugfix | bugfix |
| workflow_demo | demo |
| workflow_simple | greenfield_standard（Phase A 可暂映射） |
| workflow_complex | greenfield_standard（Phase B 独立 profile） |
| workflow_refactor | brownfield_refactor（Phase B） |
| workflow_audit | audit（Phase B） |

### A6. workflow_state.json（Phase A 最小版）

```json
{
  "profile_id": "brownfield_optimize",
  "governance": "standard",
  "execution_chain_id": "brownfield_patch",
  "current_stage": "execution",
  "artifacts": { "task": "task_003" }
}
```

Phase A 更新时机：

- `send_message` 开始时：写入 profile_id + chain_id
- 内阁 route_to 尚书令时：current_stage = execution

### A7. 测试

新增 `shuji-app/src-tauri/tests/workflow_profile_test.rs`：

- [ ] Resolver: intent=brownfield_optimize → 正确 profile + chain
- [ ] Resolver: intent=auto + 「优化性能」→ brownfield_optimize
- [ ] Resolver: intent=auto + 「实现登录功能」→ greenfield_standard（Low 时标记 pending_choice）
- [ ] GateEngine: brownfield 禁止 expand_requirements
- [ ] GateEngine: demo/bugfix 禁止 route 中书令（回归）
- [ ] GateEngine: --override-skill-gate 可绕过
- [ ] 无 workflow_config.json 时默认 auto+standard，行为不变

### A8. 文档

- 更新 `shuji-app/ARCHITECTURE.md` 增加「Workflow Profile 系统」一节（简短）
- 更新 `CLAUDE.md` Build 段加新测试命令
- **不要**新建其他 markdown

## Phase B（可选，Phase A 完成后）

- StageTracker 完整阶段机 + WorkflowTimeline UI 接 workflow_state
- Profile 外置 YAML：`assets/workflows/*.yaml` + `governance/*.yaml`
- 中书令 stage 强制 skill inject + forbid_skills gate（如禁止 overall_design）
- routing 增强：「现有代码」「读 repo」「存量」→ optimize
- clarify.md 澄清后进 optimize/refactor，非仅 standard
- expand_change_request sub-agent（brownfield 需求展开）

## Phase C（可选，长期）

- `survey_codebase` sub-agent → `.shuji/analysis/codebase_survey.md`
- 用户自定义 profile：`.shuji/workflows/custom.yaml`
- profile schema 校验 CLI
- 全部 intent：refactor / audit / complex / simple

## 验收标准（Phase A Done）

1. intent=brownfield_optimize 时，内阁 **无需** 输出 `<skill>` 即加载 workflow_optimize
2. brownfield_optimize 下 expand_requirements **工具层报错**
3. brownfield_optimize 路由尚书令后，尚书令 session 注入 brownfield_patch（工部→刑部）
4. intent=auto 且输入模糊时，内阁收到「必须 options」指令，不 silent 进 standard
5. workflow_demo_test 及全部 integration tests 通过
6. demo/bugfix gate 行为与改前一致

## 实现顺序

1. workflow/ 模块 + config 读写
2. profile + resolver 单测
3. gate.rs + 迁移 neige demo/bugfix
4. chain.rs + 尚书令 inject
5. neige/mod.rs 集成
6. Tauri commands + 最小 UI
7. 集成测试 + 文档

## 参考现有代码

| 内容 | 路径 |
|------|------|
| routing | `src-tauri/src/agent/neige/routing.rs` |
| preset 注入 | `src-tauri/src/agent/neige/mod.rs` → `inject_workflow_preset` |
| skill gate | `src-tauri/src/agent/neige/mod.rs` ~L468-490 |
| 尚书令链 | `src-tauri/src/agent/shangshuling/prompt.md` |
| preset UI | `src/components/SettingsMenu.tsx` |
| preset API | `src-tauri/src/commands/settings.rs` |
| optimize skill | `src-tauri/src/agent/neige/skills/workflow_optimize.md` |
| code_analysis | `src-tauri/src/agent/zhongshuling/skills/code_analysis.md` |

开始实施 Phase A。每完成一步运行相关测试。遇到问题在总结中说明，不要静默跳过验收项。
```

---

## 二、分阶段修改清单

### Phase A — 必做（勾选验收）

#### 模块与文件

| # | 动作 | 路径 | 验收 |
|---|------|------|------|
| A1 | 新建 workflow 模块 | `shuji-app/src-tauri/src/workflow/`（6–7 文件） | `mod workflow` 在 lib.rs 注册 |
| A2 | WorkflowConfig | `workflow/config.rs` | serde 读写 `.shuji/workflow_config.json` |
| A3 | WorkflowProfile | `workflow/profile.rs` | ≥4 个内置 profile |
| A4 | WorkflowResolver | `workflow/resolver.rs` | 单测：auto / 硬选 / 映射 |
| A5 | GateEngine | `workflow/gate.rs` | 单测 + 迁移 demo/bugfix |
| A6 | ChainEngine | `workflow/chain.rs` | greenfield_full + brownfield_patch |
| A7 | Tauri commands | `commands/settings.rs` 或 `commands/workflow.rs` | get/set_workflow_config |
| A8 | 内阁集成 | `agent/neige/mod.rs` | resolver + gate + skill 强制注入 |
| A9 | 尚书令集成 | `agent/shangshuling/mod.rs` | chain inject |
| A10 | preset 兼容 | `inject_workflow_preset` | governance 读 config；旧 preset 仍可读 |
| A11 | 最小 UI | `SettingsMenu.tsx` + `api.ts` | Intent 下拉 5 项 |
| A12 | workflow_state | `workflow/state.rs` | route 尚书令时写入 |
| A13 | 集成测试 | `tests/workflow_profile_test.rs` | ≥6 个测试 |
| A14 | 文档 | `ARCHITECTURE.md`, `CLAUDE.md` | 简短更新 |

#### 行为变更

| # | 要改 | 不要改 |
|---|------|--------|
| B1 | intent≠auto → 强制 inject cabinet_skill | Actor 邮箱机制 |
| B2 | auto+Low → 禁止 silent standard | routing 整体删除 |
| B3 | GateEngine 统一拦截 | documents 朱批逻辑 |
| B4 | 尚书令 inject chain | route_to 文档 ID 语义 |
| B5 | brownfield 禁 expand | expand_requirements sub-agent 本体 |

---

### Phase B — 可选增强

| # | 任务 | 关键点 |
|---|------|--------|
| B1 | Profile 外置 YAML | `assets/workflows/*.yaml` + loader |
| B2 | Governance overlay | `governance/*.yaml` merge → ActiveProfile |
| B3 | StageTracker 完整 | stages 硬推进（anls 创建 → plan stage） |
| B4 | 中书令 Gate | 强制 skill inject；forbid overall_design |
| B5 | WorkflowTimeline | 读 workflow_state.json |
| B6 | routing 增强 | 「现有代码」「读 repo」「存量」→ optimize |
| B7 | clarify 路由 | 澄清后进 optimize/refactor |

---

### Phase C — 可选长期

| # | 任务 |
|---|------|
| C1 | `survey_codebase` sub-agent |
| C2 | `expand_change_request` brownfield 需求展开 |
| C3 | 用户自定义 `.shuji/workflows/custom.yaml` |
| C4 | profile schema 校验 CLI |
| C5 | 全部 intent：refactor / audit / complex / simple |

---

## 三、Profile Schema 参考

### Phase B 目标 YAML

```yaml
id: brownfield_optimize
version: 1
label: 存量优化
cabinet_skill: workflow_optimize

stages:
  - id: task_record
    actor: 内阁
  - id: analysis
    actor: 中书令
    skill: code_analysis
    output_doc: anls
  - id: plan
    actor: 中书令
    skill: optimization_plan
    output_doc: plan
    requires_approval: true
  - id: execution
    actor: 尚书令
    chain: brownfield_patch
  - id: summary
    actor: 内阁
    skill: summary

gates:
  forbid_tools: [expand_requirements]
  forbid_route_to: [门下侍中]

execution_chain: brownfield_patch

escalations:
  - when_keyword: ["架构重构", "restructure"]
    to_intent: brownfield_refactor
```

### Phase A Rust 最小 struct

见主 Prompt A2。

---

## 四、Intent × Governance 矩阵

| Intent \ Governance | fast | standard | full | audit |
|---------------------|------|----------|------|-------|
| greenfield_standard | 禁 expand；少审批 | **默认** | +门下+礼部 | 偏 audit |
| brownfield_optimize | 工→刑；禁 expand | 分析→方案→朱批→工→刑 | +审批+礼部 | 可先礼部 |
| bugfix | 直工/尚书令 | 现有 bugfix | +审查 | 少见 |
| demo | 现有 demo | 现有 demo | 不适用 | 不适用 |
| auto | 解析结果 + overlay | 同左 | 同左 | 同左 |

---

## 五、给 Agent 的「不要做的事」

- 不要一次性重写 9 个 Agent 的 prompt
- 不要把 workflow 状态塞进 `state.json` 现有字段（用独立 `workflow_state.json`）
- 不要删除 `workflow_preset.json`（Phase A 双写兼容）
- 不要改 `route_to` 文档 ID 语义
- 不要为 Phase A 做 codebase survey / 新 sub-agent
- 不要 commit/push，除非用户明确要求

---

## 六、简短版 Prompt（上下文有限时用）

```markdown
在枢机 repo 实现 Workflow Phase A：

1) 新增 src-tauri/src/workflow/{config,profile,resolver,gate,chain,state}.rs
2) .shuji/workflow_config.json：intent(auto|greenfield_standard|brownfield_optimize|bugfix|demo) × governance(沿用 preset)
3) WorkflowResolver：intent≠auto 强制 inject cabinet_skill；auto 用 routing.rs，Low 禁止 silent standard
4) GateEngine：迁移 neige demo/bugfix route 门禁；brownfield 禁 expand_requirements
5) ChainEngine：greenfield_full=[吏兵工刑礼]，brownfield_patch=[工刑]；尚书令 session inject
6) 测试 tests/workflow_profile_test.rs；cargo test --tests 通过；更新 ARCHITECTURE.md

约束：最小 diff、向后兼容、保留现有 skill md 文件。
```

---

## 七、验收与测试命令

### Phase A Done 检查表

- [ ] intent=brownfield_optimize → 内阁自动加载 workflow_optimize skill
- [ ] brownfield → expand_requirements 工具报错
- [ ] brownfield → 尚书令注入 brownfield_patch 链
- [ ] auto + 模糊输入 → 内阁必须 options，不进 silent standard
- [ ] demo/bugfix gate 回归通过
- [ ] 全量 integration tests 通过

### 命令

```bash
cd shuji-app/src-tauri
cargo test --test workflow_profile_test
cargo test --test workflow_demo_test
cargo test --tests -- --skip expand_requirements
cargo clippy
cd ..
npm run lint
```

---

## 附录：一次 brownfield_optimize 消息生命周期（参考）

```
1. send_message("登录接口太慢，优化到 200ms")
2. WorkflowResolver → ActiveProfile(brownfield_optimize + standard)
3. GateEngine 挂载到内阁 ToolContext
4. 内阁：inject workflow_optimize.md + stage hint；不 inject 冲突 routing hint
5. 内阁 create task → route 中书令
6. 中书令：code_analysis → anls_001
7. 中书令：optimization_plan → plan_012
8. 内阁：<options> 朱批（governance=standard）
9. 皇帝批准 → route 尚书令
10. 尚书令：ChainEngine brownfield_patch → 工部 → 刑部 → rprt
11. 内阁 summary；workflow_state terminal
```
