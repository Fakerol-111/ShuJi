use shuji_app_lib::config::{
    default_compact_thresholds_for_role, RoleContextConfig, RuntimeConfig,
};

const NEAR_WINDOW_TOKENS: usize = 750_000;
const NEAR_WINDOW_KEEP: usize = 24;

#[test]
fn test_role_builtin_default_gongbu() {
    let t = default_compact_thresholds_for_role("工部").unwrap();
    assert_eq!(t.token_threshold, NEAR_WINDOW_TOKENS);
    assert_eq!(t.keep_recent_count, NEAR_WINDOW_KEEP);
    assert!(!t.mid_run_compact);
}

#[test]
fn test_role_builtin_default_libu_li() {
    let t = default_compact_thresholds_for_role("礼部").unwrap();
    assert_eq!(t.token_threshold, NEAR_WINDOW_TOKENS);
    assert_eq!(t.keep_recent_count, NEAR_WINDOW_KEEP);
    assert!(!t.mid_run_compact);
}

#[test]
fn test_resolve_compact_thresholds_without_override() {
    let config = RuntimeConfig::default();
    let thresholds = config.resolve_compact_thresholds("工部", None);
    assert_eq!(thresholds.token_threshold, NEAR_WINDOW_TOKENS);
    assert_eq!(thresholds.keep_recent_count, NEAR_WINDOW_KEEP);
    assert!(!thresholds.mid_run_compact);
}

#[test]
fn test_resolve_compact_thresholds_unknown_role_uses_global() {
    let config = RuntimeConfig::default();
    let thresholds = config.resolve_compact_thresholds("gongbushangshu", None);
    assert_eq!(thresholds.token_threshold, NEAR_WINDOW_TOKENS);
    assert_eq!(thresholds.keep_recent_count, NEAR_WINDOW_KEEP);
    assert!(!thresholds.mid_run_compact);
}

#[test]
fn test_resolve_compact_thresholds_with_partial_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        token_threshold: Some(5_000),
        keep_recent_count: None,
        mid_run_compact: None,
    };
    let thresholds = config.resolve_compact_thresholds("工部", Some(&role_cfg));
    assert_eq!(thresholds.token_threshold, 5_000); // override
    assert_eq!(thresholds.keep_recent_count, NEAR_WINDOW_KEEP); // role builtin
    assert!(!thresholds.mid_run_compact); // role builtin
}

#[test]
fn test_resolve_compact_thresholds_full_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        token_threshold: Some(120_000),
        keep_recent_count: Some(8),
        mid_run_compact: Some(false),
    };
    let thresholds = config.resolve_compact_thresholds("工部", Some(&role_cfg));
    assert_eq!(thresholds.token_threshold, 120_000);
    assert_eq!(thresholds.keep_recent_count, 8);
    assert!(!thresholds.mid_run_compact);
}

#[test]
fn test_resolve_compact_thresholds_mid_run_compact_false_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        token_threshold: None,
        keep_recent_count: None,
        mid_run_compact: Some(false),
    };
    let thresholds = config.resolve_compact_thresholds("工部", Some(&role_cfg));
    assert_eq!(thresholds.token_threshold, NEAR_WINDOW_TOKENS);
    assert_eq!(thresholds.keep_recent_count, NEAR_WINDOW_KEEP);
    assert!(!thresholds.mid_run_compact);
}

#[test]
fn test_resolve_compact_thresholds_mid_run_compact_true_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        token_threshold: None,
        keep_recent_count: None,
        mid_run_compact: Some(true),
    };
    let thresholds = config.resolve_compact_thresholds("工部", Some(&role_cfg));
    assert!(thresholds.mid_run_compact);
}

#[test]
fn test_legacy_char_threshold_alias_in_role_config() {
    let json = r#"{"token_threshold": null}"#;
    let cfg: RoleContextConfig = serde_json::from_str(json).unwrap();
    assert!(cfg.token_threshold.is_none());

    let json = r#"{"char_threshold": 99999}"#;
    let cfg: RoleContextConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.token_threshold, Some(99_999));
}

// ── Watchdog default tests ─────────────────────────────────────────────

#[test]
fn test_watchdog_default_max_consecutive_errors() {
    let config = RuntimeConfig::default();
    assert_eq!(config.watchdog.max_consecutive_errors, 5);
}

#[test]
fn test_watchdog_default_same_tool_warning() {
    let config = RuntimeConfig::default();
    assert_eq!(config.watchdog.same_tool_warning_count, 3);
}

#[test]
fn test_watchdog_default_read_without_write_warning() {
    let config = RuntimeConfig::default();
    assert_eq!(config.watchdog.read_without_write_warning, 5);
}

#[test]
fn test_watchdog_default_delete_create_warning() {
    let config = RuntimeConfig::default();
    assert_eq!(config.watchdog.delete_create_warning_count, 2);
}


