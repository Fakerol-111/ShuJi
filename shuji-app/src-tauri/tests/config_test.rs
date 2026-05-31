use shuji_app_lib::config::{
    default_compact_thresholds_for_role, RoleContextConfig, RuntimeConfig,
};

#[test]
fn test_role_builtin_default_gongbu() {
    let t = default_compact_thresholds_for_role("工部").unwrap();
    assert_eq!(t.char_threshold, 120_000);
    assert_eq!(t.keep_recent_count, 16);
    assert_eq!(t.history_char_threshold, 3_500);
    assert!(t.mid_run_compact);
}

#[test]
fn test_role_builtin_default_libu_li() {
    let t = default_compact_thresholds_for_role("礼部").unwrap();
    assert_eq!(t.char_threshold, 50_000);
    assert_eq!(t.keep_recent_count, 6);
    assert_eq!(t.history_char_threshold, 1_500);
}

#[test]
fn test_resolve_compact_thresholds_without_override() {
    let config = RuntimeConfig::default();
    let thresholds = config.resolve_compact_thresholds("工部", None);
    assert_eq!(thresholds.char_threshold, 120_000);
    assert_eq!(thresholds.keep_recent_count, 16);
    assert_eq!(thresholds.history_char_threshold, 3_500);
    assert!(thresholds.mid_run_compact);
}

#[test]
fn test_resolve_compact_thresholds_unknown_role_uses_global() {
    let config = RuntimeConfig::default();
    let thresholds = config.resolve_compact_thresholds("gongbushangshu", None);
    assert_eq!(thresholds.char_threshold, 80_000);
    assert_eq!(thresholds.keep_recent_count, 10);
    assert_eq!(thresholds.history_char_threshold, 2_000);
    assert!(thresholds.mid_run_compact);
}

#[test]
fn test_resolve_compact_thresholds_with_partial_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        char_threshold: Some(5_000),
        keep_recent_count: None,
        history_char_threshold: None,
        mid_run_compact: None,
    };
    let thresholds = config.resolve_compact_thresholds("工部", Some(&role_cfg));
    assert_eq!(thresholds.char_threshold, 5_000); // override
    assert_eq!(thresholds.keep_recent_count, 16); // role builtin
    assert_eq!(thresholds.history_char_threshold, 3_500); // role builtin
    assert!(thresholds.mid_run_compact); // role builtin
}

#[test]
fn test_resolve_compact_thresholds_full_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        char_threshold: Some(120_000),
        keep_recent_count: Some(8),
        history_char_threshold: Some(3_000),
        mid_run_compact: Some(false),
    };
    let thresholds = config.resolve_compact_thresholds("工部", Some(&role_cfg));
    assert_eq!(thresholds.char_threshold, 120_000);
    assert_eq!(thresholds.keep_recent_count, 8);
    assert_eq!(thresholds.history_char_threshold, 3_000);
    assert!(!thresholds.mid_run_compact);
}

#[test]
fn test_resolve_compact_thresholds_mid_run_compact_false_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        char_threshold: None,
        keep_recent_count: None,
        history_char_threshold: None,
        mid_run_compact: Some(false),
    };
    let thresholds = config.resolve_compact_thresholds("工部", Some(&role_cfg));
    assert_eq!(thresholds.char_threshold, 120_000);
    assert_eq!(thresholds.keep_recent_count, 16);
    assert_eq!(thresholds.history_char_threshold, 3_500);
    assert!(!thresholds.mid_run_compact);
}

#[test]
fn test_resolve_compact_thresholds_mid_run_compact_true_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        char_threshold: None,
        keep_recent_count: None,
        history_char_threshold: None,
        mid_run_compact: Some(true),
    };
    let thresholds = config.resolve_compact_thresholds("工部", Some(&role_cfg));
    assert!(thresholds.mid_run_compact);
}
