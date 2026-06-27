//! Playbook: structured failure-handling guides injected into agent context.
//!
//! Each playbook is a Markdown file that provides step-by-step guidance
//! for common failure scenarios encountered during pipeline execution.

/// Watchdog-detected loop or stall patterns (mapped to playbooks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogEvent {
    RepeatedTool,
    ReadWithoutWrite,
    ConsecutiveToolErrors,
    TestRedLoop,
    DeleteCreateCycle,
}

impl WatchdogEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RepeatedTool => "repeated_tool",
            Self::ReadWithoutWrite => "read_without_write",
            Self::ConsecutiveToolErrors => "consecutive_tool_errors",
            Self::TestRedLoop => "test_red_loop",
            Self::DeleteCreateCycle => "delete_create_cycle",
        }
    }

    pub fn playbook_key(self) -> &'static str {
        match self {
            Self::RepeatedTool => "repeated-tool",
            Self::ReadWithoutWrite => "read-without-write",
            Self::ConsecutiveToolErrors => "consecutive-tool-errors",
            Self::TestRedLoop => "test-red",
            Self::DeleteCreateCycle => "delete-create-cycle",
        }
    }
}

/// Optional context for formatting a short watchdog recovery hint.
pub struct WatchdogHintContext<'a> {
    pub tool: Option<&'a str>,
    pub count: u32,
    pub path: Option<&'a str>,
    pub detail: Option<&'a str>,
}

impl<'a> WatchdogHintContext<'a> {
    pub fn new(count: u32) -> Self {
        Self {
            tool: None,
            count,
            path: None,
            detail: None,
        }
    }

    pub fn tool(mut self, tool: &'a str) -> Self {
        self.tool = Some(tool);
        self
    }

    pub fn path(mut self, path: &'a str) -> Self {
        self.path = Some(path);
        self
    }

    pub fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }
}

/// Context passed to playbook selection for filtering.
pub struct PlaybookContext {
    pub event: String,
    pub step_id: Option<String>,
    pub exit_code: Option<i32>,
}

/// Get the playbook content for a given event.
/// Returns `None` if no playbook matches.
pub fn playbook_for_event(event: &str, _context: &PlaybookContext) -> Option<String> {
    watchdog_event_from_str(event)
        .map(|e| playbook_for_watchdog(e).to_string())
        .or_else(|| legacy_playbook_for_event(event))
}

fn legacy_playbook_for_event(event: &str) -> Option<String> {
    match event {
        "test-red" | "test_stalemate" => {
            Some(include_str!("../../assets/playbooks/test-red.md").to_string())
        }
        "contract-mismatch" | "contract_diff" => {
            Some(include_str!("../../assets/playbooks/contract-mismatch.md").to_string())
        }
        "lint-fail" | "lint" => {
            Some(include_str!("../../assets/playbooks/lint-fail.md").to_string())
        }
        "pipeline-deadlock" | "deadlock" => {
            Some(include_str!("../../assets/playbooks/pipeline-deadlock.md").to_string())
        }
        _ => None,
    }
}

pub fn playbook_for_watchdog(event: WatchdogEvent) -> &'static str {
    match event {
        WatchdogEvent::RepeatedTool => {
            include_str!("../../assets/playbooks/repeated-tool.md")
        }
        WatchdogEvent::ReadWithoutWrite => {
            include_str!("../../assets/playbooks/read-without-write.md")
        }
        WatchdogEvent::ConsecutiveToolErrors => {
            include_str!("../../assets/playbooks/consecutive-tool-errors.md")
        }
        WatchdogEvent::TestRedLoop => include_str!("../../assets/playbooks/test-red.md"),
        WatchdogEvent::DeleteCreateCycle => {
            include_str!("../../assets/playbooks/delete-create-cycle.md")
        }
    }
}

pub fn watchdog_event_from_str(event: &str) -> Option<WatchdogEvent> {
    match event {
        "repeated_tool" | "repeated-tool" | "RepeatedTool" => Some(WatchdogEvent::RepeatedTool),
        "read_without_write" | "read-without-write" | "ReadWithoutWrite" => {
            Some(WatchdogEvent::ReadWithoutWrite)
        }
        "consecutive_tool_errors" | "consecutive-tool-errors" | "ConsecutiveToolErrors" => {
            Some(WatchdogEvent::ConsecutiveToolErrors)
        }
        "test_red_loop" | "test-red" | "test_stalemate" | "TestRedLoop" => {
            Some(WatchdogEvent::TestRedLoop)
        }
        "delete_create_cycle" | "delete-create-cycle" | "DeleteCreateCycle" => {
            Some(WatchdogEvent::DeleteCreateCycle)
        }
        _ => None,
    }
}

/// Short actionable hint (≤200 chars) for watchdog injection into tool results.
pub fn watchdog_playbook_hint(event: WatchdogEvent, ctx: &WatchdogHintContext<'_>) -> String {
    let key = event.playbook_key();
    let hint = match event {
        WatchdogEvent::RepeatedTool => {
            let tool = ctx.tool.unwrap_or("tool");
            format!(
                "[playbook: {key}] `{tool}` 重复 {n} 次：先总结已知事实，换工具/参数或缩小范围，勿再相同调用。",
                n = ctx.count + 1
            )
        }
        WatchdogEvent::ReadWithoutWrite => format!(
            "[playbook: {key}] 连续 {n} 次只读未写：总结结论后立即 create/edit/append，或换 read_document 来源。",
            n = ctx.count + 1
        ),
        WatchdogEvent::ConsecutiveToolErrors => {
            let detail = ctx.detail.unwrap_or("多个工具失败");
            format!(
                "[playbook: {key}] {detail}（共 {n} 次）：read_file 验证状态，换更小粒度操作，勿盲重试。",
                n = ctx.count
            )
        }
        WatchdogEvent::TestRedLoop => format!(
            "[playbook: {key}] run_tests 连续失败 {n} 次：读首条失败、单测隔离、对照契约；仍红则 wake_cabinet。",
            n = ctx.count
        ),
        WatchdogEvent::DeleteCreateCycle => {
            let path = ctx.path.unwrap_or("file");
            format!(
                "[playbook: {key}] `{path}` 删建循环 {n} 次：改用 edit_file/apply_patch，勿 delete+create。",
                n = ctx.count
            )
        }
    };
    truncate_hint(hint, 200)
}

/// Get a short hint (≤200 chars) for watchdog injection.
pub fn playbook_hint(event: &str) -> Option<String> {
    if let Some(wd) = watchdog_event_from_str(event) {
        return Some(watchdog_playbook_hint(wd, &WatchdogHintContext::new(0)));
    }
    let hint = match event {
        "contract-mismatch" | "contract_diff" => {
            "[playbook: contract-mismatch] 契约与实现不一致，对照签名后修正"
        }
        "lint-fail" | "lint" => "[playbook: lint-fail] lint 未通过，按 playbook 逐项修复",
        "pipeline-deadlock" | "deadlock" => {
            "[playbook: pipeline-deadlock] Pipeline 死锁，检查 approval 与步骤依赖"
        }
        _ => return None,
    };
    Some(truncate_hint(hint.to_string(), 200))
}

fn truncate_hint(mut hint: String, max_len: usize) -> String {
    if hint.len() <= max_len {
        return hint;
    }
    let end = hint.floor_char_boundary(max_len.saturating_sub(1));
    hint.truncate(end);
    hint.push('…');
    hint
}

/// Append a watchdog playbook hint to tool result content.
pub fn append_watchdog_intervention(
    tool_content: &mut String,
    event: WatchdogEvent,
    ctx: &WatchdogHintContext<'_>,
) {
    let hint = watchdog_playbook_hint(event, ctx);
    tool_content.push_str(&format!("\n\n[Intervention] {}", hint));
}

/// List all available playbook names.
pub fn list_playbooks() -> Vec<&'static str> {
    vec![
        "repeated-tool",
        "read-without-write",
        "consecutive-tool-errors",
        "test-red",
        "delete-create-cycle",
        "contract-mismatch",
        "lint-fail",
        "pipeline-deadlock",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playbook_for_test_red() {
        let ctx = PlaybookContext {
            event: "test_stalemate".into(),
            step_id: None,
            exit_code: Some(1),
        };
        let content = playbook_for_event("test-red", &ctx);
        assert!(content.is_some());
        assert!(content.unwrap().contains("测试僵局"));
    }

    #[test]
    fn test_playbook_for_unknown_event() {
        let ctx = PlaybookContext {
            event: "unknown".into(),
            step_id: None,
            exit_code: None,
        };
        assert!(playbook_for_event("bogus", &ctx).is_none());
    }

    #[test]
    fn test_playbook_hint_length() {
        let hint = playbook_hint("test-red").unwrap();
        assert!(hint.len() <= 200, "hint too long: {}", hint.len());
        assert!(hint.contains("[playbook:"));
    }

    #[test]
    fn test_list_playbooks() {
        let books = list_playbooks();
        assert!(books.len() >= 8);
        assert!(books.contains(&"repeated-tool"));
    }

    #[test]
    fn test_playbook_for_contract_mismatch() {
        let ctx = PlaybookContext {
            event: "contract_diff".into(),
            step_id: None,
            exit_code: None,
        };
        let content = playbook_for_event("contract-mismatch", &ctx);
        assert!(content.is_some());
        assert!(content.unwrap().contains("契约"));
    }

    #[test]
    fn test_watchdog_hints_under_200_chars() {
        for event in [
            WatchdogEvent::RepeatedTool,
            WatchdogEvent::ReadWithoutWrite,
            WatchdogEvent::ConsecutiveToolErrors,
            WatchdogEvent::TestRedLoop,
            WatchdogEvent::DeleteCreateCycle,
        ] {
            let ctx = WatchdogHintContext::new(3)
                .tool("read_file")
                .path("src/main.rs")
                .detail("read_filex2, create_filex1");
            let hint = watchdog_playbook_hint(event, &ctx);
            assert!(
                hint.len() <= 200,
                "{:?} hint too long ({}): {}",
                event,
                hint.len(),
                hint
            );
            assert!(hint.contains("[playbook:"));
        }
    }

    #[test]
    fn test_watchdog_playbook_files_load() {
        for event in [
            WatchdogEvent::RepeatedTool,
            WatchdogEvent::ReadWithoutWrite,
            WatchdogEvent::ConsecutiveToolErrors,
            WatchdogEvent::TestRedLoop,
            WatchdogEvent::DeleteCreateCycle,
        ] {
            let body = playbook_for_watchdog(event);
            assert!(!body.is_empty(), "{:?} playbook empty", event);
            assert!(body.contains("恢复步骤") || body.contains("排查步骤"));
        }
    }

    #[test]
    fn test_append_watchdog_intervention_format() {
        let mut content = r#"{"ok":true}"#.to_string();
        append_watchdog_intervention(
            &mut content,
            WatchdogEvent::RepeatedTool,
            &WatchdogHintContext::new(2).tool("read_file"),
        );
        assert!(content.contains("[Intervention]"));
        assert!(content.contains("[playbook: repeated-tool]"));
        assert!(content.contains("read_file"));
    }
}
