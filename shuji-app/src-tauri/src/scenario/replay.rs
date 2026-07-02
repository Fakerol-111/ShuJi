//! Replay logic for scenarios.
//!
//! Phase 1: load and validate scenarios. Integration with PipelineEngine
//! for automated replay is planned for Phase 2.

use std::path::Path;

use super::{Scenario, ScenarioStep};

/// Replay a scenario against a project directory.
/// Phase 1: validates the scenario and checks expected files exist.
/// Phase 2: will step through PipelineEngine with mock responses.
pub async fn replay_scenario(
    scenario: &Scenario,
    working_dir: &Path,
) -> Result<String, Vec<String>> {
    // Validate first
    super::validate_scenario(scenario).map_err(|e| e)?;

    let mut log = Vec::new();
    log.push(format!(
        "场景「{}」开始回放 ({} 步骤)",
        scenario.name,
        scenario.steps.len()
    ));

    for (i, step) in scenario.steps.iter().enumerate() {
        log.push(format!("步骤 {}: agent={}", i + 1, step.agent));

        // Check expected files
        for file in &step.expect_files {
            let path = working_dir.join(file);
            if path.exists() {
                log.push(format!("  ✓ 文件存在: {}", file));
            } else {
                log.push(format!("  ✗ 文件缺失: {}", file));
            }
        }
    }

    log.push(format!("场景「{}」回放完成", scenario.name));
    Ok(log.join("\n"))
}

/// Check that all step expected files exist.
pub fn check_scenario_files(scenario: &Scenario, working_dir: &Path) -> Vec<String> {
    let mut missing = Vec::new();
    for step in &scenario.steps {
        for file in &step.expect_files {
            if !working_dir.join(file).exists() {
                missing.push(file.clone());
            }
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{MockResponse, ScenarioStep};

    #[tokio::test]
    async fn test_replay_empty_scenario() -> anyhow::Result<()> {
        let scenario = Scenario {
            name: "empty".into(),
            version: 2,
            steps: vec![],
            expected_final_status: "complete".into(),
        };
        let tmp = tempfile::TempDir::new()?;
        let result = replay_scenario(&scenario, tmp.path()).await;
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_check_files_present() -> anyhow::Result<()> {
        let scenario = Scenario {
            name: "check".into(),
            version: 2,
            steps: vec![ScenarioStep {
                agent: "test".into(),
                mock_response: MockResponse {
                    content: "".into(),
                    tool_calls: vec![],
                },
                expect_files: vec!["exists.txt".into()],
                expect_audit_events: vec![],
            }],
            expected_final_status: "complete".into(),
        };
        let tmp = tempfile::TempDir::new()?;
        tokio::fs::write(tmp.path().join("exists.txt"), "hello").await?;

        let missing = check_scenario_files(&scenario, tmp.path());
        assert!(missing.is_empty(), "file should exist");
        Ok(())
    }

    #[tokio::test]
    async fn test_check_files_missing() -> anyhow::Result<()> {
        let scenario = Scenario {
            name: "missing".into(),
            version: 2,
            steps: vec![ScenarioStep {
                agent: "test".into(),
                mock_response: MockResponse {
                    content: "".into(),
                    tool_calls: vec![],
                },
                expect_files: vec!["missing.txt".into()],
                expect_audit_events: vec![],
            }],
            expected_final_status: "complete".into(),
        };
        let tmp = tempfile::TempDir::new()?;
        let missing = check_scenario_files(&scenario, tmp.path());
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "missing.txt");
        Ok(())
    }
}
