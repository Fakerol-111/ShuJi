//! Tests for canonical pipeline plan examples stored in assets/templates/pipeline/.
//!
//! Ensures all example plans pass schema validation and use only the modern
//! protocol (dispatch_to, not legacy route_to).

use shuji_app_lib::pipeline::schema::validate_plan_json;

#[test]
fn examples_dir_readable() {
    let examples_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/templates/pipeline/");
    assert!(
        examples_dir.is_dir(),
        "pipeline examples dir must exist: {}",
        examples_dir.display()
    );
}

fn load_and_validate(name: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/templates/pipeline/")
        .join(name);
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {}", name, e));

    let result = validate_plan_json(&content);
    assert!(
        result.is_ok(),
        "example {} should pass schema validation: {:?}",
        name,
        result.err()
    );

    // Verify no legacy route_to in the plan
    let value: serde_json::Value = serde_json::from_str(&content).unwrap();
    if let Some(steps) = value.get("steps").and_then(|s| s.as_array()) {
        for (i, step) in steps.iter().enumerate() {
            if let Some(action) = step.get("action").and_then(|a| a.as_str()) {
                assert_ne!(
                    action, "route_to",
                    "example {} step {} uses legacy route_to",
                    name, i
                );
            }
        }
    }
}

#[test]
fn simple_execution_valid() {
    load_and_validate("simple-execution.json");
}

#[test]
fn design_review_execute_valid() {
    load_and_validate("design-review-execute.json");
}

#[test]
fn ask_user_then_execute_valid() {
    load_and_validate("ask-user-then-execute.json");
}

#[test]
fn parallel_branches_valid() {
    load_and_validate("parallel-branches.json");
}

#[test]
fn self_execute_validate_valid() {
    load_and_validate("self-execute-validate.json");
}

#[test]
fn all_examples_use_dispatch_to_not_route_to() {
    let examples_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/templates/pipeline/");
    for entry in std::fs::read_dir(&examples_dir).expect("examples dir") {
        let entry = entry.expect("read entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
            assert!(
                !content.contains("route_to"),
                "example {} contains legacy route_to",
                path.file_name().unwrap().to_string_lossy()
            );
        }
    }
}
