# 枢机（ShuJi）项目改进路线

> 基于代码审查生成的改进建议。按优先级排列，每项附带 1-2 个可行方案。

---

## P0 — 核心可靠性（不做则项目不可用）

### 1. 建立测试基础设施

**问题**：整个项目零测试。11 个 Agent 协作、LLM 输出解析、文件工具系统、YAML 文档生成——全是复杂的逻辑，没有任何自动化验证。

**方案 A — 从后端的工具系统开始（推荐）**
`tool/mod.rs` 的 `resolve_scoped_path` 和底层文件读写函数是纯函数，最适合先写单元测试。

```rust
// 在 tool/mod.rs 底部或 tests/ 目录中
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_rejects_absolute_path() {
        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_scoped_path(tmp.path(), "/etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("绝对路径"));
    }

    #[test]
    fn test_rejects_dotdot() {
        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_scoped_path(tmp.path(), "../other");
        assert!(result.is_err());
    }

    #[test]
    fn test_accepts_normal_relative() {
        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_scoped_path(tmp.path(), "foo/bar.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_rejects_filename_with_dotdot() {
        // 已知 bug：.. 检查会误伤合法文件名
        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_scoped_path(tmp.path(), "some..file.txt");
        // 当前行为是 Err，改进后应为 Ok
    }
}
```

添加到 `Cargo.toml`：
```toml
[dev-dependencies]
tempfile = "3"
```

运行：`cd src-tauri && cargo test`

**方案 B — 前端加 vitest**
```bash
npm install -D vitest @testing-library/react @testing-library/jest-dom
```
先在 `api.ts` 和 `types.ts` 这种纯逻辑文件上写测试，再逐步覆盖组件渲染测试。

---

### 2. 修复异步上下文中的同步 I/O

**问题**：`actor/mod.rs:286-296` 和 `session.rs:614-724` 在 tokio async 上下文中使用 `std::fs`，阻塞 runtime 线程。

**方案 — 改用 `tokio::fs`**

```rust
// actor/mod.rs:286 处，改前：
let _ = std::fs::write(&state_path, serde_json::to_string_pretty(&proj).unwrap_or_default());

// 改后：
let content = serde_json::to_string_pretty(&proj).unwrap_or_default();
let _ = tokio::fs::write(&state_path, &content).await;
```

同样的改造应用到 `session.rs` 的 `write_debug_truncated`（需要把这个函数改成 async fn，或在调用处用 `tokio::task::spawn_blocking` 包裹）。优先用 `tokio::fs`，因为读写量小、无复杂计算，`spawn_blocking` 的开销反而更大。

---

### 3. 修复 `block_in_place` + `block_on` 潜在死锁

**问题**：`neige/mod.rs:227-231` 在同步闭包里调用了 `block_in_place` + `block_on` 来跑 async 的 `expand_requirements::run`，在 tokio 线程数有限时可能死锁。

**方案 — 让调用处的 exec 闭包支持 async**

```rust
// agent/trait.rs — 修改 Agent trait
pub trait Agent: Send + Sync {
    // 改前：
    // fn execute(&mut self, ...) -> Result<...>;
    
    // 改后：
    fn execute<'a>(&'a mut self, ...) -> BoxFuture<'a, Result<...>>;
}
```

**或者短期 hack（风险低但难看）**：在当前同步闭包外不做 `block_on`，而是缓存 `expand_requirements` 的结果并异步预加载：

```rust
// neige/mod.rs — 在 ActorController::run 的循环开始前启动一个后台 task
// 让 expand_requirements 在 tokio runtime 上自然完成
let expand_handle = if needs_expand {
    Some(tokio::spawn(expand_requirements::run(...)))
} else {
    None
};
// ...然后在 exec 闭包里 await 这个 handle
```

---

## P1 — 结构性缺陷

### 4. 前端错误处理黑洞

**问题**：`ProjectDashboard.tsx` 中至少 6 处 `.catch(() => {})`，所有 API 错误被静默吞掉，用户完全不知道发生了什么。

**方案 A — 加统一错误处理层**
```typescript
// src/api.ts 或新建 src/error.ts
export function handleError(context: string): (err: unknown) => void {
  return (err: unknown) => {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`[${context}]`, err);
    // 发射到 UI（通过 Tauri event 或回调）
    // 最简单的方式：用 dispatchEvent 或全局状态
    window.dispatchEvent(new CustomEvent('app:error', { detail: `${context}: ${msg}` }));
  };
}

// 使用
getConfig().catch(handleError('加载配置'));
```

**方案 B — 在 Tauri 命令层统一处理**
Rust 端的命令函数已经返回 `Result`，可以在前端 `invoke` 时做一个 wrapper：

```typescript
// src/api.ts
async function safeInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T | null> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    console.error(`[invoke] ${command} 失败:`, err);
    // 可选：toast 通知
    return null;
  }
}
```

---

### 5. 拆分过重的 ProjectDashboard.tsx

**问题**：17 个 `useState`、多组无关的 `useEffect`，一个组件做了项目加载、聊天、仪表盘、设置弹窗、token 统计所有事情。

**方案 — 按职责拆分为 hooks + 子组件**

```typescript
// src/hooks/useProject.ts — 项目加载/切换逻辑
export function useProject() { ... }

// src/hooks/useChat.ts — 聊天消息管理
export function useChat() { ... }

// src/hooks/useTokenStats.ts — Token 统计
export function useTokenStats() { ... }

// src/components/SettingsModal.tsx — 设置弹窗（独立组件）
// src/components/TokenDashboard.tsx — Token 统计面板（独立组件）
```

`ProjectDashboard.tsx` 从 550 行缩减到约 100 行，降低心智负担和 bug 概率。

---

### 6. 工具系统脆皮字符串匹配

**问题**：
- `control.rs:205`：`tc.name.contains("write")` — 任何名字带 "write" 的工具都会被错误分类
- `tool/mod.rs:33`：`rel.contains("..")` — 文件名含 `..` 就被拒绝（如 `some..file.txt`）
- `tool/mod.rs:39`：只拦截了 `c:`、`d:`、`e:` 盘符，没覆盖 `f:` 等

**方案 — 用显式匹配和正则替换**

```rust
// control.rs — 用 match 而非 contains
let is_write_tool = matches!(tc.name.as_str(),
    "create_file" | "write_file" | "edit_file" | "append_file" | "modify_file"
);

// tool/mod.rs — 用 Path 组件检查 .. 而不是字符串 contains
fn has_dotdot(path: &str) -> bool {
    Path::new(path).components().any(|c| c == std::path::Component::ParentDir)
}

// tool/mod.rs — 用正则或循环检查任意盘符
fn has_drive_letter(path: &str) -> bool {
    if cfg!(windows) {
        let lower = path.to_lowercase();
        // 匹配 a:-z: 盘符
        lower.len() >= 2
            && lower.as_bytes()[1] == b':'
            && lower.as_bytes()[0].is_ascii_alphabetic()
    } else {
        false
    }
}
```

---

## P2 — 架构与功能完善

### 7. Agent 协作的稳健性

**问题**：LLM 输出 `<route>` 和 `<skill>` 标签的格式一致性是整条链路的命脉，但目前全靠 prompt 约束，没有格式校验或 fallback。

**方案 A — 解析后校验 + 兜底**
```rust
// agent/util.rs
pub fn extract_route(text: &str) -> Option<(Role, String)> {
    // 先用正则匹配严格格式
    let re = Regex::new(r#"<route\s+to="([^"]+)"\s+subject="([^"]*)"\s*/>"#).ok()?;
    if let Some(caps) = re.captures(text) {
        let role_name = caps.get(1)?.as_str();
        let subject = caps.get(2)?.as_str().to_string();
        if let Some(role) = Role::from_name(role_name) {
            return Some((role, subject));
        }
    }
    // fallback: 尝试非严格匹配（给 LLM 一次容错）
    // ...
    None
}
```

**方案 B — 增加 Agent 输出重试**
在 `AgentController` 的循环中，如果某次工具调用的返回不包含预期格式，让 Agent 重试该步骤而非直接退出。`control.rs` 已经有重试逻辑骨架，可在此基础上扩展。

---

### 8. 检查单次内容 500 字符限制是否够用

**问题**：工部写代码每次只能 500 字符（约 10-20 行），对真实项目来说太短了，导致生成零散、上下文碎片化。

**方案**
```toml
# config.toml — 分层次放宽
[content_limits]
create_file = 2000    # 低频操作，给更多空间
append_file = 2000
modify_file = { old_text = 500, new_text = 1000 }  # 修改可以给更大
```

但放宽 token 限制意味着更长上下文、更高成本。可以按文件扩展名差异化调整：`.md` 文件保持 500，`.rs`/`.ts` 代码文件给到 2000。

---

### 9. 前端类型安全改进

**方案清单**（小改动但收益明显）：

| 问题 | 改法 |
|------|------|
| ChatBubble.tsx:55 non-null assertion | 改为 `const opt = options.find(...); if (!opt) return null;` |
| ChatBubble.tsx:91 硬编码 `"补充"` check | 后端 `ChatOption` 加 `requires_supplement: bool` 字段 |
| 多处 `key={i}` | 用 `msg.timestamp + msg.role` 或 `msg.id` 替代 |
| main.tsx:9 非空断言 | 改为 `const root = document.getElementById('root'); if (!root) throw new Error('root not found');` |
| 重复定义：`TokenUsage` 在 api.ts | 挪到 `types.ts`，统一管理 |

---

### 10. Windows 兼容性

**问题**：
- `tool/mod.rs:414` 写死 `bash -l -c`，Windows 上不存在
- 路径检查只拦截 C/D/E 盘符

**方案**
```rust
// tool/mod.rs — 命令执行
let shell = if cfg!(windows) {
    ("cmd", "/C")
} else {
    ("bash", "-lc")
};

// 盘符检查改用通用方式
fn check_drive_letter(rel: &str) -> bool {
    if !cfg!(windows) { return false; }
    let lower = rel.to_lowercase();
    lower.len() >= 2 && lower.as_bytes()[1] == b':' && lower.as_bytes()[0].is_ascii_alphabetic()
        && rel[..2].parse::<String>().map_or(false, |_| true)  // 确保是 drive: 格式
}
```

---

### 11. CLAUDE.md 与 MEMORY.md 合并

**问题**：目前两份文件内容重叠但分开维护。

**方案**
将 `MEMORY.md` 的代码质量问题和改进路线合并到 `CLAUDE.md` 末尾的 "Known Issues" 部分，或引用方式：
```markdown
## 代码质量已知问题
> 详见 [MEMORY.md](./MEMORY.md) 和 [ROADMAP.md](./ROADMAP.md)
```
这样每次启动会话时 system prompt 只读 `CLAUDE.md`，但指明扩展文档位置。

---

## 优先级总结

| 优先级 | 项 | 预估工作量 | 影响面 |
|--------|----|-----------|--------|
| **P0** | 测试基础设施 | 2-3 天 | 核心可靠性 |
| **P0** | 修复异步 I/O 阻塞 | 0.5 天 | 稳定性 |
| **P0** | 修复 `block_on` 死锁风险 | 0.5 天 | 稳定性 |
| **P1** | 前端错误处理 | 0.5 天 | 用户体验 |
| **P1** | 拆分 ProjectDashboard | 1-2 天 | 可维护性 |
| **P1** | 脆皮字符串匹配 | 0.5 天 | 安全性/正确性 |
| **P2** | Agent 协作稳健性 | 2-3 天 | 系统可靠性 |
| **P2** | 放宽内容限制 | 0.5 天 | 代码生成质量 |
| **P2** | 前端类型安全 | 1 天 | 代码质量 |
| **P2** | Windows 兼容性 | 1 天 | 平台覆盖 |
| **P2** | 文档合并 | 0.5 天 | 可维护性 |

> 如果时间有限，**P0 三项做完就能解决最危险的稳定性问题**，让项目站得更稳。
> P1 解决后代码可维护性会上一个大台阶。
> P2 是打磨和完善，按需投入即可。
