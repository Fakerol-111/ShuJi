//! Run metrics: persistent record of each pipeline execution.
//!
//! Stored at `.shuji/metrics/run_{run_id}.json`
//! Latest always at `.shuji/metrics/latest.json`

use serde::{Deserialize, Serialize};

use crate::validate::report::ValidationReport;

/// Overall metrics for one pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetrics {
    pub run_id: String,
    pub plan_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String, // running | complete | failed | aborted
    pub steps: Vec<StepMetric>,
    pub token_summary: TokenSummary,
    pub validation: Option<ValidationReport>,
}

/// Per-step timing and result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepMetric {
    pub step_id: String,
    pub action: String,
    pub target: Option<String>,
    pub started_at: String,
    pub duration_ms: u64,
    pub status: String,
    pub tool_errors: u32,
    pub iterations: u32,
}

/// Token consumption summary for the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSummary {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// Lightweight summary for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetricsSummary {
    pub run_id: String,
    pub plan_id: String,
    pub started_at: String,
    pub status: String,
    pub step_count: usize,
    pub overall_pass: Option<bool>,
}

impl RunMetrics {
    /// Start tracking a new pipeline run.
    pub fn start(plan_id: &str) -> Self {
        let ts = chrono::Local::now().to_rfc3339();
        Self {
            run_id: format!(
                "{}_{}",
                plan_id,
                chrono::Local::now().format("%Y%m%d%H%M%S")
            ),
            plan_id: plan_id.to_string(),
            started_at: ts.clone(),
            completed_at: None,
            status: "running".to_string(),
            steps: Vec::new(),
            token_summary: TokenSummary {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            validation: None,
        }
    }

    /// Finalize the run (mark complete/failed/aborted) and save to disk.
    pub async fn finalize(
        &mut self,
        status: &str,
        project_dir: &std::path::Path,
    ) -> Result<(), String> {
        self.completed_at = Some(chrono::Local::now().to_rfc3339());
        self.status = status.to_string();
        self.save(project_dir).await
    }

    /// Persist to `.shuji/metrics/`.
    pub async fn save(&self, project_dir: &std::path::Path) -> Result<(), String> {
        let dir = project_dir.join(".shuji").join("metrics");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("create metrics dir: {}", e))?;

        // Write run-specific file
        let path = dir.join(format!("run_{}.json", self.run_id));
        let content =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize: {}", e))?;
        tokio::fs::write(&path, &content)
            .await
            .map_err(|e| format!("write metrics: {}", e))?;

        // Update latest.json
        let latest = dir.join("latest.json");
        tokio::fs::write(&latest, &content)
            .await
            .map_err(|e| format!("write latest: {}", e))?;

        Ok(())
    }

    /// Add a step metric.
    pub fn add_step(&mut self, step: StepMetric) {
        self.steps.push(step);
    }

    /// Attach validation report.
    pub fn attach_validation(&mut self, report: ValidationReport) {
        self.validation = Some(report);
    }
}

/// Load the latest run metrics from `.shuji/metrics/latest.json`.
pub async fn load_latest(project_dir: &std::path::Path) -> Option<RunMetrics> {
    let path = project_dir
        .join(".shuji")
        .join("metrics")
        .join("latest.json");
    let content = tokio::fs::read_to_string(&path).await.ok()?;
    serde_json::from_str(&content).ok()
}

/// List all run metrics files, sorted by time (newest first), limited to `limit`.
pub async fn list_runs(project_dir: &std::path::Path, limit: usize) -> Vec<RunMetricsSummary> {
    let dir = project_dir.join(".shuji").join("metrics");
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(d) => d,
        Err(_) => return vec![],
    };

    let mut summaries = Vec::new();
    let mut count = 0u32;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let fname = entry.file_name().to_string_lossy().to_string();
        if !fname.starts_with("run_") || !fname.ends_with(".json") {
            continue;
        }
        if count >= limit as u32 {
            break;
        }
        if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
            if let Ok(metrics) = serde_json::from_str::<RunMetrics>(&content) {
                summaries.push(RunMetricsSummary {
                    run_id: metrics.run_id,
                    plan_id: metrics.plan_id,
                    started_at: metrics.started_at,
                    status: metrics.status,
                    step_count: metrics.steps.len(),
                    overall_pass: metrics.validation.as_ref().map(|v| v.overall_pass),
                });
                count += 1;
            }
        }
    }

    // Sort newest first by started_at
    summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    summaries.truncate(limit);
    summaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_metrics_start_and_finalize() {
        let m = RunMetrics::start("plan-test-001");
        assert_eq!(m.status, "running");
        assert!(m.completed_at.is_none());
        assert!(m.run_id.starts_with("plan-test-001_"));
    }

    #[test]
    fn test_add_step_metric() {
        let mut m = RunMetrics::start("p1");
        m.add_step(StepMetric {
            step_id: "s1".into(),
            action: "dispatch_to".into(),
            target: Some("工部".into()),
            started_at: "2026-01-01".into(),
            duration_ms: 1000,
            status: "done".into(),
            tool_errors: 0,
            iterations: 1,
        });
        assert_eq!(m.steps.len(), 1);
        assert_eq!(m.steps[0].step_id, "s1");
    }

    #[tokio::test]
    async fn test_save_and_load_latest() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        let mut m = RunMetrics::start("plan-save-001");
        m.add_step(StepMetric {
            step_id: "s1".into(),
            action: "self_execute".into(),
            target: None,
            started_at: "2026-01-01".into(),
            duration_ms: 500,
            status: "done".into(),
            tool_errors: 0,
            iterations: 2,
        });
        m.finalize("complete", tmp.path()).await.unwrap();

        let loaded = load_latest(tmp.path()).await.unwrap();
        assert_eq!(loaded.plan_id, "plan-save-001");
        assert_eq!(loaded.status, "complete");
        assert_eq!(loaded.steps.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_list_runs() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        for i in 0..3 {
            let mut m = RunMetrics::start(&format!("plan-{:03}", i));
            m.finalize("complete", tmp.path()).await.unwrap();
        }

        let runs = list_runs(tmp.path(), 10).await;
        assert_eq!(runs.len(), 3);
        assert!(runs[0].run_id > runs[2].run_id);
        Ok(())
    }

    #[test]
    fn test_attach_validation() {
        let mut m = RunMetrics::start("p1");
        let report = ValidationReport {
            ts: "".into(),
            project_type: "rust".into(),
            overall_pass: true,
            checks: vec![],
            ctrt_id: None,
        };
        m.attach_validation(report);
        assert!(m.validation.is_some());
        assert!(matches!(m.validation.as_ref(), Some(v) if v.overall_pass));
    }

    #[tokio::test]
    async fn test_metrics_limit() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        for i in 0..5 {
            let mut m = RunMetrics::start(&format!("plan-{:03}", i));
            m.finalize("complete", tmp.path()).await.unwrap();
        }
        let runs = list_runs(tmp.path(), 2).await;
        assert_eq!(runs.len(), 2);
        Ok(())
    }
}
