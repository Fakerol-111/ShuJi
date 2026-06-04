//! Workflow routing heuristic for 内阁.
//!
//! Pure-function text analysis that maps user task descriptions to workflow
//! skills. No I/O — only keyword/phrase matching. Designed for easy unit
//! testing without mocks.
//!
//! # Priority
//!
//! 1. **Explicit skill mention** → High confidence (user knows what they want)
//! 2. **Keyword matching** → Medium confidence (bug → bugfix, 系统 → complex)
//! 3. **Fallback** → Low confidence (LLM should use `<options>` or decide)

/// How strongly the router recommends a particular workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    /// User explicitly named a skill — this is authoritative.
    High,
    /// Keyword heuristic matched — likely correct, but LLM may override.
    Medium,
    /// No clear signal — LLM should present options to the emperor.
    Low,
}

/// A workflow skill suggestion produced by the router.
#[derive(Debug, Clone)]
pub struct RoutingSuggestion {
    /// The skill name, e.g. `"workflow_bugfix"`.
    pub skill: &'static str,
    pub confidence: Confidence,
}

/// Analyze `task` and return the best workflow skill suggestion, if any.
///
/// Returns `None` for empty or very short inputs where no signal exists.
pub fn suggest_workflow(task: &str) -> Option<RoutingSuggestion> {
    let task = task.trim();
    if task.is_empty() || task.len() < 3 {
        return None;
    }

    // ── 1. Explicit skill mention (highest priority) ──
    if let Some(skill) = detect_explicit_skill(task) {
        return Some(RoutingSuggestion {
            skill,
            confidence: Confidence::High,
        });
    }

    let lower = task.to_lowercase();

    // ── 2. Intent-specific keyword patterns ──

    // Bugfix: bug report, crash, error, test failure
    if contains_any(
        &lower,
        &[
            "bug",
            "fix",
            "broken",
            "crash",
            "error",
            "not working",
            "测试失败",
            "出错",
            "修复",
            "故障",
            "异常",
        ],
    ) {
        return Some(RoutingSuggestion {
            skill: "workflow_bugfix",
            confidence: Confidence::Medium,
        });
    }

    // Refactor: structural change
    if contains_any(
        &lower,
        &[
            "refactor",
            "重构",
            "重写",
            "restructure",
            "redesign",
            "架构调整",
        ],
    ) {
        return Some(RoutingSuggestion {
            skill: "workflow_refactor",
            confidence: Confidence::Medium,
        });
    }

    // Optimize: performance tuning
    if contains_any(
        &lower,
        &[
            "optimize",
            "优化",
            "性能",
            "slow",
            "慢",
            "加速",
            "performance",
            "提速",
            "现有代码",
            "存量代码",
            "读 repo",
            "读仓库",
            "改现有",
            "改代码",
        ],
    ) {
        return Some(RoutingSuggestion {
            skill: "workflow_optimize",
            confidence: Confidence::Medium,
        });
    }

    // Audit: security, compliance, standards check
    if contains_any(
        &lower,
        &[
            "audit",
            "审计",
            "安全",
            "security",
            "合规",
            "compliance",
            "审查",
            "安全检查",
        ],
    ) {
        return Some(RoutingSuggestion {
            skill: "workflow_audit",
            confidence: Confidence::Medium,
        });
    }

    // ── 3. Complexity-based classification ──

    // Demo: tiny, single-purpose, calculator-level
    let is_demo = contains_any(
        &lower,
        &[
            "demo",
            "calc",
            "calculator",
            "hello",
            "greeting",
            "测试项目",
            "try",
            "体验",
        ],
    );

    // Complex keywords → workflow_complex
    let has_complex = contains_any(
        &lower,
        &[
            "系统",
            "平台",
            "erp",
            "架构设计",
            "微服务",
            "多模块",
            "整体方案",
            "multi-stage",
            "enterprise",
            "企业级",
            "全平台",
        ],
    );

    // Simple indicators → workflow_simple
    let has_simple = contains_any(
        &lower,
        &[
            "简单",
            "小功能",
            "minor",
            "tiny change",
            "单文件",
            "加一个",
            "添加一个",
        ],
    );

    if is_demo {
        return Some(RoutingSuggestion {
            skill: "workflow_demo",
            confidence: Confidence::Medium,
        });
    }

    if has_complex {
        return Some(RoutingSuggestion {
            skill: "workflow_complex",
            confidence: Confidence::Medium,
        });
    }

    if has_simple {
        return Some(RoutingSuggestion {
            skill: "workflow_simple",
            confidence: Confidence::Medium,
        });
    }

    // ── 4. Fallback: no clear signal → standard workflow, low confidence ──
    Some(RoutingSuggestion {
        skill: "workflow_standard",
        confidence: Confidence::Low,
    })
}

/// Check if `text` contains any of the given substrings.
fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|&k| text.contains(k))
}

/// Detect explicit workflow skill mentions in the task text.
///
/// Matches patterns like `workflow_simple`, `<skill>workflow_bugfix</skill>`,
/// or "用 workflow_complex".
fn detect_explicit_skill(task: &str) -> Option<&'static str> {
    const KNOWN: &[&str] = &[
        "workflow_demo",
        "workflow_simple",
        "workflow_standard",
        "workflow_complex",
        "workflow_bugfix",
        "workflow_refactor",
        "workflow_optimize",
        "workflow_audit",
    ];

    KNOWN.iter().find(|name| task.contains(*name)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Explicit skill ──

    #[test]
    fn test_explicit_skill_high_confidence() {
        let s = suggest_workflow("请用 workflow_simple 处理这个功能").unwrap();
        assert_eq!(s.skill, "workflow_simple");
        assert_eq!(s.confidence, Confidence::High);
    }

    #[test]
    fn test_explicit_skill_with_tag() {
        let s = suggest_workflow("<skill>workflow_complex</skill>").unwrap();
        assert_eq!(s.skill, "workflow_complex");
        assert_eq!(s.confidence, Confidence::High);
    }

    // ── Bugfix ──

    #[test]
    fn test_bugfix_by_fix_keyword() {
        let s = suggest_workflow("修复 calc.py 中的 bug").unwrap();
        assert_eq!(s.skill, "workflow_bugfix");
    }

    #[test]
    fn test_bugfix_crash_report() {
        let s = suggest_workflow("程序崩溃了，修复一下").unwrap();
        assert_eq!(s.skill, "workflow_bugfix");
    }

    #[test]
    fn test_bugfix_broken_test() {
        let s = suggest_workflow("test_calc.py 测试失败").unwrap();
        assert_eq!(s.skill, "workflow_bugfix");
    }

    // ── Complex ──

    #[test]
    fn test_complex_platform_keyword() {
        let s = suggest_workflow("做一个 ERP 管理系统").unwrap();
        assert_eq!(s.skill, "workflow_complex");
    }

    #[test]
    fn test_complex_architecture() {
        let s = suggest_workflow("微服务架构整体方案设计").unwrap();
        assert_eq!(s.skill, "workflow_complex");
    }

    // ── Demo ──

    #[test]
    fn test_demo_calculator() {
        let s = suggest_workflow("修复 calc.py 的 power 和 factorial 函数").unwrap();
        // "修复" + "calc" → bugfix takes priority over demo
        assert_eq!(s.skill, "workflow_bugfix");
    }

    #[test]
    fn test_demo_hello() {
        let s = suggest_workflow("创建一个 greeting.py 输出 Hello World").unwrap();
        // "hello" → demo, but no bug keywords, so can be demo
        assert_eq!(s.skill, "workflow_demo");
    }

    // ── Refactor ──

    #[test]
    fn test_refactor_restructure() {
        let s = suggest_workflow("重构用户模块的代码结构").unwrap();
        assert_eq!(s.skill, "workflow_refactor");
    }

    // ── Standard (low confidence fallback) ──

    #[test]
    fn test_brownfield_existing_code() {
        let s = suggest_workflow("优化现有代码的性能").unwrap();
        assert_eq!(s.skill, "workflow_optimize");
    }

    #[test]
    fn test_brownfield_read_repo() {
        let s = suggest_workflow("读 repo 分析瓶颈").unwrap();
        assert_eq!(s.skill, "workflow_optimize");
    }

    #[test]
    fn test_brownfield_modify_existing() {
        let s = suggest_workflow("改现有模块的代码").unwrap();
        assert_eq!(s.skill, "workflow_optimize");
    }

    #[test]
    fn test_standard_fallback() {
        let s = suggest_workflow("实现用户登录功能").unwrap();
        assert_eq!(s.skill, "workflow_standard");
        assert_eq!(s.confidence, Confidence::Low);
    }

    #[test]
    fn test_standard_new_feature() {
        let s = suggest_workflow("添加订单管理模块，支持创建和查询订单").unwrap();
        assert_eq!(s.skill, "workflow_standard");
        assert_eq!(s.confidence, Confidence::Low);
    }

    // ── Edge cases ──

    #[test]
    fn test_empty_input() {
        assert!(suggest_workflow("").is_none());
    }

    #[test]
    fn test_whitespace_input() {
        assert!(suggest_workflow("   ").is_none());
    }

    #[test]
    fn test_too_short_input() {
        assert!(suggest_workflow("hi").is_none());
    }

    // ── Precedence: explicit skill overrides keywords ──

    #[test]
    fn test_explicit_overrides_bugfix() {
        // Even though "bug" is present, explicit "workflow_simple" wins
        let s = suggest_workflow("workflow_simple 修复这个 bug").unwrap();
        assert_eq!(s.skill, "workflow_simple");
        assert_eq!(s.confidence, Confidence::High);
    }
}
