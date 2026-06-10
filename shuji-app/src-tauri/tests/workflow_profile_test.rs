//! Workflow Profile 集成测试
//!
//! 验证 Workflow Profile 声明化系统的核心组件协作：
//!   - WorkflowConfig 读写
//!   - WorkflowResolver 解析 Intent + Governance
//!   - GateEngine 工具拦截
//!   - ChainEngine 执行链注入
//!   - WorkflowState 阶段跟踪
//!
//! 运行: cargo test --test workflow_profile_test -- --nocapture

mod common;

use shuji_app_lib::workflow::profile::build_active;
use shuji_app_lib::workflow::*;

// ── WorkflowConfig 测试 ───────────────────────────────────

#[tokio::test]
async fn test_config_default_when_no_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg = WorkflowConfig::load_from(tmp.path()).await;
    assert_eq!(cfg.intent, Intent::Auto);
    assert_eq!(cfg.governance, Governance::Standard);
    assert!(cfg.intent_override.is_none());
}

#[tokio::test]
async fn test_config_save_and_load() {
    let tmp = tempfile::TempDir::new().unwrap();
    let cfg = WorkflowConfig {
        intent: Intent::BrownfieldOptimize,
        governance: Governance::Fast,
        intent_override: None,
    };
    cfg.save_to(tmp.path()).await.unwrap();

    let loaded = WorkflowConfig::load_from(tmp.path()).await;
    assert_eq!(loaded.intent, Intent::BrownfieldOptimize);
    assert_eq!(loaded.governance, Governance::Fast);
}

#[tokio::test]
async fn test_config_intent_override() {
    let mut cfg = WorkflowConfig {
        intent: Intent::Auto,
        governance: Governance::Standard,
        intent_override: Some(Intent::Bugfix),
    };
    assert_eq!(cfg.effective_intent(), Intent::Bugfix);
    assert_eq!(cfg.take_override(), Some(Intent::Bugfix));
    // After taking, falls back to intent
    assert_eq!(cfg.effective_intent(), Intent::Auto);
}

// ── WorkflowResolver 测试 ─────────────────────────────────

#[tokio::test]
async fn test_resolver_brownfield_hard() {
    let cfg = WorkflowConfig {
        intent: Intent::BrownfieldOptimize,
        governance: Governance::Standard,
        intent_override: None,
    };
    let tmp = tempfile::TempDir::new().unwrap();
    let result = WorkflowResolver::resolve(&cfg, tmp.path(), "优化登录性能").await;
    assert_eq!(result.profile.profile_id, "brownfield_optimize");
    assert_eq!(result.profile.cabinet_skill, "workflow_optimize");
    assert_eq!(result.profile.execution_chain_id, "brownfield_patch");
    assert!(result.locked, "brownfield_optimize should lock the profile");
}

#[tokio::test]
async fn test_resolver_greenfield_hard() {
    let cfg = WorkflowConfig {
        intent: Intent::GreenfieldStandard,
        governance: Governance::Standard,
        intent_override: None,
    };
    let tmp = tempfile::TempDir::new().unwrap();
    let result = WorkflowResolver::resolve(&cfg, tmp.path(), "实现用户登录").await;
    assert_eq!(result.profile.profile_id, "greenfield_standard");
    assert!(result.locked);
}

#[tokio::test]
async fn test_resolver_auto_optimize_medium() {
    let cfg = WorkflowConfig {
        intent: Intent::Auto,
        governance: Governance::Standard,
        intent_override: None,
    };
    let tmp = tempfile::TempDir::new().unwrap();
    let result = WorkflowResolver::resolve(&cfg, tmp.path(), "优化接口性能到 200ms").await;
    // "优化" keyword → Medium confidence → brownfield_optimize
    assert_eq!(result.profile.profile_id, "brownfield_optimize");
    assert!(!result.locked, "Medium confidence should not lock");
    assert!(result.hint.is_some());
}

#[tokio::test]
async fn test_resolver_auto_vague_low_must_options() {
    let cfg = WorkflowConfig {
        intent: Intent::Auto,
        governance: Governance::Standard,
        intent_override: None,
    };
    let tmp = tempfile::TempDir::new().unwrap();
    let result = WorkflowResolver::resolve(&cfg, tmp.path(), "实现用户登录功能").await;
    assert_eq!(result.profile.profile_id, "greenfield_standard");
    assert!(!result.locked);
    let hint = result.hint.unwrap_or_default();
    assert!(
        hint.contains("<options>"),
        "Low confidence hint should contain <options>: {}",
        hint
    );
}

#[tokio::test]
async fn test_resolver_auto_explicit_high_locks() {
    let cfg = WorkflowConfig {
        intent: Intent::Auto,
        governance: Governance::Standard,
        intent_override: None,
    };
    let tmp = tempfile::TempDir::new().unwrap();
    let result =
        WorkflowResolver::resolve(&cfg, tmp.path(), "请用 workflow_bugfix 修复这个崩溃").await;
    assert_eq!(result.profile.profile_id, "bugfix");
    assert!(result.locked, "Explicit skill mention should lock");
}

#[tokio::test]
async fn test_resolver_default_no_config_file() {
    // Emulate no workflow_config.json → defaults to auto+standard
    let tmp = tempfile::TempDir::new().unwrap();
    // Don't create config file — load_from returns defaults
    let cfg = WorkflowConfig::load_from(tmp.path()).await;
    assert_eq!(cfg.intent, Intent::Auto);
    assert_eq!(cfg.governance, Governance::Standard);

    let result = WorkflowResolver::resolve(&cfg, tmp.path(), "一个小功能").await;
    // Auto → routing suggests demo or simple → not locked
    assert!(!result.locked);
}

#[tokio::test]
async fn test_resolver_auto_refactor_match() {
    // "重构" keyword should resolve to refactor profile at Medium confidence
    let cfg = WorkflowConfig::load_from(tempfile::TempDir::new().unwrap().path()).await;
    let tmp = tempfile::TempDir::new().unwrap();
    let result =
        WorkflowResolver::resolve(&cfg, tmp.path(), "重构用户模块架构，拆分 monolith").await;
    assert_eq!(result.profile.profile_id, "refactor");
    assert!(!result.locked);
    assert!(result.hint.is_some());
}

#[tokio::test]
async fn test_resolver_auto_audit_match() {
    // "审计" keyword should resolve to audit profile at Medium confidence
    let cfg = WorkflowConfig::load_from(tempfile::TempDir::new().unwrap().path()).await;
    let tmp = tempfile::TempDir::new().unwrap();
    let result = WorkflowResolver::resolve(&cfg, tmp.path(), "对支付模块做安全审计").await;
    assert_eq!(result.profile.profile_id, "audit");
    assert!(!result.locked);
    assert!(result.hint.is_some());
}

#[tokio::test]
async fn test_build_active_refactor() {
    let p = build_active("refactor", Governance::Standard);
    assert!(p.is_some(), "refactor profile should build");
    assert_eq!(p.unwrap().profile_id, "refactor");
}

#[tokio::test]
async fn test_build_active_audit() {
    let p = build_active("audit", Governance::Standard);
    assert!(p.is_some(), "audit profile should build");
    assert_eq!(p.unwrap().profile_id, "audit");
}

// ── GateEngine 测试 ──────────────────────────────────────

#[tokio::test]
async fn test_gate_brownfield_forbid_expand() {
    let p = build_active("brownfield_optimize", Governance::Standard).unwrap();
    let err =
        GateEngine::check_tool(&p, "expand_requirements", &serde_json::json!({})).unwrap_err();
    assert!(err.message.contains("expand_requirements"));
}

#[tokio::test]
async fn test_gate_demo_forbid_zhongshuling() {
    let p = build_active("demo", Governance::Standard).unwrap();
    let err = GateEngine::check_tool(
        &p,
        "route_to",
        &serde_json::json!({"to": "中书令", "subject": "test"}),
    )
    .unwrap_err();
    assert!(err.message.contains("中书令"));
}

#[tokio::test]
async fn test_gate_bugfix_forbid_menxia() {
    let p = build_active("bugfix", Governance::Standard).unwrap();
    let err = GateEngine::check_tool(
        &p,
        "route_to",
        &serde_json::json!({"to": "门下侍中", "subject": "test"}),
    )
    .unwrap_err();
    assert!(err.message.contains("门下侍中"));
}

#[tokio::test]
async fn test_gate_override_flag_bypasses() {
    let p = build_active("bugfix", Governance::Standard).unwrap();
    // With --override-skill-gate in subject, gate should allow
    GateEngine::check_tool(
        &p,
        "route_to",
        &serde_json::json!({"to": "中书令", "subject": "--override-skill-gate 审批"}),
    )
    .expect("--override-skill-gate should bypass gate");
}

#[tokio::test]
async fn test_gate_greenfield_standard_allows_all() {
    let p = build_active("greenfield_standard", Governance::Standard).unwrap();
    // All routes and tools should be allowed
    GateEngine::check_tool(&p, "expand_requirements", &serde_json::json!({}))
        .expect("greenfield should allow expand_requirements");
    GateEngine::check_tool(
        &p,
        "route_to",
        &serde_json::json!({"to": "门下侍中", "subject": "test"}),
    )
    .expect("greenfield should allow route to 门下侍中");
}

#[tokio::test]
async fn test_gate_greenfield_fast_overlay() {
    let p = build_active("greenfield_standard", Governance::Fast).unwrap();
    // Fast overlay should forbid expand + 门下侍中
    assert!(GateEngine::check_tool(&p, "expand_requirements", &serde_json::json!({})).is_err());
    assert!(GateEngine::check_tool(
        &p,
        "route_to",
        &serde_json::json!({"to": "门下侍中", "subject": "test"})
    )
    .is_err());
}

// ── ChainEngine 测试 ─────────────────────────────────────

#[test]
fn test_chain_greenfield_full() {
    let inj = ChainEngine::build_injection("greenfield_full").unwrap();
    assert!(inj.contains("吏部"));
    assert!(inj.contains("礼部"));
    assert!(inj.contains("5."));
    assert!(!inj.contains("brownfield"));
}

#[test]
fn test_chain_brownfield_patch() {
    let inj = ChainEngine::build_injection("brownfield_patch").unwrap();
    assert!(inj.contains("工部"));
    assert!(inj.contains("刑部"));
    assert!(!inj.contains("吏部"));
}

#[test]
fn test_chain_unknown_returns_none() {
    assert!(ChainEngine::build_injection("nonexistent").is_none());
}

// ── WorkflowState 测试 ───────────────────────────────────

#[tokio::test]
async fn test_state_lifecycle() {
    let tmp = tempfile::TempDir::new().unwrap();

    // New state
    let state = WorkflowState::new("brownfield_optimize", "standard", "brownfield_patch");
    assert_eq!(state.current_stage, "init");
    state.save_to(tmp.path()).await;

    // Load and transition
    let mut loaded = WorkflowState::load_from(tmp.path()).await.unwrap();
    assert_eq!(loaded.profile_id, "brownfield_optimize");
    loaded.transition("execution");
    loaded.save_to(tmp.path()).await;

    // Verify persistence
    let final_state = WorkflowState::load_from(tmp.path()).await.unwrap();
    assert_eq!(final_state.current_stage, "execution");
}

#[tokio::test]
async fn test_state_no_file_returns_none() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(WorkflowState::load_from(tmp.path()).await.is_none());
}
