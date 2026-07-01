//! finalize_metrics — attach validation reports and persist run metrics.

use super::PipelineEngine;

impl PipelineEngine {
    /// Finalize run metrics by saving to disk.
    pub(crate) async fn finalize_metrics(&mut self, status: &str) {
        if let Some(ref mut metrics) = self.run_metrics {
            // Attach validation from artifacts if available
            if let Some(validation_json) = self.runtime.artifacts.get("validate_report") {
                if let Ok(report) = serde_json::from_str::<crate::validate::report::ValidationReport>(
                    validation_json,
                ) {
                    metrics.attach_validation(report);
                }
            }
            // Also check step artifacts for validate_delivery steps
            for step_id in self.runtime.artifacts.keys() {
                if step_id.contains("validate") || step_id == "v1" {
                    let report_path = self
                        .project_dir
                        .join(".shuji")
                        .join("validate")
                        .join("latest.json");
                    if let Ok(content) = std::fs::read_to_string(&report_path) {
                        if let Ok(report) = serde_json::from_str::<
                            crate::validate::report::ValidationReport,
                        >(&content)
                        {
                            metrics.attach_validation(report);
                            break;
                        }
                    }
                }
            }
            metrics.finalize(status, &self.project_dir).await.ok();
        }
    }
}
