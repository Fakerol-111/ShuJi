//! Playbook: structured failure-handling guides injected into agent context.
//!
//! Each playbook is a Markdown file that provides step-by-step guidance
//! for common failure scenarios encountered during pipeline execution.

/// Context passed to playbook selection for filtering.
pub struct PlaybookContext {
    pub event: String,
    pub step_id: Option<String>,
    pub exit_code: Option<i32>,
}

/// Get the playbook content for a given event.
/// Returns `None` if no playbook matches.
pub fn playbook_for_event(event: &str, _context: &PlaybookContext) -> Option<String> {
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

/// Get a short hint (≤200 chars) for watchdog injection.
pub fn playbook_hint(event: &str) -> Option<String> {
    let hint = match event {
        "test-red" | "test_stalemate" => "[playbook: test-red] 测试连续失败，按 playbook 排查",
        "contract-mismatch" | "contract_diff" => "[playbook: contract-mismatch] 契约与实现不一致",
        "lint-fail" | "lint" => "[playbook: lint-fail] lint 未通过",
        "pipeline-deadlock" | "deadlock" => "[playbook: pipeline-deadlock] Pipeline 死锁",
        _ => return None,
    };
    Some(hint.to_string())
}

/// List all available playbook names.
pub fn list_playbooks() -> Vec<&'static str> {
    vec![
        "test-red",
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
        assert_eq!(books.len(), 4);
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
}
