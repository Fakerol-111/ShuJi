# 枢机（ShuJi）

> 基于三省六部制的自动化软件开发系统。Phase 2：Rust + Tauri v2 桌面应用。

## Build & Run

```bash
# Backend (Tauri dev)
cd shuji-app
npm install
npm run tauri dev        # Dev mode with hot-reload

# Frontend only (Vite)
npm run dev              # Browser-only dev

# Build for production
npm run tauri build

# Rust checks
cd src-tauri
cargo check              # Fast type-check (preferred)
cargo build              # Full build
cargo clippy             # Lint

# Frontend type-check
npm run build            # tsc + vite build
```

Set up `.env` in `shuji-app/` or `shuji-app/src-tauri/` before running (copy from `.env.template`):
```
DEFAULT_API_KEY=sk-xxx
DEFAULT_API_URL=https://api.deepseek.com/chat/completions
DEFAULT_MODEL=deepseek-chat
```
URL with `anthropic.com` → Anthropic Messages API, otherwise → OpenAI Chat Completions.

Per-role keys override `DEFAULT_API_KEY`:
```
NEIGE_API_KEY=sk-xxx
ZHONGSHULING_API_KEY=sk-xxx
MENXIASHIZHONG_API_KEY=sk-xxx
SHANGSHULING_API_KEY=sk-xxx
LIBUSHANGSHU_API_KEY=sk-xxx
BINGBUSHANGSHU_API_KEY=sk-xxx
GONGBUSHANGSHU_API_KEY=sk-xxx
XINGBUSHANGSHU_API_KEY=sk-xxx
LIBURSHANGSHU_API_KEY=sk-xxx
```

## Architecture

### Actor Model

```
皇帝 → send_message → 内阁(actor) → route_to → 各部门(actor)
                                                     ├─ 中书令 → 方案设计
                                                     ├─ 门下侍中/门下给事中 → 审查
                                                     ├─ 尚书令 → 调度执行
                                                     │   ├─ 吏部尚书 → 详细设计
                                                     │   ├─ 兵部尚书 → 测试+契约
                                                     │   ├─ 工部尚书 → 编码
                                                     │   ├─ 刑部尚书 → 测试验证
                                                     │   ├─ 礼部尚书 → 规范检查
                                                     │   └─ 户部 → 记录归档
                                                     └─ 制司 → 独立权限
```

### Message Flow

1. User sends text → `send_message` Tauri command → `ActorSystem` routes to 内阁
2. 内阁 uses LLM + `<skill>` system to decide workflow → `route_to` other departments
3. Each department is a `tokio::spawn` actor with an `mpsc::UnboundedReceiver` mailbox
4. Actors execute tool loops → emit results via `emperor_tx` (→ frontend `chat-message` events)
5. `dept_log_tx` → frontend `dept-log` events (DeptStatusPanel)
6. `milestone_tx` → persists project state milestones to `.shuji/state.json`

### Prompt Architecture

Layered prompt injection, ordered as sent to API:

```
1. base_prompt (prompt.md)         — role definition, department table, skill reference
2. skill_prompts (Vec<String>)     — active skill content, injected via Session::new()
3. history (context_messages)      — talk_history (emperor ↔ cabinet conversation)
4. user_message                    — current input
```

- **内阁**: `skill_prompts` injected from `ActorContext.current_skill` (persists across turns). Switches via `<skill>name</skill>` output tag detected in `neige/mod.rs` loop.
- **中书令**: Self-managed skills — detects `<skill>` tag in its own `mod.rs` loop, calls `session.replace_skill()` directly (not via `ActorContext`). Has its own skill set: `overall_design`, `phase_plan`, `phase_design`.
- Other departments get `skill_prompts: &[]` — no skill system.

### Session / AgentController Split

- **Session** (`api/session.rs`): Pure LLM layer — owns message history, one `step()` = one API round-trip, auto-retries on `finish_reason=length` (halving max_tokens each retry)
- **AgentController** (`api/control.rs`): Drive loop — calls `session.step()`, executes tools, feeds results back, handles cancel/interrupt/restart, watchdog diagnostics (same-tool repetition, read-without-write, consecutive errors)

## Document-Centric Architecture

Departments communicate via **documents** under `.shuji/`, not via route_to semantics. The `route_to` `subject` is just a document ID — the receiver reads the document to understand what to do.

### Document Types & Directories

| Type | Prefix | Directory | Status Machine |
|------|--------|-----------|----------------|
| design | `dsgn` | `.shuji/designs/` | draft → approved → closed |
| plan | `plan` | `.shuji/designs/` | draft → approved → closed |
| phase_design | `pdsg` | `.shuji/designs/` | draft → approved → closed |
| detailed_design | `ddtl` | `.shuji/designs/detail/` | todo → done |
| review | `revw` | `.shuji/reviews/` | todo → done |
| task | `task` | `.shuji/tasks/` | todo → done |
| contract | `ctrt` | `.shuji/contracts/` | todo → done |
| report | `rprt` | `.shuji/reports/` | todo → done |

### YAML Frontmatter Format

```yaml
---
id: dsgn_003          # auto-assigned via .shuji/_counter
type: dsgn
status: draft         # program-validated state machine
author: 中书令         # mapped from dept name
timestamp: 2026-05-12T14:30:00
refs: [1, 3]         # referenced doc IDs (integers, no prefix)
---
content body...
```

### Document Tools

- **`create_document(type, refs)`** — auto-assigns ID, writes to `.shuji/{dir}/{type}_{id}.md`, returns doc ID
- **`update_document(id, status?, content?, append?)`** — updates status/content, validates state transitions

## 内阁 Skill System

内阁 uses `<skill>name</skill>` to dynamically switch working modes:

| Skill | Purpose |
|-------|---------|
| `clarify` | Ask emperor questions to understand requirements |
| `workflow_demo` | Single file, zero deps — route directly to 工部尚书 |
| `workflow_simple` | Multiple files, straightforward — route to 尚书令 |
| `workflow_standard` | New business logic — design → review → approval → execution |
| `workflow_complex` | Multi-stage, multi-module — full pipeline |
| `discuss` | Free chat mode, no tools |
| `summary` | Summarize work done, produce completion report |

Skills are loaded via `NeigeAgent::load_skill(name)`, injected into session as `[skill: name]\n{content}` system messages, and accumulate in context (future: context compression).

## Project Structure

```
shuji-app/
├── src/                              # Frontend (React + Vite + Tailwind)
│   ├── pages/
│   │   ├── WorkspaceSelect.tsx       # Project selection / creation
│   │   ├── ProjectDashboard.tsx      # Main chat UI + dashboard
│   │   └── LogsPage.tsx              # Department logs viewer
│   └── components/
│       ├── ChatBubble.tsx            # Message bubble with <options>
│       ├── ChatInput.tsx             # Input box
│       ├── DeptStatusPanel.tsx        # Real-time department status
│       └── WorkflowTimeline.tsx      # Progress visualization
├── assets/defaults/zuxun.md          # Default organizational rules
└── src-tauri/src/                    # Backend (Rust + Tauri v2)
    ├── commands/
    │   ├── project.rs                # create/load/get/list/delete project
    │   ├── workflow.rs               # send_message → actor system entry
    │   └── settings.rs               # .env config loader
    ├── actor/mod.rs                  # Actor system: run_actor, ActorContext, ActorSystem
    ├── agent/
    │   ├── trait.rs                  # Agent trait (AgentInput/Output, LoopDecision)
    │   ├── util.rs                   # Shared helpers (extract_tag, etc.)
    │   ├── neige/                    # 内阁 — skill-based workflow dispatcher
    │   │   ├── mod.rs                # Skill detection loop, inject_skill
    │   │   ├── prompt.md             # Base prompt (English)
    │   │   └── skills/               # Skill definitions (7 skills)
    │   ├── zhongshuling/             # 中书令 — design (3 skills: overall_design, phase_plan, phase_design)
    │   ├── menxiashizhong/           # 门下侍中 — design review (merged: was 侍中 + 给事中)
    │   ├── shangshuling/             # 尚书令 — execution dispatch
    │   ├── libushangshu/             # 吏部尚书 — detailed design
    │   ├── bingbushangshu/           # 兵部尚书 — tests + contracts
    │   ├── gongbushangshu/           # 工部尚书 — production code
    │   ├── xingbushangshu/           # 刑部尚书 — test verification
    │   ├── liburshangshu/            # 礼部尚书 — standards check
    │   └── zhisi/                    # 制司 — independent investigation
    ├── api/
    │   ├── client.rs                 # AnthropicClient (dual-format HTTP)
    │   ├── session.rs                # LLM session: step(), auto-retry, inject_skill()
    │   └── control.rs                # AgentController: tool loop, cancel/watchdog
    ├── tool/
    │   ├── mod.rs                    # Unified tool dispatch (read/write/edit/delete/rename/append/execute)
    │   └── documents.rs              # create_document + update_document (YAML frontmatter, ID counter)
    ├── models/
    │   ├── role.rs                   # Role enum (13 departments)
    │   ├── chat.rs                   # ChatMessage + ChatOption / approval_options()
    │   ├── message.rs                # Message struct
    │   ├── document.rs               # Document struct (legacy)
    │   └── project.rs                # Project + phase state enums
    ├── storage/shuji_dir.rs          # .shuji/ filesystem abstraction
    ├── logging/logger.rs             # Department-scoped JSONL logging + CONSOLE_LOCK
    └── token_tracker.rs              # Token usage aggregation + persistence
```

## Key Technical Decisions

### API Dual-Format
- URL contains `anthropic.com` → Anthropic Messages API (with `x-api-key` header)
- Otherwise → OpenAI Chat Completions API (with `Bearer` auth)
- Same `AnthropicClient` struct, auto-detected per request

### Tool Dispatch
All agents call `tool::execute_named_tool(name, args, working_dir, dept)` instead of writing their own match blocks. Central dispatch in `tool/mod.rs`. Tools return structured JSON (`ToolOutput { ok, operation, path, message, error_code }`).

### Project State
- `Project.talk`: Append-only conversation log, auto-trims to ~12 entries (oldest compressed to summary)
- `Project.task`: Milestones (append-only, never trimmed)
- `Project.summary`: Compact one-line status (auto-updated via milestone_tx)
- Persisted to `.shuji/state.json` on every milestone event

### Cancel Mechanism
- `AtomicBool` flag shared across all actors via `AppState.cancel_flag`
- Checked at the top of each `AgentController.run()` iteration
- Sets flag → interrupts current session → saves snapshot → responds to emperor

### Session Limits
| Setting | Value | Agent |
|---------|-------|-------|
| write_file agents max_tokens | 2048 | 兵部、工部 |
| append_document agents max_tokens | 1536 | 中书令、吏部、刑部 |
| read-only agents max_tokens | 1024 | 礼部 |
| text-only agents max_tokens | 512 | (未使用) |
| **Tool iterations** | | |
| write_file tool iterations | 120 | 兵部、工部 |
| append_document tool iterations | 100 | 中书令、吏部、刑部 |
| read-only tool iterations | 80 | 礼部 |
| finish_reason=length retries | 5 (halving max_tokens each time) | 所有 |
| Consecutive tool errors | 5 → auto-stop | 所有 |
| Max plan loop iterations | 6 (工部尚书 only) | 工部 |
| **Tool argument limits** | | |
| append_document content | 500 chars per call | 中书令、吏部、刑部 |
| modify_document text | 400 chars per parameter | 中书令、吏部 |
| create_file content | 500 chars per call | 兵部、工部 |
| append_file content | 500 chars per call | 兵部、工部 |
| modify_file text | 500 chars per parameter | 兵部、工部 |

### Edge Cases Handled
- **Truncated tool calls**: Assistant message is filtered to only include valid `tool_call_id`s before pushing to history (prevents 400 error)
- **All tool calls broken**: Returns `StepResult::Text` instead of empty `ToolCalls` (prevents infinite loop)
- **Windows CRLF**: `log_console!` uses `write!` with explicit `\n` instead of `eprintln!` to avoid pipe corruption
- **Skill loop dedup**: If 内阁 outputs same `<skill>` tag twice, code breaks the loop (prevents infinite skill reload)
- **Self-routing prevention**: Base prompt explicitly forbids `route_to(to="内阁")`

## Interactive Mode

- 决策 tab: user types → `send_message` → actor system processes → results emitted as `chat-message` events
- 讨论 tab: user types → `discuss_with_cabinet` → standalone 内阁 LLM call (no project state modification)
- `<options>` in agent output → rendered as clickable buttons (A/B/C) in ChatBubble
- Cancel button → sets `cancel_flag` → actors stop at next check point
- Dashboard sidebar → token usage stats by role (今日/近3天/近7天/汇总)
- Logs page `/logs` → department-scoped JSONL files in `.shuji/logs/`

## Project Status

PoC / prototype phase. Core actor system + collaboration flow works end-to-end with the frontend. No automated test suite yet — all verification is manual. When adding tests, prefer integration tests that exercise full agent loops rather than unit tests on individual modules.
