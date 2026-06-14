//! Scenario replay: load structured scenario JSON and replay deterministically.
//!
//! Phase 1: data structures, loader, and validation.
//! Phase 2: integration with PipelineEngine for automated replay.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A step in a scenario — each corresponds to one agent interaction round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioStep {
    /// Agent role name (e.g. "neige", "gongbu").
    pub agent: String,
    /// Mock response for this step.
    pub mock_response: MockResponse,
    /// Files that must exist after this step completes.
    #[serde(default)]
    pub expect_files: Vec<String>,
    /// Audit events that must have been emitted.
    #[serde(default)]
    pub expect_audit_events: Vec<String>,
}

/// Mock response that the replay injects instead of calling the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockResponse {
    /// Text content of the response.
    #[serde(default)]
    pub content: String,
    /// Tool calls to include (alternative to content).
    #[serde(default)]
    pub tool_calls: Vec<MockToolCall>,
}

/// A single tool call within a mock response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockToolCall {
    pub name: String,
    #[serde(default)]
    pub arguments: String,
}

/// A complete scenario definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub steps: Vec<ScenarioStep>,
    /// Expected final status.
    #[serde(default = "default_final_status")]
    pub expected_final_status: String,
}

fn default_version() -> u32 {
    2
}
fn default_final_status() -> String {
    "complete".to_string()
}

/// Load a scenario from a JSON string.
pub fn load_scenario(json: &str) -> Result<Scenario, String> {
    serde_json::from_str::<Scenario>(json).map_err(|e| format!("场景加载失败: {}", e))
}

/// Load a scenario from a file path.
pub async fn load_scenario_from_file(path: &Path) -> Result<Scenario, String> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("读取场景文件失败: {}", e))?;
    load_scenario(&content)
}

/// Validate a scenario's internal consistency.
pub fn validate_scenario(scenario: &Scenario) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if scenario.name.is_empty() {
        errors.push("场景名称为空".to_string());
    }
    if scenario.steps.is_empty() {
        errors.push("场景步骤为空".to_string());
    }

    for (i, step) in scenario.steps.iter().enumerate() {
        if step.agent.is_empty() {
            errors.push(format!("步骤 {} 的 agent 为空", i));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Collect expected file paths from all steps.
pub fn expected_files(scenario: &Scenario) -> Vec<String> {
    let mut files = Vec::new();
    for step in &scenario.steps {
        files.extend(step.expect_files.clone());
    }
    files.sort();
    files.dedup();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_minimal_scenario() {
        let json = r#"{
            "name": "Todo CLI",
            "version": 2,
            "steps": [
                {
                    "agent": "neige",
                    "mock_response": {
                        "content": "收到任务，开始规划。",
                        "tool_calls": [
                            {
                                "name": "submit_pipeline_plan",
                                "arguments": "{\"plan_id\":\"plan-001\",\"summary\":\"test\",\"steps\":[]}"
                            }
                        ]
                    },
                    "expect_files": [".shuji/pipeline/runtime.json"],
                    "expect_audit_events": ["create_document"]
                }
            ]
        }"#;

        let scenario = load_scenario(json).unwrap();
        assert_eq!(scenario.name, "Todo CLI");
        assert_eq!(scenario.steps.len(), 1);
        assert_eq!(scenario.steps[0].agent, "neige");
        assert_eq!(
            scenario.steps[0].mock_response.tool_calls[0].name,
            "submit_pipeline_plan"
        );
    }

    #[test]
    fn test_validate_valid_scenario() {
        let scenario = Scenario {
            name: "test".into(),
            version: 2,
            steps: vec![ScenarioStep {
                agent: "neige".into(),
                mock_response: MockResponse {
                    content: "ok".into(),
                    tool_calls: vec![],
                },
                expect_files: vec![],
                expect_audit_events: vec![],
            }],
            expected_final_status: "complete".into(),
        };
        assert!(validate_scenario(&scenario).is_ok());
    }

    #[test]
    fn test_validate_empty_scenario() {
        let scenario = Scenario {
            name: "".into(),
            version: 2,
            steps: vec![],
            expected_final_status: "complete".into(),
        };
        assert!(validate_scenario(&scenario).is_err());
    }

    #[test]
    fn test_expected_files_dedup() {
        let scenario = Scenario {
            name: "test".into(),
            version: 2,
            steps: vec![
                ScenarioStep {
                    agent: "a".into(),
                    mock_response: MockResponse {
                        content: "".into(),
                        tool_calls: vec![],
                    },
                    expect_files: vec!["f1.txt".into(), "f2.txt".into()],
                    expect_audit_events: vec![],
                },
                ScenarioStep {
                    agent: "b".into(),
                    mock_response: MockResponse {
                        content: "".into(),
                        tool_calls: vec![],
                    },
                    expect_files: vec!["f1.txt".into(), "f3.txt".into()],
                    expect_audit_events: vec![],
                },
            ],
            expected_final_status: "complete".into(),
        };
        let files = expected_files(&scenario);
        assert_eq!(files.len(), 3);
    }
}
