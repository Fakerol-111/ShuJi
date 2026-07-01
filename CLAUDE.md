# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# 枢机（ShuJi）

> 基于三省六部制的自动化软件开发系统。每个部门是一个 LLM agent，通过角色分工和文档化通信，模拟从需求分析到编码测试的完整软件工程流程。Rust + Tauri v2 桌面应用。

## Build & Run

```bash
cd shuji-app
npm install                     # Install frontend deps
npm run tauri dev               # Dev mode with hot-reload
npm run dev                     # Browser-only (frontend only)
npm run tauri build             # Production build
```

## Test Commands

```bash
# Rust — run from shuji-app/src-tauri/
cargo test --lib                 # unit tests (config, routing, token counting, agent/util, etc.)
cargo test --tests               # integration tests (pipeline, audit, workflow, validate, etc.)
cargo test --tests -- --skip expand_requirements --test-threads=1  # Skip real-API test
cargo test --test <file> <test_name> -- --nocapture  # Single test

# Key integration test files
cargo test --test tool_test              # File CRUD, command safety, non-blocking exec
cargo test --test path_security_test     # Path traversal (22 cases)
cargo test --test document_test          # Document CRUD, type validation (24 tests)
cargo test --test actor_test             # ActorMessage, cancel, mpsc ordering (25 tests)
cargo test --test session_test           # Mock LLM, finish_reason, tool filtering
cargo test --test session_control_test   # Session control flow
cargo test --test config_test            # RuntimeConfig threshold overrides (13 tests inc. watchdog)
cargo test --test audit_test             # Audit system: lineage, RefIndex, diffs, checklists
cargo test --test checkpoint_test        # Checkpoint save/load/find (8 tests)
cargo test --test watchdog_behavior_test # Watchdog self-healing patterns
cargo test --test workflow_demo_test     # E2E mock LLM: 内阁 → 工部尚书
cargo test --test workflow_profile_test  # Workflow Profile: resolver, gate, chain, config, state
cargo test --test workflow_mock_test     # Mock ActorHarness workflow (4 scenarios)
cargo test --test pipeline_test          # Pipeline engine (validation, execution, resume, deadlock)
cargo test --test validate_test          # validate_delivery end-to-end
cargo test --test command_security_test  # Command safety checks
cargo test --test dispatch_gate_test     # Dispatch gate logic
cargo test --test editor_test            # External editor integration
cargo test --test learning_test          # Role learning store
cargo test --test send_message_routing_test  # Message routing logic
cargo test --test scenario_replay_test   # Scenario replay framework
cargo test --test expand_requirements_test  # Real LLM call (requires .env, skipped by default)

# Frontend
npm run lint                     # tsc --noEmit
npm test                         # Vitest (36 test files)
npm run format:check             # Prettier

# Rust lint
cargo clippy --all-targets
```

**Test pattern**: Async tools use `block_on()` wrapper (current-thread tokio). Async-only tests use `#[tokio::test]`. All tests use `tempfile::TempDir` via `common::create_test_project()` for isolation. Run with `--test-threads=1` to avoid state contention.

**Total**: 约 730 个测试（Rust 单元 + 集成 + 前端 Vitest，以 `scripts/count_tests.sh` 输出为准）。

**Pre-commit**: `cargo fmt --check && cargo clippy --all-targets && cargo test --lib && cargo test --tests -- --skip expand_requirements --test-threads=1`

## Environment Setup

Copy `.env.template` in `shuji-app/` or `shuji-app/src-tauri/`:
```
DEFAULT_API_KEY=sk-xxx
DEFAULT_API_URL=https://api.deepseek.com/chat/completions
DEFAULT_MODEL=deepseek-v4-flash
```
URL containing `anthropic.com` → Anthropic Messages API, otherwise → OpenAI Chat Completions.

Per-role keys override `DEFAULT_API_KEY` with `{ROLE}_API_KEY` format (9 roles: NEIGE, ZHONGSHULING, etc.).

**API config priority**: `api_config.json` (UI-managed, per-role vendor/model) > `.env` (backward compat). First-time UI save auto-migrates from `.env` to `api_config.json`.

## Architecture

> 完整架构说明（对外叙事、分层、朱批、恢复机制）见 **[shuji-app/docs/ARCHITECTURE.md](shuji-app/docs/ARCHITECTURE.md)**。以下为维护者速查。

**主路径（Pipeline-first）**：

```
皇帝需求 → send_message → 内阁 submit_pipeline_plan → PipelineEngine
  → 各部门 Actor（文档产出）→ approval_gate → validate_delivery → 审计/报告
```

Legacy `route_to` 仅用于 Pipeline 计划步骤内部转发、尚书令/执行部门任务内路由，以及 dispatch 层兼容；**不是**内阁主编排方式。

```
PipelineEngine 调度的 9 部门(actor)
  ├─ 中书令 → 方案设计 (7 skills)
  ├─ 门下侍中 → 审查 (2 skills)
  ├─ 尚书令 → 执行调度
  │   ├─ 吏部尚书 → 详细设计
  │   ├─ 兵部尚书 → 测试+接口契约
  │   ├─ 工部尚书 → TDD 编码 (批次计划循环)
  │   ├─ 刑部尚书 → 运行测试验证
  │   └─ 礼部尚书 → 规范检查+审计
  └─ expand_requirements / survey_codebase → 同步 sub-agent
```

### 9 Actors + 2 Sub-agents

Each actor is a `tokio::spawn` with an `mpsc::UnboundedReceiver` mailbox. Communication is document-centric: pipeline steps and internal `route_to` pass document IDs; receivers read documents to understand the task.

- **内阁 (Neige)**: Orchestrator. Submits `submit_pipeline_plan` to PipelineEngine (route_tool removed). Has soul system, runtime skill creation, pause/resume and must-approve gating (3 retries → auto-approve).
- **中书令 (Zhongshuling)**: Designer. Self-managed 7 skills for design/analysis/diagnosis. Skills have `## 输出块` structured output templates.
- **门下侍中 (Menxiashizhong)**: Reviewer. 2 skills: `review_overall`, `review_phase`. Skills have `## 输出块` structured output templates.
- **尚书令 (Shangshuling)**: Executor. Loads execution chain from `WorkflowState`, routes to specific ministries.
- **吏部尚书 (Libushangshu)**: Detailed design. Uses document tools only.
- **兵部尚书 (Bingbushangshu)**: Tests + contracts. Uses file-write + document tools.
- **工部尚书 (Gongbushangshu)**: TDD coding with **batch plan loop** — splits large tasks into plan batches, executes one batch per re-entry, switches reasoning on/off between planning and execution phases. Has `force_stop` for clean batch transitions.
- **刑部尚书 (Xingbushangshu)**: Test verification. Runs tests, files bugs.
- **礼部尚书 (Liburshangshu)**: Standards check + audit. Uses audit checklist tools.
- **expand_requirements**: Synchronous sub-agent, expands vague requirements into structured specs.
- **survey_codebase**: Synchronous sub-agent, scans codebase and produces analysis documents.

### Shared Agent Runner (`agent/runner.rs`)

8 non-cabinet agents share a common execution framework via `runner.rs`:
- `build_compact_handler()` — creates CompactFn callback (40-msg interval)
- `build_checkpoint_handler()` — creates CheckpointFn callback
- `load_and_compact_context()` — loads persisted context from disk, compacts, restores session
- `save_context()` — snapshots session to disk for next execution

内阁 has its own inline versions (no runner). 工部 partially uses runner (compact + checkpoint handlers) but has custom batch-plan context loading.

### Agent Trait (`agent/trait.rs`)

```rust
pub trait Agent {
    fn role(&self) -> Role;
    async fn execute(&self, input: &AgentInput) -> Result<AgentOutput>;
    fn after_execute(&self, output: &AgentOutput) -> LoopDecision { Done }
    fn set_interrupt_flag(&mut self, flag: Arc<AtomicBool>) {}
    fn reset_plan(&self) {}
    fn plan_display(&self) -> String { "null" }
}
```

`AgentInput` carries: role, task_description, context_messages, project/working dirs, configs, discuss_mode flag, fast_cancel flag. `AgentOutput` carries: content, optional route, skill name, paused state.

### Tool Registry Pattern

All agents compose tool lists via `tool::registry` group functions — factory functions returning `Vec<ToolDefinition>`:

| Group | Tools | Used By |
|---|---|---|
| `doc_inspect_tools()` | read_document, list_dir, search_text | Cabinet, designers |
| `code_inspect_tools()` | read_file, list_dir_tree, search_text | Code agents |
| `file_write_tools_for_code()` | create, edit, apply_patch, delete, rename | Code agents (no modify/append) |
| `file_write_tools()` | Full set + modify_file, append_file | Non-code agents |
| `document_tools()` | create/modify/append/set_status documents | All document workers |
| `audit_checklist_tools()` | init/update checklist, add_violation | 礼部 only |
| `execute_command_tool()` / `run_tests_tool()` | Shell commands | General / 工部 only |
| `reauth_tool()` | request_reauth | 尚书令 |

Tools return structured `ToolOutput { ok, operation, path, message, error_code }`. Dispatch via `execute_named_tool()` in `dispatch.rs` with gating logic (append_document/route_to checks approval status before proceeding), cache invalidation, and result size truncation.

### Skill System (内阁: 12 skills)

内阁 uses `<skill>name</skill>` to switch workflows. Skills are `.md` files injected as `[skill: name]` system messages:

| Skill | Purpose |
|---|---|
| `workflow_demo` | Single file, zero deps → route to 工部 directly |
| `workflow_simple` | Small scope (1-3 files) → route to 尚书令 |
| `workflow_standard` | New business logic → design → review → approval → execution |
| `workflow_complex` | Multi-stage, multi-module → full pipeline |
| `workflow_bugfix` | Bug reports and test failures |
| `workflow_refactor` | Structural changes (rename, reorganize) |
| `workflow_optimize` | Performance tuning, existing code changes |
| `workflow_audit` | Security, compliance, standards check |
| `clarify` | Ask emperor questions to understand requirements |
| `discuss` | Free chat mode, no tools, no project state modification |
| `reflect` | Post-execution review, extracts 经验/教训 to soul |
| `summary` | Summarize work done, produce completion report |

Routing heuristic (`routing.rs`): pure-function text analyzer. Priority: explicit skill mention > keyword matching (bugfix/refactor/optimize/audit/demo/complex/simple) > fallback to `workflow_standard`.

中书令 has 7 self-managed skills (design/analysis/diagnosis). 门下侍中 has 2 (review_overall, review_phase). Others have no skill system.

### Prompt Architecture (4 layers)

```
1. base_prompt (prompt.md)         — role definition, department table, tool reference
2. soul_prompt (optional)          — [soul: role] accumulated experience
3. context_messages                — skill messages, [对话摘要] summaries, recent conversation
4. user_message                    — current input
```

Skills and summaries are stored inside `context_messages` as regular system/user/assistant/tool messages (not separate layers) — maximizes LLM prefix cache hit rate.

**Optional output blocks**: 中书令/门下侍中 skill files end with `## 输出块` template for structured summaries (design conclusions, pending issues, refs, route). Other departments have output block embedded in base prompt. These survive [对话摘要] compaction so the last round's structured block retains key data.

### Session / AgentController Split

- **Session** (`api/session/mod.rs`): Pure LLM layer. Owns message history. `step()` = one API round-trip. Auto-retries on `finish_reason=length` (halving max_tokens). Handles: tool call truncation, ID validation, orphaned tool message cleanup (two-pass sanitize). `PersistedContext` for 3-layer save/load (base, soul, context). `trim_tool_results()` truncates verbose tool outputs on save.
- **AgentController** (`api/control/mod.rs`): Drive loop. Calls `step()`, executes tools, feeds results back, handles cancel/interrupt/restart. Supports `CompactFn` (persist compressed context) and `CheckpointFn` (git commit + snapshot). Watchdog: same-tool repetition, read-without-write patterns, consecutive error tracking (5 → auto-stop). Watchdog injects intervention hints into tool results to guide LLM self-correction.

### Context Compaction

Single-layer compression (`api/compact/mod.rs`) with two prompt variants:
- Cabinet context (`compact/prompt.md`) — summarizes multi-turn tool-usage context
- Department context (`compact/dept_prompt.md`) — summarizes design/coding/testing context

When `context_messages` token count exceeds threshold → older non-skill messages sent to LLM for summarization into `[对话摘要]` entry. Skill messages are stripped from the compressible batch and re-appended to the keep zone. Recent messages (default 24) preserved. Each `[对话摘要]` followed by JSON state record for workflow reconstruction.

**Mid-run compaction**: All 9 departments register CompactFn at 20-iteration intervals. Compresses + saves to `.shuji/context/{role}.json`. Running session untouched.

**Three threshold levels** (priority): `context_config.json` per-role > department built-in recommendations (`default_compact_thresholds_for_role()`) > `config.toml` global defaults.

### Batch Plan Loop (工部尚书)

工部 splits large tasks into batches (`PlanState { batches: Vec<PlanBatch>, current, complete }`):
1. LLM calls `submit_plan(batches)` → sets `force_stop` flag → AgentController exits
2. `after_execute()` returns `Continue` → actor re-enters `execute()` with next batch injected as user message
3. LLM executes batch, calls `complete_task` → sets `force_stop` → cycle repeats
4. All batches done → creates report doc, routes back to 尚书令

Reasoning enabled during planning, disabled during batch execution (thinking vs. doing separation).

### Soul System (Role Learning Memory)

All 9 long-lived actors read project soul from `.shuji/soul/{Role}.md` (e.g. `Neige.md`). Optional global soul lives at `~/.shuji/soul/{Role}.md` when global learning is enabled in `~/.shuji/learning_config.json`.

- **Injection order**: `base prompt → [soul: Role] → context_messages`
- **`update_soul` tool** (内阁): writes structured entries to project soul + `index.jsonl`; can queue `global_candidate` entries in `~/.shuji/soul/pending_global.jsonl` for UI approval
- **Limits**: 500 chars/entry, 4000 chars injected, 8KB file triggers LLM compaction
- **Restore fix**: `PersistedContext` refresh on load so stale `soul_prompt` does not override disk updates
- **Auto-extract**: pipeline completion + emperor approval notes (conservative, evidence-backed)

### Checkpoint System

Isolated git repo at `.shuji/.git/` (completely separate from project's `.git/`):
- Initialized with local git user (no global config needed)
- `.gitignore` excludes `.shuji/` from project git
- On trigger: `git add -A` + commit (skips if no changes) → session snapshot to `.shuji/checkpoints/{role}/{hash}.json`
- Index at `.shuji/checkpoints/index.json`, capped at 500 entries
- Restore: `git stash` working changes → `git checkout --detach <hash>` → restore session context
- Auto-checkpoint every 300s (configurable), plus final checkpoint after each execution

### Audit System

Event-driven audit, split into focused submodules (`audit/`):

- **Audit log** (`audit/log.rs`): Append-only `.shuji/audit.jsonl` with SHA-256 hash chain. Written by every document tool operation.
- **RefIndex** (`audit/ref_index.rs`): `.shuji/audit/ref_index.json` — `HashMap<String, RefIndexEntry>` with forward refs and reverse `ref_by` index. O(1) lookup. `check_immutability()` detects if modifying an approved document affects downstream.
- **Document lineage** (`audit/lineage.rs`): `LineageNode` recursive tree via `build_lineage()`.
- **Trace** (`audit/trace.rs`): `trace_document()` returns `TraceResult` (target, downstream, upstream) with stage classification.
- **Diff tracking** (`audit/diff.rs`): `save_diff()` computes unified diff (via `diffy` crate), stored at `.shuji/audit/diffs/`.
- **Checklist** (`audit/checklist.rs`): `.shuji/audit/checklist.json` — structured audit checklist.
- **Violations** (`audit/violation.rs`): `.shuji/audit/violations.jsonl` — severity, rule ID, status.
- **Re-auth** (`audit/reauth.rs`): `.shuji/audit/reauth_request.json` — 礼部 requests re-authentication.
- **Report** (`audit/report.rs`): Aggregated markdown delivery report.
- **Timeline** (`audit/timeline.rs`): `build_timeline()` — aggregates events by type and role.
- **Query** (`audit/query.rs`): `query_documents()` with combined filters.
- **Doc store** (`audit/doc_store.rs`): `read_doc_by_id()`, `find_by_numeric_id()`.
- **Document line** (`audit/document_line/`): End-to-end audit view linking docs, pipeline steps, approvals, validation, diffs, and semantic checkpoints. Submodules: `types.rs`, `events.rs`, `scan.rs`, `context.rs`.

### Document-Centric Architecture

Departments communicate via documents under `.shuji/`, not via route_to semantics. YAML frontmatter format with auto-assigned IDs.

**Document types**: dsgn, plan, pdsg, ddtl, revw, task, ctrt, rprt, anls, reqs, precepts.

**朱批 (Approval System)**: plan/revw documents require emperor approval before downstream can proceed. `route_to` and `append_document` hard-gate against unapproved documents. `set_document_status` tool (approved/rejected) requires `emperor_note`.

### Cancel Mechanism

Two layers:
1. **AtomicBool** (`AppState.cancel_flag`): Full workflow cancellation by user. Checked at top of every `AgentController.run()` iteration.
2. **FastMessage** (`actor/mod.rs`): 内阁 can interrupt specific departments via `cancel_agent` tool. Uses dedicated `mpsc::UnboundedSender` per actor.

### Discuss Mode

`discuss` skill → standalone `discuss_with_cabinet` Tauri command. No project state modification, no tools. Returns `ChatMessage` directly (not routed through actor system).

### Config Priority Chain

```
Runtime behavior: config.local.toml  >  config.toml  >  compile-time defaults
API credentials:  api_config.json    >  .env         >  hardcoded fallback
Compaction:       context_config.json (per-role) > department built-ins > global defaults
```

- `config.toml`: Version-controlled, team-shared runtime config (timeouts, max_tokens, iteration limits, compaction thresholds, watchdog, reasoning)
- `config.local.toml` (gitignored): Selective field overrides — only non-default values take effect
- `api_config.json` (gitignored): UI-managed per-role API key/url/model. Supports presets (balanced/economy/quality) with model mapping
- `context_config.json`: Per-role compaction threshold overrides (managed via UI or manual edit)

### Session Limits (configurable via config.toml)

| Setting | Default | Agent |
|---|---|---|
| write_file max_tokens | 0 (unlimited) | 兵部、工部 |
| append_document max_tokens | 4096 | 中书令、吏部、刑部 |
| read-only max_tokens | 2048 | 礼部 |
| write-heavy tool iterations | 60 | 兵部、工部 |
| document-heavy tool iterations | 100 | 中书令、吏部、刑部 |
| read-only tool iterations | 80 | 礼部 |
| finish_reason=length retries | 5 (halving each time) | All |
| Consecutive tool errors | 5 → auto-stop | All |
| Max plan loop iterations | 6 | 工部 only |
| Checkpoint interval | 300s | All |

### Edge Cases Handled

- **Truncated tool calls**: Filter assistant message to valid `tool_call_id`s only (prevents 400 error)
- **All tool calls broken**: Return `StepResult::Text` instead of empty `ToolCalls` (prevents infinite loop)
- **Orphaned tool messages**: Two-pass sanitize — collect all IDs first, then filter (eliminates ordering-dependent races)
- **Soul message drift**: `PersistedContext` stores `soul_prompt` separately, preserving position between base and skill prompts across save/load
- **Windows CRLF**: `log_console!` uses `write!` with explicit `\n` instead of `eprintln!`
- **Skill loop dedup**: Break loop if 内阁 outputs same `<skill>` tag twice
- **Self-routing prevention**: Base prompt forbids `route_to(to="内阁")`
- **Must-approve re-prompt guard**: 3 consecutive tries without `<options>` → auto-approve and continue
- **Compaction concurrency safety**: Active role tracking + `compacting_roles` in AppState prevents double-clicks; atomic tmp+rename writes
- **Path security** (`resolve_scoped_path`): Rejects absolute paths and `..` traversal. Falls back to ancestor-walking + canonicalize. Catches symlink escape attacks.
- **Command safety** (`check_safe_command`): Token-based matching. Blocks `sudo rm`, `format X:`, `shutdown`, `mkfs`, `dd`, `wget`/`curl` to external URLs.

### Token Tracking

Two parallel systems:
- **`token_tracker.rs`**: Persisted JSON, per-call records (prompt/cached/uncached/completion), aggregated by time windows (今日/近3天/近7天/汇总). Exposed via `get_token_stats` command.
- **`round_metrics.rs`**: Live in-memory, tracks current role, skill, cumulative tokens with cache split, dept iterations. Exposed via `get_round_metrics` command.

Cache fields parsed from API response: OpenAI `usage.prompt_tokens_details.cached_tokens` or Anthropic `usage.cache_read_input_tokens`.

### Project State Persistence

- `Project.talk`: Append-only, auto-trims to ~12 entries (oldest → summary)
- `Project.task`: Milestones (append-only, never trimmed)
- `Project.summary`: One-line status, auto-updated
- Persisted to `.shuji/state.json` on every milestone event

### API Dual-Format

Single `AnthropicClient` struct auto-detects format per request:
- URL contains `anthropic.com` → Anthropic Messages API (`x-api-key` header)
- Otherwise → OpenAI Chat Completions API (`Bearer` auth)
- Non-Anthropic APIs auto-enable reasoning/thinking tokens

### Key File Locations

```
shuji-app/
├── src/                              # Frontend (React + Vite + Tailwind CSS 4 + Vitest)
│   ├── pages/                        # WorkspaceSelect, ProjectDashboard, LogsPage, SettingsPage, SetupPage
│   ├── components/
│   │   ├── ui/                       # Primitive UI kit (Button, Card, Tabs, etc.)
│   │   ├── ChatBubble.tsx            # <options> clickable buttons
│   │   ├── ChatInput.tsx / ChatPanel.tsx
│   │   ├── CommandBar.tsx            # Pipeline stage command bar
│   │   ├── DeptStatusPanel.tsx       # Real-time dept status
│   │   ├── DeptCard.tsx / DeptCardRail.tsx / DeptInspector.tsx  # Department detail views
│   │   ├── ReasoningPopover.tsx      # LLM reasoning/thinking content display
│   │   ├── DocPreview.tsx / DocTree.tsx  # Document browser
│   │   ├── DecisionPanel.tsx / AuditPanel.tsx  # Decision/audit tabs
│   │   ├── CheckpointPanel.tsx       # Checkpoint snapshots list/restore
│   │   ├── TokenPanel.tsx / ContextPanel.tsx  # Sidebar panels
│   │   ├── ProjectOverview.tsx / WorkflowTimeline.tsx
│   │   ├── HelpDrawer.tsx / DemoTour.tsx
│   │   ├── SettingsMenu.tsx / SealLogo.tsx
│   │   └── settings/                 # Settings tabs: ApiSettingsTab, ReasoningSettingsTab, etc.
│   ├── hooks/                        # React hooks: useChat, useClickOutside, usePendingApprovals, etc.
│   ├── utils/                        # chat.ts, error.ts, approvalGate.ts, etc.
│   ├── constants/                    # constants.ts, reasoning.ts, presets.ts
│   ├── api.ts                        # Tauri invoke wrappers
│   ├── types.ts                      # TypeScript type definitions (RoleName union, etc.)
│   └── test/setup.ts                 # Vitest setup (jsdom, testing-library)
└── src-tauri/src/
    ├── commands/                     # Tauri command handlers
    │   ├── project.rs                # Project CRUD + demo generator
    │   ├── settings/                 # Settings submodules: api_config, reasoning, approval, etc.
    │   ├── checkpoint.rs             # list/restore checkpoints
    │   ├── shuji_docs.rs             # .shuji/ file tree + doc viewer
    │   └── workflow/                 # send_message, compact, context_stats, audit, bootstrap
    ├── actor/mod.rs                  # Actor system: run_actor, ActorContext, FastMessage/FastChannel
    ├── agent/
    │   ├── trait.rs                  # Agent trait + AgentInput/Output, LoopDecision
    │   ├── runner.rs                 # Shared execution framework (compact/checkpoint/context helpers)
    │   ├── util.rs                   # Tag extraction helpers
    │   ├── expand_requirements.rs    # Requirements sub-agent
    │   ├── survey_codebase.rs        # Codebase survey sub-agent
    │   ├── neige/                    # 内阁: mod.rs, prompt.md, routing.rs, skills/ (12 .md files)
    │   ├── zhongshuling/             # 中书令: mod.rs, prompt.md, skills/ (7 skills)
    │   ├── menxiashizhong/           # 门下侍中: mod.rs, prompt.md, skills/ (2 skills)
    │   ├── shangshuling/             # 尚书令: mod.rs, prompt.md
    │   ├── libushangshu/             # 吏部: mod.rs, prompt.md
    │   ├── bingbushangshu/           # 兵部: mod.rs, prompt.md
    │   ├── gongbushangshu/           # 工部: mod.rs, prompt.md (batch plan loop)
    │   ├── xingbushangshu/           # 刑部: mod.rs, prompt.md
    │   └── liburshangshu/            # 礼部: mod.rs, prompt.md
    ├── api/
    │   ├── client.rs                 # AnthropicClient (dual-format HTTP)
    │   ├── session/                  # Session, PersistedContext, step()
    │   │   ├── mod.rs                # Session 门面、构造、状态 setter
    │   │   ├── types.rs              # ToolCallInfo, StepResult, SessionSnapshot
    │   │   ├── persisted_context.rs  # 3-layer save/load
    │   │   ├── request.rs            # build_step_body, api_request
    │   │   ├── response.rs           # process_assistant_message, tool call 解析
    │   │   ├── length_retry.rs       # finish_reason=length 处理
    │   │   ├── stream.rs             # step_stream
    │   │   ├── debug.rs              # write_debug_truncated
    │   │   └── token_usage.rs        # usage 解析、token_tracker 记录
    │   ├── control/                  # AgentController: tool loop, watchdog, callbacks
    │   │   ├── mod.rs                # AgentController 门面、构造、setter
    │   │   ├── types.rs              # public types
    │   │   ├── iterations.rs         # 迭代上限判断
    │   │   ├── loop_runner.rs        # run() 主循环
    │   │   ├── lifecycle.rs          # cancel/checkpoint/compact 检查
    │   │   ├── tool_exec.rs          # read-only 并发执行、工具结果 feed
    │   │   ├── routing.rs            # route_to 解析、自路由检查
    │   │   ├── watchdog.rs           # repetition/error/read-without-write 状态
    │   │   └── wrap_up.rs            # max-iteration 收尾
    │   ├── compact/                  # Context compaction (2 prompt variants)
    │   ├── reasoning.rs              # Per-vendor reasoning/thinking token injection
    │   ├── intent.rs                 # User intent classification
    │   ├── stream.rs                 # Streaming response handling
    │   └── token_count.rs            # Token counting utilities
    ├── tool/
    │   ├── registry.rs               # Tool group factory functions
    │   ├── dispatch.rs               # Central tool dispatch + gate logic
    │   ├── file_ops.rs / documents/ / command_ops.rs / audit_tools.rs
    │   ├── neige_special.rs          # 内阁-specific tools (cancel_agent, create_skill, etc.)
    │   ├── shangshuling_special.rs   # 尚书令-specific tools
    │   ├── editor.rs / lint_ops.rs / python_cmd.rs / test_env.rs
    │   ├── cache.rs / path.rs / output.rs / tool_log.rs
    ├── validate/                     # Delivery validation: contract, lint, diff, tests_runner
    ├── learning/                     # Role learning: store, extract, inject, config
    ├── pipeline/                     # PipelineEngine: engine/ (run_loop, step, route, graph, metrics), handlers, artifacts, supervisor
    ├── workflow/                     # Workflow Profile: state, stage, graph
    ├── metrics/                      # Metrics aggregation
    ├── scenario/                     # Scenario replay framework
    ├── precepts/                     # Rule/policy management
    ├── audit/                        # Audit subsystem
    │   ├── mod.rs                    # Public re-exports (facade)
    │   ├── log.rs                    # AuditEntry, hash chain, append, read_all, verify
    │   ├── ref_index.rs              # RefIndex, build_ref_index, check_immutability
    │   ├── document_line/            # Document line: types, events, scan, context (graph build + impact)
    │   ├── checklist.rs / violation.rs / reauth.rs / diff.rs
    │   ├── lineage.rs / trace.rs / report.rs / timeline.rs / query.rs / doc_store.rs
    ├── models/                       # role.rs, chat.rs, message.rs, project.rs
    ├── config/                       # RuntimeConfig: types.rs (structs + load/merge), reasoning.rs, approval.rs, compaction.rs
    ├── storage/                      # shuji_dir.rs, checkpoint.rs
    ├── logging/logger.rs             # Department-scoped JSONL logging
    ├── playbook/                     # Watchdog playbook patterns
    ├── templates/                    # Document templates
    ├── round_metrics.rs / token_tracker.rs
    └── lib.rs                        # Tauri builder, plugin registration
```

### Code Style

- **Rust**: `cargo fmt` (4-space indent), clean `clippy` warnings before commit, prefer `Result<_, String>` / `anyhow::Result<_>`, avoid `unwrap()`
- **TypeScript/React**: Prettier formatting (`npm run format`), `ChatMessage.role` is `RoleName` union type, new generic components in `components/ui/` with barrel export, new hooks prefixed `use` in `hooks/`
- **Events**: Tauri events in kebab-case: `chat-message`, `dept-log`, `plan-update`, `project-update`

### Design Philosophy

- **文档是契约**: Departments communicate via documents, not LLM conversation context
- **流程适配任务**: 内阁 picks cheapest workflow matching task complexity
- **职责隔离**: Designers don't code, coders don't review, testers don't analyze
- **Soul 学习**: Cabinet accumulates experience across sessions
- **可审计性**: All key steps auto-logged, document diffs preserved, bidirectional lineage tracked

### Project Status

Core actor system, collaboration flow, document system, audit system, checkpoint system, pipeline engine, and frontend work end-to-end. 约 730 个测试（以 `scripts/count_tests.sh` 输出为准）。
