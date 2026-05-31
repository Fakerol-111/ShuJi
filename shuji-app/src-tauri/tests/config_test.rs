use shuji_app_lib::config::{RoleContextConfig, RuntimeConfig};

#[test]
fn test_resolve_compact_thresholds_without_override() {
    let config = RuntimeConfig::default();
    let thresholds = config.resolve_compact_thresholds("gongbushangshu", None);
    assert_eq!(thresholds.char_threshold, 80_000);
    assert_eq!(thresholds.keep_recent_count, 10);
    assert_eq!(thresholds.history_char_threshold, 2_000);
}

#[test]
fn test_resolve_compact_thresholds_with_partial_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        char_threshold: Some(5_000),
        keep_recent_count: None,
        history_char_threshold: None,
    };
    let thresholds = config.resolve_compact_thresholds("gongbushangshu", Some(&role_cfg));
    assert_eq!(thresholds.char_threshold, 5_000); // override
    assert_eq!(thresholds.keep_recent_count, 10); // fallback to global
    assert_eq!(thresholds.history_char_threshold, 2_000); // fallback to global
}

#[test]
fn test_resolve_compact_thresholds_full_override() {
    let config = RuntimeConfig::default();
    let role_cfg = RoleContextConfig {
        char_threshold: Some(120_000),
        keep_recent_count: Some(8),
        history_char_threshold: Some(3_000),
    };
    let thresholds = config.resolve_compact_thresholds("gongbushangshu", Some(&role_cfg));
    assert_eq!(thresholds.char_threshold, 120_000);
    assert_eq!(thresholds.keep_recent_count, 8);
    assert_eq!(thresholds.history_char_threshold, 3_000);
}
