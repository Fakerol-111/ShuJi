//! StageTracker: ordered stage machine for workflow progression.
//!
//! Each profile defines a list of stages (in YAML). The StageTracker
//! holds the current stage index and enforces forward-only transitions.
//! When a profile has no explicit stages, default stages are synthesized
//! from the profile type.

/// A single workflow stage.
#[derive(Debug, Clone)]
pub struct WorkflowStage {
    pub id: String,
    pub actor: String,
    pub skill: Option<String>,
    pub output_doc: Option<String>,
    pub requires_approval: bool,
    pub description: String,
}

/// Stage tracker: holds the stage list and current position.
#[derive(Debug, Clone)]
pub struct StageTracker {
    pub stages: Vec<WorkflowStage>,
    pub current_index: usize,
}

impl StageTracker {
    /// Create a tracker from an explicit stage list.
    pub fn new(stages: Vec<WorkflowStage>) -> Self {
        Self {
            stages,
            current_index: 0,
        }
    }

    /// Generate default stages based on profile id (when YAML has no stages).
    pub fn default_for_profile(profile_id: &str) -> Self {
        let stages = match profile_id {
            "greenfield_standard" | "greenfield_full" => vec![
                stage("init", "内阁", "任务记录", false),
                stage("expand", "expand_requirements", "需求展开", false),
                stage("design", "中书令", "方案设计", false),
                stage("review", "门下侍中", "审查", true),
                stage("execution", "尚书令", "尚书令执行", false),
                stage("summary", "内阁", "汇总报告", false),
            ],
            "brownfield_optimize" => vec![
                stage("init", "内阁", "任务记录", false),
                stage("analysis", "中书令", "代码分析", false),
                stage("plan", "中书令", "优化方案", true),
                stage("execution", "尚书令", "尚书令执行", false),
                stage("summary", "内阁", "汇总报告", false),
            ],
            "bugfix" | "demo" => vec![
                stage("init", "内阁", "任务记录", false),
                stage("execution", "尚书令", "尚书令执行", false),
                stage("summary", "内阁", "汇总报告", false),
            ],
            _ => vec![
                stage("init", "内阁", "初始化", false),
                stage("execution", "尚书令", "执行", false),
                stage("summary", "内阁", "汇总", false),
            ],
        };
        Self::new(stages)
    }

    /// Current stage reference.
    pub fn current(&self) -> Option<&WorkflowStage> {
        self.stages.get(self.current_index)
    }

    /// Transition forward to the next stage. Returns false if already at the last.
    pub fn advance(&mut self) -> bool {
        if self.current_index + 1 < self.stages.len() {
            self.current_index += 1;
            true
        } else {
            false
        }
    }

    /// Check if the given target actor is allowed at the current stage.
    /// Returns Ok(()) if allowed, Err with reason if not.
    pub fn check_route_to(&self, actor: &str) -> Result<(), String> {
        let current = match self.current() {
            Some(s) => s,
            None => return Err("没有活跃的阶段".to_string()),
        };
        // Allow routing to the current stage's actor
        if current.actor == actor {
            return Ok(());
        }
        // Also allow routing to the next stage's actor (forward transition)
        if self.current_index + 1 < self.stages.len() {
            let next = &self.stages[self.current_index + 1];
            if next.actor == actor {
                return Ok(());
            }
        }
        Err(format!(
            "当前阶段「{}」(={}) 不允许路由到「{}」",
            current.id, current.actor, actor
        ))
    }

    /// Build a human-readable stage summary for session injection.
    pub fn build_injection(&self) -> String {
        let mut lines = vec!["[Workflow Stages]".to_string()];
        for (i, stage) in self.stages.iter().enumerate() {
            let marker = if i == self.current_index {
                "→"
            } else if i < self.current_index {
                "✓"
            } else {
                " "
            };
            let approval = if stage.requires_approval {
                " [需朱批]"
            } else {
                ""
            };
            lines.push(format!(
                "  {} {}. {}({}){}",
                marker,
                i + 1,
                stage.description,
                stage.actor,
                approval
            ));
        }
        lines.push(format!(
            "当前阶段: {} ({})",
            self.current().map_or("-", |s| &s.description),
            self.current_index + 1
        ));
        lines.join("\n")
    }
}

fn stage(id: &str, actor: &str, description: &str, requires_approval: bool) -> WorkflowStage {
    WorkflowStage {
        id: id.to_string(),
        actor: actor.to_string(),
        skill: None,
        output_doc: None,
        description: description.to_string(),
        requires_approval,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_stages() -> Vec<WorkflowStage> {
        vec![
            stage("init", "内阁", "任务记录", false),
            stage("design", "中书令", "方案设计", true),
            stage("execution", "尚书令", "执行", false),
        ]
    }

    #[test]
    fn test_starts_at_index_0() {
        let t = StageTracker::new(sample_stages());
        assert_eq!(t.current_index, 0);
        assert_eq!(t.current().unwrap().id, "init");
    }

    #[test]
    fn test_advance_forward() {
        let mut t = StageTracker::new(sample_stages());
        assert!(t.advance());
        assert_eq!(t.current_index, 1);
        assert_eq!(t.current().unwrap().id, "design");
    }

    #[test]
    fn test_advance_past_end() {
        let mut t = StageTracker::new(sample_stages());
        t.advance();
        t.advance();
        assert!(!t.advance()); // past end
        assert_eq!(t.current_index, 2);
    }

    #[test]
    fn test_check_route_allows_current_actor() {
        let t = StageTracker::new(sample_stages());
        assert!(t.check_route_to("内阁").is_ok());
    }

    #[test]
    fn test_check_route_allows_next_actor() {
        let t = StageTracker::new(sample_stages());
        assert!(t.check_route_to("中书令").is_ok());
    }

    #[test]
    fn test_check_route_blocks_wrong_actor() {
        let t = StageTracker::new(sample_stages());
        assert!(t.check_route_to("礼部").is_err());
    }

    #[test]
    fn test_build_injection_contains_design_marker() {
        // Advance past init to see ✓ on stage 0 and → on stage 1
        let mut t = StageTracker::new(sample_stages());
        t.advance();
        let inj = t.build_injection();
        assert!(
            inj.contains("✓"),
            "stage 0 (init) should be marked done: {}",
            inj
        );
        assert!(
            inj.contains("→"),
            "stage 1 (design) should be current: {}",
            inj
        );
        assert!(
            inj.contains("[需朱批]"),
            "design requires approval: {}",
            inj
        );
    }

    #[test]
    fn test_default_for_greenfield() {
        let t = StageTracker::default_for_profile("greenfield_standard");
        assert!(t.stages.len() >= 5);
        assert_eq!(t.stages[0].actor, "内阁");
    }

    #[test]
    fn test_default_for_bugfix() {
        let t = StageTracker::default_for_profile("bugfix");
        assert_eq!(t.stages.len(), 3);
    }
}
