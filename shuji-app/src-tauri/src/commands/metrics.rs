//! Tauri commands for run metrics.

use crate::metrics::{list_runs, load_latest, RunMetrics, RunMetricsSummary};

#[tauri::command]
pub async fn get_latest_run_metrics(project_dir: String) -> Result<Option<RunMetrics>, String> {
    Ok(load_latest(&std::path::Path::new(&project_dir)).await)
}

#[tauri::command]
pub async fn list_run_metrics(
    project_dir: String,
    limit: Option<usize>,
) -> Result<Vec<RunMetricsSummary>, String> {
    Ok(list_runs(&std::path::Path::new(&project_dir), limit.unwrap_or(10)).await)
}
