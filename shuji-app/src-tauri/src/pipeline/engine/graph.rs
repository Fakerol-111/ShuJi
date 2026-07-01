//! preview_pipeline_on_graph — pre-fill workflow graph from pipeline plan steps.

use super::PipelineEngine;

impl PipelineEngine {
    /// 预填充文移图：遍历 pipeline plan 中的所有 route_to/parallel 步骤，
    /// 以 planned 状态添加到 workflow graph 中，让前端提前看到整个计划 DAG。
    pub async fn preview_pipeline_on_graph(&mut self) {
        if self.workflow_graph.is_none() {
            return;
        }

        let steps = self.runtime.plan.steps.clone();
        let mut last_role = "内阁".to_string();

        for step in &steps {
            match step.action.as_str() {
                "route_to" => {
                    let target = step
                        .action_params
                        .get("target")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !target.is_empty() {
                        if let Some(ref graph_lock) = self.workflow_graph {
                            let mut g = graph_lock.lock().await;
                            g.add_planned_edge(
                                &last_role,
                                target,
                                &step.step_id,
                                &step.description,
                            );
                            let _ = g.save_to(&self.project_dir).await;
                        }
                        last_role = target.to_string();
                    }
                }
                "parallel" => {
                    let targets: Vec<serde_json::Value> = step
                        .action_params
                        .get("targets")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    for t in &targets {
                        let target = t["target"].as_str().unwrap_or("");
                        if !target.is_empty() {
                            if let Some(ref graph_lock) = self.workflow_graph {
                                let mut g = graph_lock.lock().await;
                                let sub_step_id = format!(
                                    "{}-{}",
                                    step.step_id,
                                    t["name"].as_str().unwrap_or("sub")
                                );
                                g.add_planned_edge(
                                    &last_role,
                                    target,
                                    &sub_step_id,
                                    t["task"].as_str().unwrap_or(""),
                                );
                                let _ = g.save_to(&self.project_dir).await;
                            }
                        }
                    }
                    // 并行步骤的最后一个 target 作为下一条边的 from_role
                    if let Some(last_t) = targets.last() {
                        if let Some(t) = last_t["target"].as_str() {
                            last_role = t.to_string();
                        }
                    }
                }
                _ => {
                    // ask_user / approval_gate / self_execute — 不产生部门间边
                }
            }
        }
    }
}
