# 枢机（ShuJi）工具（Tool）优化清单

> 基于 **2026-06-07 代码实读** 整理，以 `src-tauri/src/tool/`、`api/control.rs`、`api/session.rs`、各 agent `mod.rs` / prompt 为准。  
> 目标：**减少 tool 调用轮次、降低 context token 消耗、减少工具选择错误与 watchdog 误触发**。

---

## 如何使用本文档

1. **先做 P0**：一轮 API 少调几次工具，收益最大。
2. **再做 P1**：减 token、减误调用。
3. **最后 P2/P3**：执行层加速、prompt 对齐。
4. 每完成一项，将 `[ ]` 改为 `[x]`，并跑对应验收步骤。

---

## 现状速览（代码事实）

### 工具注册入口

| 文件 | 职责 |
|------|------|
| `src-tauri/src/tool/registry.rs` | 工具分组：`inspect_tools`、`file_write_tools`、`document_tools` 等 |
| `src-tauri/src/tool/mod.rs` | 工具实现 + `execute_named_tool` 统一分发 |
| `src-tauri/src/tool/documents.rs` | 文档 CRUD、`read_document`、`find_document` |
| `src-tauri/src/api/session.rs:346-348` | **每个 agent 自动注入 `route_to`** |
| `src-tauri/src/api/control.rs` | tool 循环、watchdog、串行执行 |

### 各部门当前工具数量（含自动注入的 `route_to`）

| 部门 | 约计 | 组成 |
|------|------|------|
| 兵部 / 中书令 / 吏部 / 门下 / 尚书令 | ~10 | `inspect_tools`(5) + `document_tools`(4) + `route_to` |
| 工部 | **~18** | inspect(5) + file_write(6) + document(4) + run_tests + submit_plan + complete_task + route_to |
| 刑部 | **~17** | inspect(5) + file_write(6) + document(4) + execute_command + route_to |
| 礼部 | ~13 | inspect(5) + document(4) + audit_checklist(3) + route_to |
| 内阁 | **~15** | inspect(5) + document(4) + summarize + 4 special + route_to |

### `inspect_tools()` 五件套（所有文档型部门共用）

```
read_file | list_dir | find_document | read_document | search_text
```

### 已知低效模式

| 模式 | 典型轮次 | 根因 |
|------|----------|------|
| `find_document` → `read_file` | 2 轮 | `read_document` 已合并二者，prompt 仍教旧路径 |
| `list_dir` × N 再 `read_file` | 3+ 轮 | `list_dir` 只列一层 |
| `modify_file` 失败 → read/delete/create | 3+ 轮 | 错误提示未引导 `apply_patch` |
| `create_document` + `append_document` × 8 | 9 轮 | 无批量写入接口 |
| 同路径重复 `read_file` | 2+ 轮 | 无 session 内读缓存 |

---

## 优先级总览

| 优先级 | 数量 | 主题 |
|--------|------|------|
| **P0** | 5 | 按角色拆 registry、去掉 find、写文件收敛、route_to 条件注入、list_dir 增强 |
| **P1** | 6 | 运行时截断、read_document 默认策略、schema 校验、内阁/礼部瘦身、刑部 run_tests |
| **P2** | 4 | 并行只读、读缓存、ripgrep search、批量写文档 |
| **P3** | 3 | Prompt 统一、watchdog 扩展、文档写入决策树 |

---

## P0 — 减少调用轮次

### P0-1 按角色拆分工具组，废除全员 `inspect_tools()`

**现象**：`registry.rs` 仅有一套 `inspect_tools()`，文档型与编码型部门需求不同，却共用 5 个读工具。

**代码**：
- `registry.rs:12-19` — `inspect_tools()`
- 各 agent `mod.rs` — 均 `tools.extend(inspect_tools())`

**修复方向**：在 `registry.rs` 新增分组函数，各 agent 按需组合：

```rust
// 建议新增（名称可调整）
pub fn doc_inspect_tools() -> Vec<ToolDefinition> {
    // read_document, list_dir
}
pub fn code_inspect_tools() -> Vec<ToolDefinition> {
    // read_file, search_text, list_dir（或 list_dir_tree）
}
pub fn minimal_inspect_tools() -> Vec<ToolDefinition> {
    // read_document only
}
```

**各部门目标工具集**：

| 部门 | 读工具 | 写工具 | 其他 |
|------|--------|--------|------|
| 内阁 | `read_document`, `list_dir` | `create_document`, `append_document` | special 见 P1-5 |
| 中书令/门下/吏部/兵部/尚书令 | `read_document`, `list_dir`, `search_text`? | `document_tools` 全套 | `route_to` |
| 工部 | `read_file`, `list_dir_tree`, `search_text` | 见 P0-3 | `run_tests`, plan 工具, `route_to` |
| 刑部 | 同工部读 | 见 P0-3 | `run_tests`, `route_to` |
| 礼部 | `read_file`, `read_document` | `create_document`, `append_document` | audit 三件套, `route_to` |

**涉及文件**：
- `src-tauri/src/tool/registry.rs`
- `src-tauri/src/agent/*/mod.rs`（9 个部门 + sub-agent）

**验收**：
- [ ] 礼部 tools 列表不含 `search_text`（若不需要）
- [ ] 工部 tools 列表不含 `find_document` / `read_document`
- [ ] `cargo test --lib` 通过

---

### P0-2 去掉或降级 `find_document`

**现象**：`read_document` 已合并 find + YAML 解析 + 可选 section（`documents.rs:782-865`），但各 prompt 仍教 `find_document` + `read_file`。

**代码**：
- `documents.rs:922-971` — `tool_find_document`
- `registry.rs:16` — 仍在 `inspect_tools` 中
- Prompt 引用：`zhongshuling/prompt.md`, `gongbushangshu/prompt.md`, `shangshuling/prompt.md` 等

**修复方向**（二选一）：
1. **推荐**：从所有 agent 工具列表移除 `find_document`；`execute_named_tool` 保留实现，返回「请改用 read_document(id=...)」
2. 保留但仅在 `neige_inspect` 中，描述改为「仅当 read_document 失败时的 fallback」

**同步改 prompt**：全文搜索 `find_document`，改为 `read_document(id, section?, max_chars?)`。

**验收**：
- [ ] grep 各 prompt 无「先 find 再 read」工作流
- [ ] `read_document` 工具描述写明「首选，替代 find_document + read_file」

---

### P0-3 工部/刑部写文件工具收敛为「三板斧」

**现象**：`file_write_tools()` 有 6 个工具，LLM 常在 `modify_file`(≤800) / `apply_patch`(50KB) / `append_file` 间犹豫；`modify_file` 失败提示建议 read→delete→create（3 轮）。

**代码**：
- `registry.rs:23-31` — `file_write_tools()`
- `mod.rs:471-519` — `tool_modify_file` 错误文案
- `gongbushangshu/prompt.md` — 已提 apply_patch，但与 6 工具并存

**修复方向**：

工部 / 刑部仅保留：
```
create_file | apply_patch | delete_file | rename_file
```

移除或不对其暴露：
```
modify_file | append_file
```

在 `gongbushangshu/prompt.md` / `xingbushangshu/prompt.md` 写死决策树：

```markdown
- 改已有文件 → apply_patch（任何幅度）
- 新建文件   → create_file（>8KB：create 空文件 + apply_patch）
- 删除/改名  → delete_file / rename_file
- 禁止 modify_file / append_file
```

修正 `modify_file` 全局错误提示（若其他部门仍保留）：引导 `apply_patch` 而非 read→delete→create。

**验收**：
- [ ] 工部 `tools()` 返回 4 个写文件工具（非 6 个）
- [ ] Demo calc 修复任务 tool 轮次可观察下降

---

### P0-4 `route_to` 改为按角色注入，取消全局注入

**现象**：`Session::new` 给**所有** agent 注入 `route_to`，包括不应路由的 sub-agent；每轮 API 多占 schema token，且可能误路由。

**代码**：
- `api/session.rs:346-348` — 无条件 `all_tools.push(route_tool())`
- `registry.rs:60-87` — `route_tool()` 定义

**修复方向**：

1. 从 `Session::new` 移除自动注入
2. 在需要路由的 agent `tools()` 中显式 `push(route_tool())`：

| 需要 route_to | 不需要 |
|---------------|--------|
| 内阁、中书令、门下、尚书令、吏部、兵部、工部、刑部、礼部 | `expand_requirements` sub-agent、`survey_codebase` sub-agent |

3. 可选：`route_tool()` 的 `to` enum 按调用方角色缩小（如兵部只能 `to=尚书令`）

**验收**：
- [ ] sub-agent 工具列表无 `route_to`
- [ ] 内阁 route 行为不变

---

### P0-5 增强目录浏览：`list_dir_tree` 或 `list_dir` 加 `depth`

**现象**：`list_dir` 只列一层（`mod.rs:768-801`），探索代码库需多次 `list_dir` + `read_file`。

**修复方向**（二选一）：

**方案 A** — 新工具 `list_dir_tree`：
```json
{
  "path": "src-tauri",
  "depth": 2,
  "glob": "*.rs"
}
```
返回缩进树形文本，单层最多 N 项，超出显示 `... (+12 more)`。

**方案 B** — 扩展 `list_dir` 参数：
```json
{ "path": ".", "depth": 2, "glob": "*.py" }
```

**涉及文件**：
- `tool/mod.rs` — 实现
- `registry.rs` — 加入 `code_inspect_tools`
- `gongbushangshu/prompt.md`, `xingbushangshu/prompt.md`, `survey_codebase_prompt.md`

**验收**：
- [ ] 一次调用可列出 `src-tauri/src/agent/` 下所有 `mod.rs`
- [ ] 深度/项数有上限，防止 token 爆炸

---

## P1 — 减 token / 减误调用

### P1-1 运行时截断 tool 结果（不仅 persist 时）

**现象**：`trim_tool_results(2000)` 仅在 `PersistedContext` 保存时调用；运行中 `feed_tool_result` 可塞入全文。

**代码**：
- `api/session.rs:109-117` — `trim_tool_results`
- `api/control.rs:541` — `session.feed_tool_result(&tc.id, &tc.name, &tool_content)` 无截断
- `mod.rs:690-726` — `read_file` 最多 200 行全文返回
- `mod.rs:558-595` — `execute_command` 返回完整 stdout

**修复方向**：在 `control.rs` 写入 session 前统一 `truncate_tool_result(name, content)`：

| 工具 | 建议上限 |
|------|----------|
| `read_file` / `read_document` | 8000 字符 + 提示续读方式 |
| `search_text` | 保持 max_results=50，单行 ≤200 字符 |
| `list_dir` / `list_dir_tree` | 8000 字符 |
| `execute_command` | stdout+stderr 各 2000（对齐 `run_tests`） |
| `summarize_logs` | 4000 字符 |

**验收**：
- [ ] 读 500 行文件后 context 增量可控
- [ ] 截断消息含 `truncated: true` 与续读提示

---

### P1-2 `read_document` 默认策略 + prompt 推广

**现象**：`read_document` 支持 `section`、`max_chars`（`documents.rs:816-834`），但 prompt 几乎不提；默认返回全文。

**修复方向**：
1. 工具描述改为强调：`read_document(id)` 首次建议 `max_chars=4000`；已知章节用 `section="接口设计"`
2. 可选：文档型 agent 首次调用无 `max_chars` 时默认 4000（代码层）
3. 更新所有设计类 prompt 的工具表

**涉及文件**：
- `documents.rs` — `read_document_tool_def`
- `agent/zhongshuling/prompt.md`, `menxiashizhong/`, `libushangshu/`, `bingbushangshu/`, `shangshuling/`

**验收**：
- [ ] 设计 agent 读 50KB 文档不会一次灌满 context

---

### P1-3 补齐 schema 与运行时长度校验

**现象**：`modify_document` schema 写 ≤300 字符，`append_document` 写 ≤2000，但 `tool_modify_document` / `tool_append_document` **无运行时 `len()` 检查**（与 `modify_file` / `append_file` 不一致）。

**代码**：
- `documents.rs:336-349` — modify 无长度检查
- `documents.rs:373-448` — append 无长度检查
- `documents.rs:483-524` — schema maxLength 已声明

**修复方向**：与 `mod.rs` 中 file 工具一致，超长返回结构化 `error_code: content_too_long`。

**验收**：
- [ ] 传入 5000 字符 `append_document` 一次失败并提示拆分，不浪费一轮 LLM

---

### P1-4 刑部用 `run_tests` 替代裸 `execute_command`（测试场景）

**现象**：工部有结构化 `run_tests`（自动检测项目类型、解析通过/失败、截断 stderr）；刑部仍用 `execute_command`。

**代码**：
- `xingbushangshu/mod.rs:25-30` — 无 `run_tests`
- `gongbushangshu/mod.rs:77` — 有 `run_tests`
- `mod.rs:1149-1286` — `tool_run_tests` 实现

**修复方向**：
- 刑部 `tools()` 加 `run_tests_tool()`，移除或降级 `execute_command`（仅保留 lint/format 专用描述）
- 更新 `xingbushangshu/prompt.md`

**验收**：
- [ ] 刑部验证测试时调用 `run_tests`，不手写 `cargo test` 命令

---

### P1-5 内阁工具瘦身

**现象**：内阁工具过多（`neige/mod.rs:40-49`），部分与 UI/皇帝流程重复。

**当前**：
```
inspect(5) + document(4) + summarize_logs + cancel_agent + update_soul
+ create_skill + expand_requirements + survey_codebase + route_to
```

**建议**：

| 工具 | 决策 |
|------|------|
| `set_document_status` | **移除**（朱批走 UI `DocPreview`） |
| `modify_document` | **移除**（内阁主要 create + append） |
| `find_document` | **移除**（见 P0-2） |
| `search_text` | discuss 模式移除；决策模式可选保留 |
| `expand_requirements` + `survey_codebase` | 中期合并为 `spawn_subagent(type, task)` 一个 schema |

**`discuss_tools()`**（`neige/mod.rs:53-56`）目标：
```
read_document, list_dir, summarize_logs  // 只读 3 个
```

**验收**：
- [ ] 内阁决策模式 tools ≤ 12 个
- [ ] discuss 模式 tools ≤ 4 个

---

### P1-6 礼部工具瘦身

**现象**：礼部有完整 `document_tools` + `inspect_tools` + audit 三件套。

**建议**：
- 读：`read_file`, `read_document`（去掉 `find_document`, `list_dir`, `search_text` 或仅保留 `list_dir`）
- 写：`create_document`, `append_document`（去掉 `modify_document`, `set_document_status`）
- 审计：保留 `init_checklist`, `update_checklist_item`, `add_violation`
- 路由：`route_to` 或仅用 `request_reauth`（`registry.rs:308-330`）

**验收**：
- [ ] 礼部 tools ≤ 10 个

---

## P2 — 执行层加速

### P2-1 同轮只读 tool call 并行执行

**现象**：`control.rs` 对 API 返回的多个 tool call **串行** `for` 执行；读操作无依赖可并行。

**代码**：
- `api/control.rs:297-541` — 串行循环
- `api/session.rs:480` — `parallel_tool_calls=false` 仅在 `tool_choice_none` 时

**修复方向**：
1. 将一轮 calls 分为 `reads`（read_file, read_document, list_dir, find_document, search_text）与 `writes`
2. `reads` 用 `futures::future::join_all` 并行
3. `writes` 仍串行（避免文件竞争）

**验收**：
- [ ] 一轮 3 个 `read_file` wall-clock 接近 1 次而非 3 次

---

### P2-2 Session 内文件读缓存

**现象**：同一 actor 循环内重复 `read_file` 同路径，触发 watchdog 同工具重复（`control.rs:313-326`）。

**修复方向**：
- 在 `AgentController` 或 `ToolContext` 维护 `HashMap<PathBuf, (mtime, content_hash, result)>`
- 命中时返回 `{"ok":true,"cached":true,"message":"..."}`
- 任何 write/delete/rename/patch 后 invalidate 该 path

**验收**：
- [ ] 连续两次 read 同文件第二次标记 cached
- [ ] patch 后第三次 read 重新读盘

---

### P2-3 `search_text` 改用 ripgrep

**现象**：当前递归 `read_dir` + 全文读入（`mod.rs:888-946`），大仓库慢且占内存。

**修复方向**：
1. 优先 `rg --json -e pattern --glob GLOB`（项目根为 cwd）
2. 无 rg 时 fallback 现有实现
3. 统一 max_results、跳过目录列表与现有一致

**验收**：
- [ ] 10k 文件仓库搜索 < 2s
- [ ] 无 rg 环境仍可用

---

### P2-4 文档批量写入：`write_document` 或 `append_document` 多段

**现象**：兵部/中书令典型 `create_document` + `append_document` × N（8+ 轮）。

**修复方向**（二选一）：

**方案 A** — 扩展 `append_document`：
```json
{
  "id": "ctrt_5",
  "contents": ["## 模块 A\n...", "## 模块 B\n..."],
  "max_total_chars": 8000
}
```

**方案 B** — 新工具 `write_document_sections`：
```json
{
  "id": "ctrt_5",
  "sections": [
    { "heading": "User API", "content": "..." },
    { "heading": "Order API", "content": "..." }
  ]
}
```

**验收**：
- [ ] 创建含 5 节的契约文档 ≤ 3 轮 tool call

---

## P3 — Prompt 与 Watchdog 对齐

### P3-1 统一各 prompt 工具表与工作流

**需全文搜索替换的 prompt 文件**：

| 文件 | 改动要点 |
|------|----------|
| `agent/zhongshuling/prompt.md` | `find_document` → `read_document` |
| `agent/gongbushangshu/prompt.md` | 写文件决策树；`list_dir_tree` |
| `agent/xingbushangshu/prompt.md` | `run_tests`；写文件决策树 |
| `agent/bingbushangshu/prompt.md` | `read_document` 读契约 |
| `agent/shangshuling/prompt.md` | `read_document` |
| `agent/menxiashizhong/prompt.md` | 读 revw/dsgn 用 `read_document` |
| `agent/libushangshu/prompt.md` | 同上 |
| `agent/liburshangshu/prompt.md` | 审计流程工具表 |
| `agent/neige/prompt.md` | 工具列表与 `tools()` 一致 |
| `agent/expand_requirements_prompt.md` | 已正确：直接 `read_file` task 路径 |
| `agent/survey_codebase_prompt.md` | `list_dir_tree` 替代多层 list_dir |

**验收**：
- [ ] `rg find_document agent/` 仅剩 fallback 说明或为零
- [ ] 各 prompt 工具表与对应 `mod.rs tools()` 一致

---

### P3-2 扩展 Watchdog 计数范围

**现象**：
- 读而不写只计 `read_file | list_dir | find_document`（`control.rs:351-352`）
- 同工具重复只比 `path` 或 `command`（`control.rs:314-319`）

**修复方向**：
```rust
let is_read = matches!(tc.name.as_str(),
    "read_file" | "list_dir" | "list_dir_tree" | "find_document"
    | "read_document" | "search_text");
// 同工具 key_arg：
// read_document / append_document / modify_document → args["id"]
```

**验收**：
- [ ] 连续 5 次 `read_document` 无写入会注入干预提示

---

### P3-3 工具描述内嵌决策树（减 LLM 选择成本）

在 `registry.rs` 各 `*_tool_def` 的 `description` 中加入一行「何时用 / 何时不用」：

| 工具 | 描述补充 |
|------|----------|
| `read_document` | 读 .shuji 文档首选；不要用 find_document + read_file |
| `apply_patch` | 修改已有代码文件首选；不要用 modify_file |
| `create_file` | 仅新建；文件已存在会报错 |
| `run_tests` | 跑测试首选；不要手写 cargo test / pytest |
| `execute_command` | 仅 lint/format/构建等非测试命令 |

**验收**：
- [ ] 新对话工部首次写代码直接用 apply_patch 的比例上升（可人工观察或查 tool log）

---

## 实施状态

| 项 | 状态 | 说明 |
|----|------|------|
| P0-1 按角色拆 registry | ✅ | doc_inspect / code_inspect / minimal_inspect + file_write_tools_for_code |
| P0-2 降级 find_document | ✅ | 从所有 agent 列表移除；description 标记为 fallback |
| P0-3 写文件收敛 | ✅ | 工部/刑部仅 create/apply_patch/delete/rename + modify_file 错误提示改向 apply_patch |
| P0-4 route_to 条件注入 | ✅ | 移出 Session::new，各 agent tools() 显式 push；sub-agent 无 route_to |
| P0-5 list_dir_tree | ✅ | 新工具：支持 depth/glob，默认 depth=2，上限 200 项 |
| P1-1 运行时截断 | ✅ | truncate_tool_result_by_name()：read_file/read_document ≤8000，exec ≤4000 等 |
| P1-2 read_document 默认值 | ✅ | 未传 max_chars 时默认 4000，描述已更新 |
| P1-3 文档工具校验 | ✅ | modify_document new_text ≤300、append_document content ≤2000 运行时检查 |
| P1-4 刑部 run_tests | ✅ | file_write_tools_for_code + run_tests_tool，移除 execute_command |
| P1-5 内阁瘦身 | ✅ | minimal_inspect_tools + create/append → ~10 工具；discuss 模式 3 个 |
| P1-6 礼部瘦身 | ✅ | 定制工具集：read_file + read_document + create/append + audit 三件套 + route |
| P2-1 并行只读 | ✅ | join_all 并发执行 reads，writes 串行 |
| P2-2 读缓存 | ✅ | static LazyLock<HashMap>，mtime 校验，写操作自动 invalidation |
| P2-3 ripgrep search | ✅ | try_rg_search() 优先，无 rg 时 fallback 现有实现 |
| P2-4 文档批量写入 | ✅ | append_document contents 数组（≤5 项，每项 ≤2000） |
| P3-1 prompt 同步 | ✅ | 4 个 prompt 中的 find_document → read_document，工具列表对齐 |
| P3-2 Watchdog 扩展 | ✅ | key_arg 含 id，is_read 含 read_document/search_text/list_dir_tree |
| P3-3 工具描述决策树 | ✅ | run_tests/execute_command/modify_file 描述内嵌"首选/勿用"引导 |

## 建议实施顺序（里程碑）

### ✅ 里程碑 1：减轮次（已完成）

1. ✅ P0-1 按角色拆 registry
2. ✅ P0-2 去掉 find_document
3. ✅ P0-3 工部/刑部写文件收敛
4. ⬜ P3-1 prompt 同步（可后续逐步进行）

### ✅ 里程碑 2：减 token（已完成）

5. ✅ P0-4 route_to 条件注入
6. ✅ P1-1 运行时截断
7. ✅ P1-2 read_document 默认 max_chars
8. ✅ P1-3 文档工具长度校验

### ✅ 里程碑 3：部门定制（已完成）

9. ✅ P1-4 刑部 run_tests
10. ✅ P1-5 内阁瘦身
11. ✅ P1-6 礼部瘦身

### ✅ 里程碑 4：基础设施（已完成）

12. ✅ P0-5 list_dir_tree
13. ✅ P2-1 并行只读
14. ✅ P2-2 读缓存
15. ✅ P2-3 ripgrep search

### ✅ 里程碑 5：批量写入与收尾（已完成）

16. ✅ P2-4 文档批量写入
17. ✅ P3-2/P3-3 watchdog + 描述优化
18. ✅ P3-1 prompt 同步

---

## 目标态：各部门工具清单

完成后各部门工具应接近下表（`route_to` 仅列在需要的路由部门）：

### 内阁（决策模式）

```
read_document, list_dir
create_document, append_document
summarize_logs, cancel_agent, update_soul
expand_requirements, survey_codebase, create_skill
route_to
```

### 内阁（廷议 / discuss 模式）

```
read_document, list_dir, summarize_logs
```

### 中书令 / 门下 / 吏部 / 兵部 / 尚书令

```
read_document, list_dir, search_text
create_document, modify_document, append_document, set_document_status
route_to
[+ 尚书令: request_reauth]
```

### 工部

```
read_file, list_dir_tree, search_text
create_file, apply_patch, delete_file, rename_file
create_document, append_document          // 报告用
run_tests, submit_plan, complete_task
route_to
```

### 刑部

```
read_file, list_dir_tree, search_text
apply_patch, delete_file                  // 仅修测试，通常不需 create_file
create_document, append_document        // 报告用
run_tests
route_to
```

### 礼部

```
read_file, read_document
create_document, append_document
init_checklist, update_checklist_item, add_violation
route_to
```

---

## 关键文件索引

| 文件 | 改动类型 |
|------|----------|
| `src-tauri/src/tool/registry.rs` | 工具分组、各部门组合 |
| `src-tauri/src/tool/mod.rs` | list_dir_tree、截断、rg search、缓存 |
| `src-tauri/src/tool/documents.rs` | read_document 默认值、批量 append、长度校验 |
| `src-tauri/src/api/session.rs` | 移除全局 route_to 注入 |
| `src-tauri/src/api/control.rs` | 运行时截断、并行读、watchdog |
| `src-tauri/src/agent/*/mod.rs` | 各部门 `tools()` |
| `src-tauri/src/agent/**/*.md` | prompt 工具表与工作流 |

---

## 验收回归清单（发版前）

```
[ ] cargo test --lib
[ ] cargo test --test tool_test
[ ] Demo calc：工部 tool 轮次可观察下降
[ ] 中书令读 dsgn 文档：单轮 read_document，无 find+read
[ ] 刑部跑测试：调用 run_tests 而非 execute_command
[ ] 内阁 discuss：工具 ≤4 个，无法写文档
[ ] 读 300 行文件：context 中 tool result 被截断
[ ] .shuji/logs/tool-calls/ 抽查无大量重复 read_file 同路径
```

---

## 附录：当前工具参数上限（代码默认值）

| 工具 | 参数限制 | 文件位置 |
|------|----------|----------|
| `create_file` | content ≤ 8000 | `mod.rs:736-746` |
| `append_file` | content ≤ 2000 | `mod.rs:215-218` |
| `modify_file` | old/new ≤ 800 | `mod.rs:497-504` |
| `apply_patch` | patch ≤ 50000 | `mod.rs:390-400` |
| `read_file` | >200 行需 offset/limit | `mod.rs:708-712` |
| `append_document` | schema 2000，**运行时未校验** | `documents.rs:513-524` |
| `modify_document` | schema 300，**运行时未校验** | `documents.rs:483-499` |
| `execute_command` | 超时 120s | `mod.rs:574` |
| `run_tests` | 超时 300s，stderr 截断 2000 | `mod.rs:1207-1274` |

---

*文档生成自代码审计。若实现与本文冲突，以代码为准并更新本文。*
