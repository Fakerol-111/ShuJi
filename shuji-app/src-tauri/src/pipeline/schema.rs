//! Pipeline plan JSON Schema validation.
//!
//! Validates JSON before deserialization, providing field-level error messages.
//! Used in `tool_submit_pipeline_plan` to reject invalid plans early.

use std::collections::HashSet;
use std::path::Path;

use super::{PipelinePlan, PlanStep};

/// Error type for plan validation failures.
#[derive(Debug, Clone)]
pub struct PlanValidationError {
    pub message: String,
    pub field_path: Option<String>,
}

impl std::fmt::Display for PlanValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.field_path {
            Some(path) => write!(f, "{} (at {})", self.message, path),
            None => write!(f, "{}", self.message),
        }
    }
}

/// Validate a plan JSON string against the JSON Schema + Rust-level checks.
/// Returns deserialized `PipelinePlan` on success.
pub fn validate_plan_json(json: &str) -> Result<PipelinePlan, PlanValidationError> {
    // 1. Parse as generic Value first
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| PlanValidationError {
        message: format!("JSON 解析失败: {}", e),
        field_path: None,
    })?;

    // 2. JSON Schema validation (file embedded at compile time)
    let schema_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas/pipeline_plan.schema.json");
    let schema_content =
        std::fs::read_to_string(&schema_path).map_err(|e| PlanValidationError {
            message: format!("读取 schema 文件失败: {}", e),
            field_path: None,
        })?;

    let schema: jsonschema::JSONSchema = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .compile(
            &serde_json::from_str::<serde_json::Value>(&schema_content).map_err(|e| {
                PlanValidationError {
                    message: format!("schema 解析失败: {}", e),
                    field_path: None,
                }
            })?,
        )
        .map_err(|e| PlanValidationError {
            message: format!("schema 编译失败: {}", e),
            field_path: None,
        })?;

    if let Err(errors) = schema.validate(&value) {
        let first = errors.into_iter().next().unwrap();
        let instance_path = first.instance_path.to_string();
        return Err(PlanValidationError {
            message: format!("Schema 校验失败: {} ({})", first, instance_path),
            field_path: Some(instance_path),
        });
    }

    // 3. Deserialize to PipelinePlan
    let plan: PipelinePlan =
        serde_json::from_value(value.clone()).map_err(|e| PlanValidationError {
            message: format!("Plan 反序列化失败: {}", e),
            field_path: None,
        })?;

    // 4. Rust-level validation (Schema can't express these)
    validate_plan_semantics(&plan)?;

    Ok(plan)
}

/// Semantic checks beyond JSON Schema expressiveness.
fn validate_plan_semantics(plan: &PipelinePlan) -> Result<(), PlanValidationError> {
    let steps = &plan.steps;

    // 4a. step_id uniqueness
    let mut seen_ids = HashSet::new();
    for step in steps {
        if !seen_ids.insert(&step.step_id) {
            return Err(PlanValidationError {
                message: format!("step_id 重复: {}", step.step_id),
                field_path: Some(format!("steps[{}].step_id", step.step_id)),
            });
        }
    }

    // 4b. depends_on references must exist
    let all_ids: HashSet<&str> = steps.iter().map(|s| s.step_id.as_str()).collect();
    for step in steps {
        for dep in &step.depends_on {
            if !all_ids.contains(dep.as_str()) {
                return Err(PlanValidationError {
                    message: format!("depends_on 引用不存在的步骤: {}", dep),
                    field_path: Some(format!("steps[{}].depends_on", step.step_id)),
                });
            }
        }
    }

    // 4c. depends_on cycle detection (DFS)
    if let Some(cycle) = find_cycle(steps) {
        return Err(PlanValidationError {
            message: format!("依赖存在环: {}", cycle.join(" → ")),
            field_path: Some("depends_on".into()),
        });
    }

    // 4d. route_to target must be valid Role
    for step in steps {
        if step.action == "route_to" {
            let target = step
                .action_params
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !target.is_empty() && crate::models::role::Role::from_name(target).is_none() {
                return Err(PlanValidationError {
                    message: format!("route_to 目标不是合法部门: {}", target),
                    field_path: Some(format!("steps[{}].action_params.target", step.step_id)),
                });
            }
        }
    }

    Ok(())
}

/// Detect a cycle in depends_on graph using DFS.
/// Returns the cycle path if found.
fn find_cycle(steps: &[PlanStep]) -> Option<Vec<String>> {
    use std::collections::HashMap;

    let id_to_idx: HashMap<&str, usize> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.step_id.as_str(), i))
        .collect();

    // Build adjacency list
    let n = steps.len();
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for (i, step) in steps.iter().enumerate() {
        for dep in &step.depends_on {
            if let Some(&j) = id_to_idx.get(dep.as_str()) {
                adj[i].push(j);
            }
        }
    }

    // DFS with coloring: 0=white, 1=gray, 2=black
    let mut color = vec![0u8; n];
    let mut parent = vec![n; n];

    fn dfs(
        u: usize,
        adj: &[Vec<usize>],
        color: &mut [u8],
        parent: &mut [usize],
        steps: &[PlanStep],
    ) -> Option<Vec<String>> {
        color[u] = 1; // gray
        for &v in &adj[u] {
            if color[v] == 1 {
                // Back edge — cycle
                let mut cycle = vec![steps[v].step_id.clone()];
                let mut cur = u;
                cycle.push(steps[cur].step_id.clone());
                while cur != v {
                    cur = parent[cur];
                    cycle.push(steps[cur].step_id.clone());
                }
                cycle.reverse();
                return Some(cycle);
            }
            if color[v] == 0 {
                parent[v] = u;
                if let Some(cycle) = dfs(v, adj, color, parent, steps) {
                    return Some(cycle);
                }
            }
        }
        color[u] = 2; // black
        None
    }

    for i in 0..n {
        if color[i] == 0 {
            if let Some(cycle) = dfs(i, &adj, &mut color, &mut parent, steps) {
                return Some(cycle);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_plan_passes_validation() {
        let json = r#"{
            "plan_id": "plan-20260613-001",
            "summary": "test plan",
            "estimated_complexity": "low",
            "created": "2026-06-13T12:00:00",
            "steps": [
                {
                    "step_id": "s1",
                    "description": "step 1",
                    "action": "route_to",
                    "action_params": {"target": "工部", "task": "do work"}
                }
            ]
        }"#;
        let result = validate_plan_json(json);
        assert!(result.is_ok(), "valid plan should pass: {:?}", result.err());
    }

    #[test]
    fn test_rejects_duplicate_step_id() {
        let json = r#"{
            "plan_id": "plan-20260613-002",
            "summary": "duplicate",
            "steps": [
                {"step_id": "s1", "description": "a", "action": "route_to", "action_params": {"target": "工部", "task": "a"}},
                {"step_id": "s1", "description": "b", "action": "route_to", "action_params": {"target": "刑部", "task": "b"}}
            ]
        }"#;
        let result = validate_plan_json(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("重复"),
            "should mention duplicate: {}",
            err.message
        );
    }

    #[test]
    fn test_rejects_cycle() {
        let json = r#"{
            "plan_id": "plan-cycle",
            "summary": "cycle",
            "steps": [
                {"step_id": "s1", "description": "a", "action": "route_to", "action_params": {"target": "工部", "task": "a"}, "depends_on": ["s2"]},
                {"step_id": "s2", "description": "b", "action": "route_to", "action_params": {"target": "刑部", "task": "b"}, "depends_on": ["s1"]}
            ]
        }"#;
        let result = validate_plan_json(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("环"),
            "should mention cycle: {}",
            err.message
        );
    }

    #[test]
    fn test_rejects_invalid_action() {
        let json = r#"{
            "plan_id": "plan-bad",
            "summary": "bad action",
            "steps": [
                {"step_id": "s1", "description": "a", "action": "fly_to_moon", "action_params": {}}
            ]
        }"#;
        let result = validate_plan_json(json);
        assert!(result.is_err(), "invalid action should be rejected");
    }

    #[test]
    fn test_rejects_unknown_role() {
        let json = r#"{
            "plan_id": "plan-bad-role",
            "summary": "bad role",
            "steps": [
                {"step_id": "s1", "description": "a", "action": "route_to", "action_params": {"target": "御膳房", "task": "cook"}}
            ]
        }"#;
        let result = validate_plan_json(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("合法部门"),
            "should mention invalid dept: {}",
            err.message
        );
    }

    #[test]
    fn test_rejects_non_existent_depends_on() {
        let json = r#"{
            "plan_id": "plan-missing-dep",
            "summary": "missing dep",
            "steps": [
                {"step_id": "s1", "description": "a", "action": "route_to", "action_params": {"target": "工部", "task": "a"}, "depends_on": ["ghost_step"]}
            ]
        }"#;
        let result = validate_plan_json(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.message.contains("不存在"),
            "should mention missing dep: {}",
            err.message
        );
    }

    #[test]
    fn test_empty_steps_rejected() {
        let json = r#"{
            "plan_id": "plan-empty",
            "summary": "no steps",
            "steps": []
        }"#;
        let result = validate_plan_json(json);
        assert!(result.is_err(), "empty steps should be rejected");
    }

    #[test]
    fn test_missing_required_fields() {
        let json = r#"{
            "plan_id": "plan-no-steps",
            "summary": "missing steps"
        }"#;
        let result = validate_plan_json(json);
        assert!(result.is_err(), "missing steps should be rejected");
    }
}
